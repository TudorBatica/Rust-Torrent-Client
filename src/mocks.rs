use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;
use sha1::{Digest, Sha1};
use tempfile::TempDir;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use crate::config;
use crate::config::Config;
use crate::dependency_provider::TransferDeps;
use crate::core_models::entities::{Block, DataBlock, TorrentLayout};
use crate::core_models::events::InternalEvent;
use crate::file_provider::{FileProv, TempFileProv};
use crate::p2p_conn::PeerConnector;
use crate::piece_picker::{PiecePicker, RarestPiecePicker};
use crate::tracker::{TrackerClient};

pub fn generate_mock_layout(num_of_pieces: usize, blocks_in_head_pieces: usize, blocks_in_last_piece: usize) -> TorrentLayout {
    let piece_len = config::BLOCK_SIZE_BYTES * blocks_in_head_pieces;
    let last_piece_len = if num_of_pieces > 1 { config::BLOCK_SIZE_BYTES * blocks_in_last_piece } else { piece_len };

    return TorrentLayout {
        pieces: num_of_pieces,
        head_pieces_length: piece_len,
        last_piece_length: last_piece_len,
        usual_block_length: config::BLOCK_SIZE_BYTES,
        head_pieces_last_block_length: config::BLOCK_SIZE_BYTES,
        last_piece_last_block_length: config::BLOCK_SIZE_BYTES,
        output_file_path: "".to_string(),
        blocks_in_head_pieces,
        blocks_in_last_piece,
        output_file_length: config::BLOCK_SIZE_BYTES * ((num_of_pieces - 1) * blocks_in_head_pieces + blocks_in_last_piece),
    };
}

#[derive(Clone, Debug)]
pub struct MockTorrent {
    pub pieces: Vec<Vec<Block>>,
    pub pieces_data: Vec<Vec<u8>>,
    pub piece_hashes: Vec<Vec<u8>>,
    pub layout: TorrentLayout,
}

impl MockTorrent {
    pub fn generate(num_of_pieces: usize, blocks_in_head_pieces: usize, blocks_in_last_piece: usize) -> Self {
        let mut piece_hashes = Vec::new();
        let mut pieces_data = Vec::new();
        let mut pieces: Vec<Vec<Block>> = Vec::new();
        let layout = generate_mock_layout(num_of_pieces, blocks_in_head_pieces, blocks_in_last_piece);

        for piece_idx in 0..layout.pieces {
            // add block positions for this piece
            let mut blocks: Vec<Block> = Vec::new();
            for block_idx in 0..layout.blocks_in_piece(piece_idx) {
                blocks.push(
                    Block {
                        piece_idx,
                        offset: layout.usual_block_length * block_idx,
                        length: layout.block_length(piece_idx, block_idx),
                    }
                );
            }
            pieces.push(blocks);

            // add piece data & hash
            let piece_len = layout.piece_length(piece_idx);
            let piece_data = vec![piece_idx as u8; piece_len];
            let mut hasher = Sha1::new();
            hasher.update(&piece_data);
            piece_hashes.push(hasher.finalize().to_vec());
            pieces_data.push(piece_data);
        }

        return MockTorrent {
            pieces,
            pieces_data,
            piece_hashes,
            layout,
        };
    }

    pub fn data_block(&self, piece_idx: usize, block_idx: usize) -> DataBlock {
        let block_len = self.layout.block_length(piece_idx, block_idx);
        let offset = self.layout.usual_block_length * block_idx;
        let data = self.pieces_data[piece_idx][offset..(offset + block_len)].to_vec();
        return DataBlock { piece_idx, offset, data };
    }
}

pub struct MockDepsProvider {
    _output_temp_dir: TempDir,
    piece_picker: Arc<Mutex<dyn PiecePicker>>,
    mock_torrent: MockTorrent,
    output_tx: Sender<InternalEvent>,
}

impl MockDepsProvider {
    pub fn new(mut mock_torrent: MockTorrent, output_tx: Sender<InternalEvent>) -> Self {
        // initialize a temp file where the downloaded torrent file(s) will be stored
        let output_temp_dir = tempfile::tempdir().unwrap();
        let temp_file_path = output_temp_dir.path().join("torrent_output.bin");
        let file_path = temp_file_path.to_str().unwrap().to_string();
        let temp_file = std::fs::File::create(&temp_file_path).unwrap();
        temp_file.set_len(mock_torrent.layout.output_file_length as u64).unwrap();
        mock_torrent.layout.output_file_path = file_path;

        let piece_picker = Arc::new(Mutex::new(RarestPiecePicker::init(mock_torrent.layout.clone())));
        return MockDepsProvider { _output_temp_dir: output_temp_dir, piece_picker, mock_torrent, output_tx };
    }
}

#[async_trait]
impl TransferDeps for MockDepsProvider {
    fn announce_url(&self) -> String {
        return "announce".to_string();
    }

    fn client_config(&self) -> Config {
        return Config {
            listening_port: 1483,
            client_id: "toThe3toThe6toThe9".to_string(),
        };
    }

    fn file_provider(&self) -> Box<dyn FileProv> {
        return Box::new(TempFileProv::new(self.mock_torrent.layout.clone()));
    }

    fn info_hash(&self) -> Vec<u8> {
        return vec![1, 0, 0, 0, 1, 0, 1];
    }

    fn output_tx(&self) -> Sender<InternalEvent> {
        return self.output_tx.clone();
    }

    fn peer_connector(&self) -> Box<dyn PeerConnector> {
        let connector = crate::p2p_conn::MockPeerConnector::new();
        return Box::new(connector);
    }

    fn piece_hashes(&self) -> Vec<Vec<u8>> {
        return self.mock_torrent.piece_hashes.clone();
    }

    fn piece_picker(&self) -> Arc<Mutex<dyn PiecePicker>> {
        return self.piece_picker.clone();
    }

    fn torrent_layout(&self) -> TorrentLayout {
        return self.mock_torrent.layout.clone();
    }

    fn tracker_client(&self) -> Box<dyn TrackerClient> {
        let client = crate::tracker::MockTrackerClient::new();
        return Box::new(client);
    }
}
