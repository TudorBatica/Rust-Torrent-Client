use std::collections::HashMap;
use tokio::sync::mpsc::{Sender, Receiver};
use crate::choke::state::ChokeEvent;
use crate::core_models::entities::DataBlock;
use crate::core_models::events::InternalEvent;
use crate::p2p::state::P2PInboundEvent;

pub async fn broadcast_events(mut rx: Receiver<InternalEvent>,
                              choke_tx: Sender<ChokeEvent>,
                              data_collector_tx: Sender<DataBlock>,
                              mut p2p_tx: HashMap<usize, Sender<P2PInboundEvent>>,
) {
    while let Some(event) = rx.recv().await {
        match event {
            InternalEvent::BlockDownloaded(transfer_idx, block) => {
                choke_tx.send(ChokeEvent::BlockDownloadedFromPeer(transfer_idx)).await.unwrap();
                data_collector_tx.send(block).await.unwrap();
            }
            InternalEvent::BlockStored(block) => {
                for (_, tx) in p2p_tx.iter() {
                    let _ = tx.send(P2PInboundEvent::BlockStored(block.clone())).await;
                }
            }
            InternalEvent::DownloadComplete => {
                break;
            }
            InternalEvent::PieceStored(piece_idx) => {
                for (_, tx) in p2p_tx.iter() {
                    let _ = tx.send(P2PInboundEvent::PieceStored(piece_idx)).await;
                }
            }
            InternalEvent::P2PTransferTerminated(transfer_idx) => {
                p2p_tx.remove(&transfer_idx);
                choke_tx.send(ChokeEvent::UnregisterPeer(transfer_idx)).await.unwrap();
            }
            InternalEvent::ChokePeer(transfer_idx) => {
                match p2p_tx.get(&transfer_idx) {
                    None => {}
                    Some(tx) => { let _ = tx.send(P2PInboundEvent::Choke).await; }
                }
            }
            InternalEvent::UnchokePeer(transfer_idx) => {
                match p2p_tx.get(&transfer_idx) {
                    None => {}
                    Some(tx) => { let _ = tx.send(P2PInboundEvent::Unchoke).await; }
                }
            }
            InternalEvent::ClientInterestedInPeer(idx, interested) => {
                choke_tx.send(ChokeEvent::ClientInterestedInPeer(idx, interested)).await.unwrap()
            }
            InternalEvent::PeerInterestedInClient(idx, interested) => {
                choke_tx.send(ChokeEvent::PeerInterestedInClient(idx, interested)).await.unwrap()
            }
        }
    }
}