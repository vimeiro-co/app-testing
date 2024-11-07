use std::sync::Arc;

use anyhow::Result;
use futures_util::StreamExt;
use matrix_sdk::Room;
use matrix_sdk_ui::{
    eyeball_im::VectorDiff,
    timeline::{self, RoomExt, TimelineEventItemId, TimelineItem, TimelineItemContent},
};
use tokio::{sync::Mutex, time};

pub async fn watch_timeline(room: Room) -> Result<()> {
    let timeline = room.timeline().await?;
    let (timeline_items, mut timeline_stream) = timeline.subscribe().await;
    let timeline_items = Arc::new(Mutex::new(timeline_items));

    // we'll hold up to max_items things in the timeline
    let max_items = 20;
    {
        let timeline_items = timeline_items.clone();
        tokio::spawn(async move {
            while let Some(diff) = timeline_stream.next().await {
                let mut items = timeline_items.lock().await;
                match diff {
                    VectorDiff::Append { values } => {
                        items.extend(values);
                    }
                    VectorDiff::Clear => {
                        items.clear();
                    }
                    VectorDiff::PushFront { value } => {
                        items.push_front(value);
                    }
                    VectorDiff::PushBack { value } => {
                        items.push_back(value);
                    }
                    VectorDiff::PopFront => {
                        items.pop_front();
                    }
                    VectorDiff::PopBack => {
                        items.pop_back();
                    }
                    VectorDiff::Insert { index, value } => {
                        items.insert(index, value);
                    }
                    VectorDiff::Set { index, value } => {
                        items[index] = value;
                    }
                    VectorDiff::Remove { index } => {
                        items.remove(index);
                    }
                    VectorDiff::Truncate { length, .. } => {
                        items.truncate(length);
                    }
                    VectorDiff::Reset { values } => {
                        items.clear();
                        items.extend(values);
                    }
                }
            }
        });
    }

    {
        let timeline_items = timeline_items.clone();
        tokio::spawn(async move {
            loop {
                time::sleep(time::Duration::from_secs(5)).await;

                log::info!("Timeline:");
                let items = timeline_items.lock().await;
                for item in items.iter() {
                    match display(item) {
                        Some(s) => log::info!("{}", s),
                        None => continue,
                    }
                }
                log::info!("");

                // grab some more items for next loop
                if items.len() < max_items {
                    let _ = timeline.paginate_backwards(10).await;
                }
            }
        });
    }

    Ok(())
}

// Formats a timeline item as a string for display.
fn display(item: &TimelineItem) -> Option<String> {
    match item.kind() {
        timeline::TimelineItemKind::Event(event) => {
            let event_id = match event.identifier() {
                TimelineEventItemId::EventId(id) => id.to_string(),
                TimelineEventItemId::TransactionId(id) => id.to_string(),
            };
            let body = match event.content() {
                TimelineItemContent::Message(msg) => msg.body(),
                TimelineItemContent::UnableToDecrypt(_) => "Unable to decrypt",
                _ => "---",
            };
            Some(format!("{}: {}", event_id, body).to_string())
        }
        timeline::TimelineItemKind::Virtual(_) => None,
    }
}
