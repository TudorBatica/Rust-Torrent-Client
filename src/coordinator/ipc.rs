use std::collections::HashMap;
use tokio::sync::mpsc::{Sender, Receiver};
use crate::core_models::entities::DataBlock;
use crate::core_models::events::InternalEvent;
use crate::p2p::state::P2PInboundEvent;

pub async fn broadcast_events(mut rx: Receiver<InternalEvent>,
                              data_collector_tx: Sender<DataBlock>,
                              mut p2p_tx: HashMap<usize, Sender<P2PInboundEvent>>,
) {
    while let Some(event) = rx.recv().await {
        match event {
            InternalEvent::BlockDownloaded(block) => {
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
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use tokio::sync::mpsc;
    use crate::coordinator::ipc;
    use crate::core_models::entities::{DataBlock};
    use crate::core_models::events::InternalEvent;

    #[tokio::test]
    async fn test_broadcast_block_downloaded() {
        let (tx, rx) = mpsc::channel(32);
        let (data_collector_tx, mut data_collector_rx) = mpsc::channel(32);
        let p2p_tx = HashMap::new();

        let block = DataBlock::new(1, 2, vec![0x01, 0x02, 0x03]);
        let event = InternalEvent::BlockDownloaded(block.clone());
        tx.send(event).await.unwrap();

        tokio::spawn(async move { ipc::broadcast_events(rx, data_collector_tx, p2p_tx).await; });

        let received_event = data_collector_rx.recv().await.unwrap();
        assert_eq!(received_event, block);
    }
}
