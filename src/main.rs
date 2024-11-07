use anyhow::{Context, Result};
use matrix_sdk::{
    config::SyncSettings,
    encryption::{BackupDownloadStrategy, EncryptionSettings},
    matrix_auth::MatrixSession,
    ruma::{
        events::{
            key::verification::request::ToDeviceKeyVerificationRequestEvent,
            room::message::OriginalSyncRoomMessageEvent,
        },
        OwnedRoomId,
    },
    Client,
};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{cmp::min, path::PathBuf};

mod timeline;
mod verification;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    homeserver_url: String,
    username: String,
    password: String,
    db_path: PathBuf,
    session_path: PathBuf,

    // sets up a timeline for this room if specified
    timeline_test_room: Option<OwnedRoomId>,
}

async fn login(config: &Config) -> Result<Client> {
    log::info!(
        "Connecting: homeserver={} username={}",
        config.homeserver_url,
        config.username
    );

    let client = match config.session_path.exists() {
        true => {
            log::info!("Restoring login from session.");
            let client = Client::builder()
                .homeserver_url(config.homeserver_url.clone())
                .sqlite_store(config.db_path.clone(), None)
                .with_encryption_settings(EncryptionSettings {
                    auto_enable_cross_signing: false,
                    backup_download_strategy: BackupDownloadStrategy::AfterDecryptionFailure,
                    auto_enable_backups: false,
                })
                .build()
                .await?;

            let session_file =
                std::fs::File::open(&config.session_path).context("Unable to open session file")?;
            let session: MatrixSession =
                serde_yaml::from_reader(session_file).context("Unable to parse session file")?;
            client.restore_session(session).await?;

            client
        }
        false => {
            log::info!("Logging in with username/password.");
            let client = Client::builder()
                .homeserver_url(config.homeserver_url.clone())
                .with_encryption_settings(EncryptionSettings {
                    auto_enable_cross_signing: false,
                    backup_download_strategy: BackupDownloadStrategy::AfterDecryptionFailure,
                    auto_enable_backups: false,
                })
                .build()
                .await?;
            client
                .matrix_auth()
                .login_username(config.username.clone(), config.password.as_str())
                .initial_device_display_name("app-testing")
                .await?;

            let matrix_sdk::AuthSession::Matrix(session) =
                client.session().expect("Logged in client has no session!?")
            else {
                anyhow::bail!("Logged in client has no session!?");
            };
            let session_file = std::fs::File::create(&config.session_path)
                .context("Unable to create session file")?;
            serde_yaml::to_writer(session_file, &session)
                .context("Unable to write session to file")?;

            client
        }
    };

    Ok(client)
}
#[tokio::main]
async fn main() -> Result<()> {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    let f = std::fs::File::open("config.yaml").context("Unable to open config.yaml")?;
    let config: Config = serde_yaml::from_reader(f)?;
    let client = login(&config).await?;

    client.add_event_handler(|ev: OriginalSyncRoomMessageEvent, _: Client| async move {
        let msg = format!("{}", ev.content.body().replace(|c: char| !c.is_ascii(), ""));
        log::info!("Message: {}...", &msg[0..min(60, msg.len())]);
    });

    client.add_event_handler(
        |ev: ToDeviceKeyVerificationRequestEvent, client: Client| async move {
            let request = client
                .encryption()
                .get_verification_request(&ev.sender, &ev.content.transaction_id)
                .await
                .expect("Request object wasn't created");
            tokio::spawn(verification::request_verification_handler(client, request));
        },
    );

    // WORKING HERE: looking at the decryption task debug messages to figure out
    // if it's running. Also, try connecting this app to matrix.org in the
    // encrypted DM w/ Daniel and see if it decrypts properly.

    let sync_settings = SyncSettings::default();
    let sync_service = matrix_sdk_ui::sync_service::SyncService::builder(client.clone())
        .build()
        .await?;
    let mut state_sub = sync_service.state();
    tokio::spawn(async move {
        loop {
            let state = state_sub.next().await;
            match state {
                Some(state) => {
                    log::info!("sync_service state: {:?}", state);
                }
                None => {
                    log::info!("sync_service state: None");
                    break;
                }
            }
        }
    });
    sync_service.start().await;

    log::info!("First sync");
    client.sync_once(sync_settings.clone()).await?;

    // if timeline_test_room is set, listen to its timeline
    if let Some(room_id) = config.timeline_test_room {
        let Some(room) = client.get_room(&room_id) else {
            anyhow::bail!("Unable to find room: {}", room_id);
        };
        log::info!("Watching timeline for room: {}", room_id);
        timeline::watch_timeline(room).await?;
    }

    log::info!("Sync forever");
    client.sync(sync_settings).await?;

    Ok(())
}
