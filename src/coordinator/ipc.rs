use std::collections::HashMap;
use tokio::sync::mpsc::{Sender, Receiver};
use crate::core_models::entities::DataBlock;
use crate::core_models::events::InternalEvent;
use crate::p2p::state::P2PInboundEvent;

pub async fn broadcast(mut rx: Receiver<InternalEvent>,
                       data_collector_tx: Sender<DataBlock>,
                       p2p_tx: HashMap<usize, Sender<P2PInboundEvent>>,
) {
    let mut endgame = false;
    while let Some(event) = rx.recv().await {
        match event {
            InternalEvent::BlockDownloaded(block) => {
                data_collector_tx.send(block).await.unwrap();
            }
            InternalEvent::BlockStored(block) => {
                if endgame {
                    for (_, tx) in p2p_tx.iter() {
                        tx.send(P2PInboundEvent::BlockStored(block.clone())).await.unwrap();
                    }
                }
            }
            InternalEvent::DownloadComplete => {
                break;
            }
            InternalEvent::EndGameEnabled(transfer_idx) => {
                endgame = true;
                for (idx, tx) in p2p_tx.iter() {
                    if *idx != transfer_idx {
                        tx.send(P2PInboundEvent::EndgameEnabled).await.unwrap();
                    }
                }
            }
            InternalEvent::PieceStored(piece_idx) => {
                for (_, tx) in p2p_tx.iter() {
                    tx.send(P2PInboundEvent::PieceStored(piece_idx)).await.unwrap();
                }
            }
        }
    }
}