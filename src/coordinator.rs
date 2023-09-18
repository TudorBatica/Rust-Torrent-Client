use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use crate::dependency_provider::TransferDeps;
use crate::core_models::events::InternalEvent;
use crate::{data_collector};
use crate::core_models::entities::Bitfield;
use crate::p2p::{state, task};
use crate::p2p::state::P2PTransferError;

pub enum TransferError {
    TrackerCallFailed(String),
}

pub async fn run(deps: Arc<dyn TransferDeps>, mut rx: Receiver<InternalEvent>) -> Result<(), TransferError> {
    // contact tracker
    let tracker_client = deps.tracker_client();
    let tracker_resp = match tracker_client.announce().await {
        Ok(resp) => resp,
        Err(err) => return Err(TransferError::TrackerCallFailed(err.to_string()))
    };

    // initialize deps & state
    let layout = deps.torrent_layout();
    let client_bitfield = Bitfield::init(layout.pieces);
    let mut endgame = false;

    // spawn tasks
    let (data_collector_handle, data_collector_tx) = data_collector::spawn(deps.clone()).await;
    let mut p2p_handles: HashMap<usize, JoinHandle<Result<(), P2PTransferError>>> = HashMap::new();
    let mut p2p_tx: HashMap<usize, Sender<state::P2PInboundEvent>> = HashMap::new();
    for (transfer_idx, peer) in tracker_resp.peers.into_iter().enumerate() {
        let (handle, tx) = task::spawn(
            peer, transfer_idx, client_bitfield.clone(), deps.clone(),
        ).await;
        p2p_handles.insert(transfer_idx, handle);
        p2p_tx.insert(transfer_idx, tx);
    }

    // broadcast incoming events
    while let Some(event) = rx.recv().await {
        match event {
            InternalEvent::BlockDownloaded(_) => {}
            InternalEvent::BlockStored(_) => {}
            InternalEvent::DataCollectorStarted(_) => {}
            InternalEvent::DownloadComplete => {}
            InternalEvent::EndGameEnabled => {
                endgame = true;
            }
            InternalEvent::PieceStored(_) => {}
        }
    }

    return Ok(());
}
