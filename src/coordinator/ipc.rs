use std::collections::HashMap;
use tokio::sync::mpsc::{Sender, Receiver};
use crate::choke::models::ChokeEvent;
use crate::core_models::entities::DataBlock;
use crate::core_models::events::InternalEvent;
use crate::p2p::models::P2PEvent;
use crate::tracker::task::TrackerEvent;

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


pub async fn broadcast_events(pieces_count: usize,
                              mut rx: Receiver<InternalEvent>,
                              choke_tx: Sender<ChokeEvent>,
                              data_collector_tx: Sender<DataBlock>,
                              p2p_tx: Vec<(usize, Sender<P2PEvent>)>,
                              tracker_tx: Sender<TrackerEvent>,
) {
    let mut p2p_transfers: HashMap<usize, PeerTransfer> = p2p_tx.into_iter()
        .map(|(idx, tx)| (idx, PeerTransfer::new(tx)))
        .collect();

    let mut stored_pieces = 0;
    let mut connected_peers = 0;

    while let Some(event) = rx.recv().await {
        match event {
            InternalEvent::BlockDownloaded(transfer_idx, block) => {
                choke_tx.send(ChokeEvent::BlockDownloadedFromPeer(transfer_idx)).await.unwrap();
                tracker_tx.send(TrackerEvent::Downloaded(block.data.len() as u64)).await.unwrap();
                data_collector_tx.send(block).await.unwrap();
            }
            InternalEvent::BlockStored(block) => {
                for peer in p2p_transfers.values().filter(|peer| peer.is_connected) {
                    let _ = peer.tx.send(P2PEvent::BlockStored(block.clone())).await;
                }
            }
            InternalEvent::DownloadComplete => {
                tracker_tx.send(TrackerEvent::CompletedAnnounce).await.unwrap();
                break;
            }
            InternalEvent::PieceStored(piece_idx) => {
                for (_, peer) in p2p_transfers.iter() {
                    let _ = peer.tx.send(P2PEvent::PieceStored(piece_idx)).await;
                }
                stored_pieces += 1;
                print_transfer_state(connected_peers, stored_pieces, pieces_count);
            }
            InternalEvent::P2PTransferTerminated(transfer_idx) => {
                let transfer = p2p_transfers.remove(&transfer_idx);
                choke_tx.send(ChokeEvent::UnregisterPeer(transfer_idx)).await.unwrap();
                if let Some(p2p_transfer) = transfer {
                    if p2p_transfer.is_connected {
                        connected_peers -= 1;
                        print_transfer_state(connected_peers, stored_pieces, pieces_count);
                    }
                }
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
                connected_peers += 1;
                print_transfer_state(connected_peers, stored_pieces, pieces_count);
            }
            InternalEvent::BlockUploaded(size) => {
                tracker_tx.send(TrackerEvent::Uploaded(size as u64)).await.unwrap();
            }
        }
    }
}

fn print_transfer_state(connected_peers: usize, stored_pieces: usize, total_pieces: usize) {
    let progress = format!("{:.1}", (stored_pieces as f64 / total_pieces as f64) * 100.0);
    print!("\rProgress: {}%({}/{} pieces) | Connected Peers: {}", progress, stored_pieces, total_pieces, connected_peers);
}
