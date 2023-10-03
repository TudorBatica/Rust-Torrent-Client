use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use std::collections::HashMap;
use tokio::task::JoinHandle;
use crate::coordinator::ipc;
use crate::core_models::entities::{Bitfield, Peer};
use crate::core_models::events::InternalEvent;
use crate::data_collector;
use crate::dependency_provider::TransferDeps;
use crate::p2p::state::{P2PInboundEvent, P2PTransferError};
use crate::p2p::task;

#[derive(Debug)]
pub enum TransferError {
    TrackerCallFailed(String),
}

pub async fn run(deps: Arc<dyn TransferDeps>, rx: Receiver<InternalEvent>) -> Result<(), TransferError> {
    let tracker_client = deps.tracker_client();
    let layout = deps.torrent_layout();
    let client_bitfield = Bitfield::init(layout.pieces);

    let tracker_resp = match tracker_client.announce().await {
        Ok(resp) => resp,
        Err(err) => {
            println!("Initial announce failed {:?}", err);
            return Err(TransferError::TrackerCallFailed(err.to_string()));
        }
    };

    let (data_collector_handle, data_collector_tx) = data_collector::spawn(deps.clone()).await;
    let (_p2p_handles, p2p_tx) = spawn_p2p_tasks(deps.clone(), client_bitfield.clone(), tracker_resp.peers).await;

    ipc::broadcast_events(rx, data_collector_tx, p2p_tx).await;
    data_collector_handle.await.unwrap();

    return Ok(());
}

async fn spawn_p2p_tasks(deps: Arc<dyn TransferDeps>, client_bitfield: Bitfield, peers: Vec<Peer>)
                         -> (HashMap<usize, JoinHandle<Result<(), P2PTransferError>>>,
                             HashMap<usize, Sender<P2PInboundEvent>>) {
    let mut p2p_handles: HashMap<usize, JoinHandle<Result<(), P2PTransferError>>> = HashMap::new();
    let mut p2p_tx: HashMap<usize, Sender<P2PInboundEvent>> = HashMap::new();
    for (transfer_idx, peer) in peers.into_iter().enumerate() {
        let (handle, tx) = task::spawn(
            peer, transfer_idx, client_bitfield.clone(), deps.clone(),
        ).await;
        p2p_handles.insert(transfer_idx, handle);
        p2p_tx.insert(transfer_idx, tx);
    }

    return (p2p_handles, p2p_tx);
}
