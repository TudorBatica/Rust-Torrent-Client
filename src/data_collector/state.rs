use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::file_provider::FileProvider;
use crate::core_models::entities::{BlockPosition, TorrentLayout};
use crate::piece_picker::PiecePicker;

pub struct DataCollectorState {
    pub file_provider: Box<dyn FileProvider>,
    pub acquired_pieces: usize,
    pub written_data: HashMap<usize, HashSet<BlockPosition>>,
    pub piece_hashes: Vec<Vec<u8>>,
    pub layout: TorrentLayout,
    pub piece_picker: Arc<Mutex<PiecePicker>>,
}

impl DataCollectorState {
    pub fn init(file_provider: Box<dyn FileProvider>,
                piece_hashes: Vec<Vec<u8>>,
                layout: TorrentLayout,
                piece_picker: Arc<Mutex<PiecePicker>>) -> Self {
        return DataCollectorState {
            acquired_pieces: 0,
            written_data: (0..piece_hashes.len()).into_iter()
                .map(|piece_idx| (piece_idx, HashSet::new()))
                .collect(),
            file_provider,
            layout,
            piece_picker,
            piece_hashes,
        };
    }
}