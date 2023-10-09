use std::collections::HashMap;
use tokio::sync::mpsc::{Sender, Receiver};
use crate::choke::models::ChokeEvent;
use crate::core_models::entities::DataBlock;
use crate::core_models::events::InternalEvent;
use crate::p2p::models::P2PEvent;

struct PeerTransfer {
    tx: Sender<P2PEvent>,
    is_connected: bool,
}

impl PeerTransfer {
    fn new(tx: Sender<P2PEvent>) -> Self {
        return PeerTransfer {
            tx,
            is_connected: false,
        };
    }
}


pub async fn broadcast_events(mut rx: Receiver<InternalEvent>,
                              choke_tx: Sender<ChokeEvent>,
                              data_collector_tx: Sender<DataBlock>,
                              p2p_tx: Vec<(usize, Sender<P2PEvent>)>,
) {
    let mut p2p_transfers: HashMap<usize, PeerTransfer> = p2p_tx.into_iter()
        .map(|(idx, tx)| (idx, PeerTransfer::new(tx)))
        .collect();
    while let Some(event) = rx.recv().await {
        match event {
            InternalEvent::BlockDownloaded(transfer_idx, block) => {
                choke_tx.send(ChokeEvent::BlockDownloadedFromPeer(transfer_idx)).await.unwrap();
                data_collector_tx.send(block).await.unwrap();
            }
            InternalEvent::BlockStored(block) => {
                for peer in p2p_transfers.values().filter(|peer| peer.is_connected) {
                    let _ = peer.tx.send(P2PEvent::BlockStored(block.clone())).await;
                }
            }
            InternalEvent::DownloadComplete => {
                break;
            }
            InternalEvent::PieceStored(piece_idx) => {
                for (_, peer) in p2p_transfers.iter() {
                    let _ = peer.tx.send(P2PEvent::PieceStored(piece_idx)).await;
                }
            }
            InternalEvent::P2PTransferTerminated(transfer_idx) => {
                p2p_transfers.remove(&transfer_idx);
                choke_tx.send(ChokeEvent::UnregisterPeer(transfer_idx)).await.unwrap();
            }
            InternalEvent::ChokePeer(transfer_idx) => {
                match p2p_transfers.get(&transfer_idx) {
                    None => {}
                    Some(peer) => { let _ = peer.tx.send(P2PEvent::ChokePeer).await; }
                }
            }
            InternalEvent::UnchokePeer(transfer_idx) => {
                match p2p_transfers.get(&transfer_idx) {
                    None => {}
                    Some(peer) => { let _ = peer.tx.send(P2PEvent::UnchokePeer).await; }
                }
            }
            InternalEvent::ClientInterestedInPeer(idx, interested) => {
                choke_tx.send(ChokeEvent::ClientInterestedInPeer(idx, interested)).await.unwrap()
            }
            InternalEvent::PeerInterestedInClient(idx, interested) => {
                choke_tx.send(ChokeEvent::PeerInterestedInClient(idx, interested)).await.unwrap()
            }
            InternalEvent::PeerConnectionEstablished(idx) => {
                match p2p_transfers.get_mut(&idx) {
                    None => {}
                    Some(peer) => { peer.is_connected = true; }
                }
            }
        }
    }
}