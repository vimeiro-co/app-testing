use futures_util::StreamExt;
use matrix_sdk::{
    crypto::SasState,
    encryption::verification::{
        SasVerification, Verification, VerificationRequest, VerificationRequestState,
    },
    Client,
};
use std::time::Duration;

pub async fn request_verification_handler(client: Client, request: VerificationRequest) {
    log::info!(
        "Accepting verification request from {}",
        request.other_user_id(),
    );
    request
        .accept()
        .await
        .expect("Can't accept verification request");

    log::info!("Starting verification request");
    let mut stream = request.changes();

    log::info!("Waiting on stream changes");

    while let Some(state) = stream.next().await {
        match state {
            VerificationRequestState::Created { .. }
            | VerificationRequestState::Requested { .. }
            | VerificationRequestState::Ready { .. } => (),
            VerificationRequestState::Transitioned { verification } => {
                if let Verification::SasV1(s) = verification {
                    tokio::spawn(sas_verification_handler(client, s));
                    break;
                }
            }
            VerificationRequestState::Done | VerificationRequestState::Cancelled(_) => break,
        }
    }
    log::info!("Verification request finished");
}

async fn sas_verification_handler(client: Client, sas: SasVerification) {
    log::info!(
        "Starting verification with {} {}",
        &sas.other_device().user_id(),
        &sas.other_device().device_id()
    );
    sas.accept().await.unwrap();

    let mut stream = sas.changes();
    while let Some(state) = stream.next().await {
        match state {
            SasState::KeysExchanged {
                emojis: _,
                decimals: _,
            } => {
                // auto confirm
                let s = sas.clone();
                tokio::spawn(async move {
                    log::info!("Received SAS codes from other device, auto confirming in 5...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    match s.confirm().await {
                        Ok(r) => log::info!("Confirmed: {:?}", r),
                        Err(e) => log::info!("Error confirming: {:?}", e),
                    }
                });
            }
            SasState::Done { .. } => {
                let device = sas.other_device();

                log::info!(
                    "Successfully verified device {} {} {:?}",
                    device.user_id(),
                    device.device_id(),
                    device.local_trust_state()
                );
                break;
            }
            SasState::Cancelled(cancel_info) => {
                log::info!("Verification cancelled, reason: {}", cancel_info.reason());
                break;
            }
            SasState::Created { .. }
            | SasState::Started { .. }
            | SasState::Accepted { .. }
            | SasState::Confirmed => (),
        }
    }
}
