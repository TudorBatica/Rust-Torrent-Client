use std::sync::Arc;
use tokio::fs::File;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use crate::data_collector::handler;
use crate::data_collector::handler::DataCollectionResult;
use crate::data_collector::state::DataCollectorState;
use crate::file_provider::TokioFileProvider;
use crate::core_models::entities::{BlockPosition, TorrentLayout};
use crate::core_models::internal_events::CoordinatorInput;
use crate::core_models::internal_events::DownloadAssemblerEvent::{BlockAcquired, DownloadComplete, PieceAcquired};
use crate::piece_picker::PiecePicker;

pub fn spawn(piece_picker: Arc<Mutex<PiecePicker>>,
             piece_hashes: Vec<Vec<u8>>,
             layout: TorrentLayout,
             file: File,
             mut rx: Receiver<(BlockPosition, Vec<u8>)>,
             tx: Sender<CoordinatorInput>,
) -> JoinHandle<()> {
    let mut state = DataCollectorState::init(
        Box::new(TokioFileProvider::new(file)),
        piece_hashes,
        layout,
        piece_picker,
    );

    return tokio::spawn(async move {
        while let Some((block, data)) = rx.recv().await {
            match handler::handle_block(&block, data, &mut state).await {
                DataCollectionResult::NoUpdate => {}
                DataCollectionResult::BlockStored => {
                    let _ = tx.send(CoordinatorInput::DASEvent(BlockAcquired(block))).await;
                }
                DataCollectionResult::PieceAcquired => {
                    let _ = tx.send(CoordinatorInput::DASEvent(PieceAcquired(block.piece_idx))).await;
                    let _ = tx.send(CoordinatorInput::DASEvent(BlockAcquired(block))).await;
                }
                DataCollectionResult::DownloadComplete => {
                    let _ = tx.send(CoordinatorInput::DASEvent(PieceAcquired(block.piece_idx))).await;
                    let _ = tx.send(CoordinatorInput::DASEvent(BlockAcquired(block))).await;
                    let _ = tx.send(CoordinatorInput::DASEvent(DownloadComplete)).await;
                    break;
                }
            }
        }
    });
}
