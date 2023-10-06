use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time;
use crate::choke::handler;
use crate::choke::state::{ChokeEvent, PeerState};
use crate::core_models::events::InternalEvent;

const CHANGE_UNCHOKED_PEERS_INTERVAL_SECS: u64 = 10;
const OPTIMISTIC_UNCHOKE_INTERVAL_SECS: u64 = 30;

pub async fn spawn(output_tx: Sender<InternalEvent>, peer_transfers_count: usize)
               -> (JoinHandle<()>, Sender<ChokeEvent>) {
    let (tx_to_self, rx) = mpsc::channel::<ChokeEvent>(1024);
    let tx_to_self_clone = tx_to_self.clone();
    let handle = tokio::spawn(async move {
        return run(output_tx, tx_to_self_clone, rx, peer_transfers_count).await;
    });

    return (handle, tx_to_self);
}

async fn run(output_tx: Sender<InternalEvent>,
             tx_to_self: Sender<ChokeEvent>,
             mut rx: Receiver<ChokeEvent>,
             peer_transfers_count: usize) {
    let mut peers: HashMap<usize, PeerState> = (0..peer_transfers_count).into_iter()
        .map(|idx| (idx, PeerState::new(idx)))
        .collect();

    tokio::spawn(unchoke_peers_scheduler(tx_to_self.clone()));
    tokio::spawn(optimistic_unchoke_scheduler(tx_to_self));

    while let Some(event) = rx.recv().await {
        let internal_events = handler::handle(event, &mut peers);
        for event in internal_events {
            output_tx.send(event).await.unwrap();
        }
    }
}

async fn unchoke_peers_scheduler(tx: Sender<ChokeEvent>) {
    let mut interval = time::interval(Duration::from_secs(CHANGE_UNCHOKED_PEERS_INTERVAL_SECS));
    loop {
        interval.tick().await;
        tx.send(ChokeEvent::UnchokePeers).await.unwrap();
    }
}

async fn optimistic_unchoke_scheduler(tx: Sender<ChokeEvent>) {
    let mut interval = time::interval(Duration::from_secs(OPTIMISTIC_UNCHOKE_INTERVAL_SECS));
    loop {
        interval.tick().await;
        tx.send(ChokeEvent::OptimisticUnchoke).await.unwrap();
    }
}