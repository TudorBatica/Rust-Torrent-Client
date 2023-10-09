use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time;
use crate::tracker::client::{TrackerClient, TrackerRequestEvent};

pub enum TrackerEvent {
    Downloaded(u64),
    Uploaded(u64),
    RegularAnnounce,
    CompletedAnnounce,
}

pub fn spawn(client: Box<dyn TrackerClient>, interval: u64)
                   -> (JoinHandle<()>, Sender<TrackerEvent>) {
    let (tx_to_self, rx) = mpsc::channel::<TrackerEvent>(1024);
    let tx_to_self_clone = tx_to_self.clone();
    let handle = tokio::spawn(async move {
        return run(tx_to_self_clone, rx, interval, client).await;
    });

    return (handle, tx_to_self);
}

async fn run(tx_to_self: Sender<TrackerEvent>, mut rx: Receiver<TrackerEvent>,
             interval: u64, client: Box<dyn TrackerClient>) {
    let mut downloaded: u64 = 0;
    let mut uploaded: u64 = 0;

    let regular_announce_handle = tokio::spawn(regular_announce_scheduler(tx_to_self, interval));

    while let Some(event) = rx.recv().await {
        match event {
            TrackerEvent::Downloaded(size) => downloaded += size,
            TrackerEvent::Uploaded(size) => uploaded += size,
            TrackerEvent::RegularAnnounce => {
                println!("Regular announce: up {} down {}", uploaded, downloaded);
                let _ = client.announce(TrackerRequestEvent::Regular(downloaded, uploaded)).await;
            }
            TrackerEvent::CompletedAnnounce => {
                println!("Completed announce: up {} down {}", uploaded, downloaded);
                let _ = client.announce(TrackerRequestEvent::Completed(downloaded, uploaded)).await;
                regular_announce_handle.abort();
                break;
            }
        }
    }
}

async fn regular_announce_scheduler(tx: Sender<TrackerEvent>, interval: u64) {
    let mut interval = time::interval(Duration::from_secs(interval));
    interval.tick().await;
    loop {
        interval.tick().await;
        tx.send(TrackerEvent::RegularAnnounce).await.unwrap();
    }
}