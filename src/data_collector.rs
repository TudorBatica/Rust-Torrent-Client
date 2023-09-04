use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sha1::{Digest, Sha1};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use crate::core_models::entities::{BlockPosition, TorrentLayout};
use crate::core_models::events::InternalEvent;
use crate::file_provider::FileProvider;
use crate::piece_picker::PiecePicker;

pub struct DataCollectorState {
    pub acquired_pieces: usize,
    pub written_data: HashMap<usize, HashSet<BlockPosition>>,
}

impl DataCollectorState {
    pub fn init(num_of_pieces: usize) -> Self {
        return DataCollectorState {
            acquired_pieces: 0,
            written_data: (0..num_of_pieces)
                .into_iter()
                .map(|piece_idx| (piece_idx, HashSet::new()))
                .collect(),
        };
    }
}

pub async fn run(piece_picker: Arc<Mutex<dyn PiecePicker>>,
                 mut file_provider: Box<dyn FileProvider>,
                 piece_hashes: Vec<Vec<u8>>,
                 layout: TorrentLayout,
                 mut rx: Receiver<(BlockPosition, Vec<u8>)>,
                 tx: Sender<InternalEvent>) {
    let mut state = DataCollectorState::init(layout.pieces);
    while let Some((block, data)) = rx.recv().await {
        let blocks = state.written_data.get_mut(&block.piece_idx).unwrap();
        if !blocks.insert(block.clone()) {
            continue;
        }

        // write the new block
        let block_absolute_offset = block.piece_idx * layout.head_pieces_length + block.offset;
        file_provider.write(block_absolute_offset, data).await;

        // check if we have a complete piece
        if blocks.len() < layout.blocks_in_piece(block.piece_idx) {
            {
                let mut picker = piece_picker.lock().await;
                picker.remove_block(&block);
            }
            tx.send(InternalEvent::BlockStored(block)).await.unwrap();
            continue;
        }

        // compute piece hash
        let piece_offset = block.piece_idx * layout.head_pieces_length;
        let piece_len = layout.piece_length(block.piece_idx);
        let piece_buff = file_provider.read(piece_offset, piece_len).await;
        let mut hasher = Sha1::new();
        hasher.update(piece_buff);
        let piece_hash = hasher.finalize().into_iter().collect::<Vec<u8>>();

        // if piece is corrupt, reinsert it for download
        if piece_hash.cmp(&piece_hashes[block.piece_idx]).is_ne() {
            {
                let mut picker = piece_picker.lock().await;
                picker.reinsert_piece(block.piece_idx);
            }
            state.written_data.insert(block.piece_idx, HashSet::new());
            continue;
        }

        // new piece acquired! :)
        state.acquired_pieces += 1;
        {
            let mut picker = piece_picker.lock().await;
            picker.remove_block(&block);
        }

        let piece_idx = block.piece_idx.clone();
        tx.send(InternalEvent::BlockStored(block)).await.unwrap();
        tx.send(InternalEvent::PieceStored(piece_idx)).await.unwrap();
        if state.acquired_pieces == layout.pieces {
            tx.send(InternalEvent::DownloadComplete).await.unwrap();
        }
    }
}
