use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::fs::File;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use crate::config;
use crate::metadata::Torrent;

type BlockOffset = usize;
type BlockLength = usize;
type PieceIndex = usize;

pub enum InternalEvent {
    PieceAcquired(usize),
    PieceInTransfer(usize),
}

pub struct CoordinatorTransferState {
    pub acquired_pieces: u64,
    pub bitfield: Bitfield,
    pub file: File,
    pub piece_hashes: Vec<Vec<u8>>,
    pub pieces_to_download: Arc<Mutex<HashMap<usize, Vec<(BlockOffset, BlockLength)>>>>,
    pub outgoing_senders: Vec<Sender<InternalEvent>>,
    pub incoming_base_sender: Sender<InternalEvent>,
    pub incoming_receiver: Receiver<InternalEvent>,
}

impl CoordinatorTransferState {
    pub fn init(torrent: &Torrent, file: File) -> Self {
        let (incoming_tx, incoming_rx) = mpsc::channel::<InternalEvent>(512);

        let bitfield = Bitfield::init(torrent.get_pieces_hashes().len());

        let last_piece_len = torrent.info.length.expect("only single file torrents supported") - (torrent.info.piece_length * torrent.get_pieces_hashes().len() as u64 - 1);
        let mut pieces_to_download: HashMap<usize, Vec<(usize, usize)>> = HashMap::with_capacity(torrent.get_pieces_hashes().len());
        for piece_idx in 0..torrent.get_pieces_hashes().len() {
            let piece_len = if piece_idx == torrent.get_pieces_hashes().len() - 1 { last_piece_len } else { torrent.info.piece_length };
            let blocks = (piece_len as f64 / (config::BLOCK_SIZE_BYTES as f64)).ceil() as usize;
            let last_block_len = piece_len as usize - (blocks - 1) * config::BLOCK_SIZE_BYTES;
            let mut blocks_vec = Vec::with_capacity(blocks);
            for block in 0..(blocks - 1) {
                blocks_vec.push((block * config::BLOCK_SIZE_BYTES, config::BLOCK_SIZE_BYTES));
            }
            blocks_vec.push(((blocks - 1) * config::BLOCK_SIZE_BYTES, last_block_len));
            pieces_to_download.insert(piece_idx, blocks_vec);
        }

        return CoordinatorTransferState {
            acquired_pieces: 0,
            bitfield,
            file,
            pieces_to_download: Arc::new(Mutex::new(pieces_to_download)),
            piece_hashes: torrent.get_pieces_hashes(),
            outgoing_senders: vec![],
            incoming_base_sender: incoming_tx,
            incoming_receiver: incoming_rx,
        };
    }
}

pub struct PeerTransferState {
    pub client_bitfield: Bitfield,
    pub peer_bitfield: Bitfield,
    pub pieces_to_download: Arc<Mutex<HashMap<PieceIndex, Vec<(BlockOffset, BlockLength)>>>>,
    pub piece_hashes: Vec<Vec<u8>>,
    pub client_is_choked: bool,
    pub peer_is_choked: bool,
    pub client_is_interested: bool,
    pub peer_is_interested: bool,
    pub piece_in_download: Option<usize>,
    pub ongoing_requests: HashSet<(PieceIndex, BlockOffset, BlockLength)>,
}

pub fn register_new_peer_transfer(coordinator_state: &mut CoordinatorTransferState)
                                  -> (PeerTransferState, (Sender<InternalEvent>, Receiver<InternalEvent>)) {
    // create channel for coordinator task -> peer transfer task communication
    let (coord_to_peer_tx, coord_to_peer_rx) = mpsc::channel::<InternalEvent>(512);
    coordinator_state.outgoing_senders.push(coord_to_peer_tx);

    return (PeerTransferState {
        client_bitfield: coordinator_state.bitfield.clone(),
        peer_bitfield: Bitfield::init(coordinator_state.piece_hashes.len()),
        pieces_to_download: coordinator_state.pieces_to_download.clone(),
        piece_hashes: coordinator_state.piece_hashes.clone(),
        client_is_choked: true,
        peer_is_choked: true,
        client_is_interested: false,
        peer_is_interested: false,
        piece_in_download: None,
        ongoing_requests: HashSet::new(),
    }, (coordinator_state.incoming_base_sender.clone(), coord_to_peer_rx));
}

//todo: might need to move Bitfield somewhere else
#[derive(Clone)]
pub struct Bitfield {
    content: Vec<u8>,
}

impl Bitfield {
    pub fn new(bytes: Vec<u8>) -> Self {
        return Bitfield { content: bytes };
    }

    pub fn init(num_of_pieces: usize) -> Self {
        let length_in_bytes = (num_of_pieces as f64 / 8.0).ceil() as usize;
        return Bitfield { content: vec![0u8; length_in_bytes] };
    }

    pub fn piece_acquired(&mut self, piece_idx: usize) {
        let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
        let mask = 1 << (7 - bit_idx);
        self.content[byte_idx as usize] |= mask;
    }

    pub fn has_piece(&self, piece_idx: usize) -> bool {
        let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
        self.content[byte_idx as usize] & (1 << (7 - bit_idx)) != 0
    }
}


#[cfg(test)]
mod tests {
    use crate::transfer::state::Bitfield;

    #[test]
    pub fn bitfield_initialization_test() {
        let bitfield: Bitfield = Bitfield::init(4);
        assert_eq!(bitfield.content.len(), 1); // 1 byte
        assert_eq!(bitfield.content[0], 0);
    }

    #[test]
    pub fn bitfield_update_test() {
        let mut bitfield: Bitfield = Bitfield::init(4);
        bitfield.piece_acquired(3);
        bitfield.piece_acquired(1);
        bitfield.piece_acquired(2);
        assert_eq!(bitfield.content[0], 112); // internal representation should be 01110000
    }

    #[test]
    pub fn bitfield_has_test() {
        let mut bitfield: Bitfield = Bitfield::init(4);
        bitfield.piece_acquired(1);
        bitfield.piece_acquired(2);
        assert_eq!(bitfield.has_piece(1) && bitfield.has_piece(2), true);
        assert_eq!(!bitfield.has_piece(0) && !bitfield.has_piece(3), true);
    }
}