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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use tokio::sync::mpsc;
    use crate::coordinator::ipc;
    use crate::core_models::entities::{Block, DataBlock};
    use crate::core_models::events::InternalEvent;

    #[tokio::test]
    async fn test_broadcast_block_downloaded() {
        let (tx, rx) = mpsc::channel(32);
        let (data_collector_tx, mut data_collector_rx) = mpsc::channel(32);
        let p2p_tx = HashMap::new();

        let block = DataBlock::new(1, 2, vec![0x01, 0x02, 0x03]);
        let event = InternalEvent::BlockDownloaded(block.clone());
        tx.send(event).await.unwrap();

        tokio::spawn(async move { ipc::broadcast(rx, data_collector_tx, p2p_tx).await; });

        let received_event = data_collector_rx.recv().await.unwrap();
        assert_eq!(received_event, block);
    }
}
