use std::sync::Arc;
use log::{error, info};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use crate::coordinator::ipc;
use crate::core_models::entities::{Bitfield, Peer};
use crate::core_models::events::InternalEvent;
use crate::{choke, data_collector, tracker};
use crate::dependency_provider::TransferDeps;
use crate::p2p::models::{P2PEvent, P2PError};
use crate::p2p::task;
use crate::tracker::client::{TrackerClient, TrackerRequestEvent, TrackerResponse};

#[derive(Debug)]
pub enum TransferError {
    TrackerCallFailed(String),
}

pub async fn run(deps: Arc<dyn TransferDeps>, rx: Receiver<InternalEvent>) -> Result<(), TransferError> {
    info!("Starting transfer at... {}", chrono::prelude::Utc::now());

    let tracker_client = deps.tracker_client();
    let layout = deps.torrent_layout();
    let client_bitfield = Bitfield::init(layout.pieces);

    let tracker_resp = call_initial_announce(&tracker_client).await?;

    let (_data_collector_handle, data_collector_tx) = data_collector::spawn(deps.clone());
    let (_p2p_handles, p2p_tx) = spawn_p2p_tasks(deps.clone(), client_bitfield.clone(), tracker_resp.peers);
    let (_choke_handle, choke_tx) = choke::task::spawn(deps.output_tx().clone(), p2p_tx.len());
    let (tracker_handle, tracker_tx) = tracker::task::spawn(tracker_client, tracker_resp.interval);

    ipc::broadcast_events(rx, choke_tx, data_collector_tx, p2p_tx, tracker_tx).await;
    let _ = tracker_handle.await;

    info!("Transfer completed at... {}", chrono::prelude::Utc::now());

    return Ok(());
}

async fn call_initial_announce(client: &Box<dyn TrackerClient>) -> Result<TrackerResponse, TransferError> {
    return match client.announce(TrackerRequestEvent::Started).await {
        Ok(resp) => Ok(resp),
        Err(err) => {
            error!("Initial announce failed {:?}", err);
            return Err(TransferError::TrackerCallFailed(err.to_string()));
        }
    };
}

fn spawn_p2p_tasks(deps: Arc<dyn TransferDeps>, client_bitfield: Bitfield, peers: Vec<Peer>)
                   -> (Vec<(usize, JoinHandle<Result<(), P2PError>>)>,
                       Vec<(usize, Sender<P2PEvent>)>) {
    let mut p2p_handles: Vec<(usize, JoinHandle<Result<(), P2PError>>)> = vec![];
    let mut p2p_tx: Vec<(usize, Sender<P2PEvent>)> = vec![];
    for (transfer_idx, peer) in peers.into_iter().enumerate() {
        let (handle, tx) = task::spawn(
            peer, transfer_idx, client_bitfield.clone(), deps.clone(),
        );
        p2p_handles.push((transfer_idx, handle));
        p2p_tx.push((transfer_idx, tx));
    }

    return (p2p_handles, p2p_tx);
}
