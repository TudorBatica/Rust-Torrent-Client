use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use crate::internal_events::{CoordinatorEvent, CoordinatorInput};
use crate::metadata::Torrent;
use crate::transfer::piece_picker::PiecePicker;

pub struct CoordinatorTransferState {
    pub bitfield: Bitfield,
    pub pieces_count: usize,
    pub txs_to_peers: HashMap<usize, Sender<CoordinatorEvent>>,
    pub tx_to_coordinator: Sender<CoordinatorInput>,
    pub rx_coordinator: Receiver<CoordinatorInput>,
    pub piece_picker: Arc<Mutex<PiecePicker>>,
    pub next_peer_transfer_idx: usize,
    pub download_file_name: String,
}

impl CoordinatorTransferState {
    pub fn init(torrent: &Torrent, piece_picker: Arc<Mutex<PiecePicker>>) -> Self {
        let (tx_to_self, rx) = mpsc::channel::<CoordinatorInput>(512);
        return CoordinatorTransferState {
            bitfield: Bitfield::init(torrent.piece_hashes.len()),
            pieces_count: torrent.piece_hashes.len(),
            txs_to_peers: HashMap::new(),
            tx_to_coordinator: tx_to_self,
            rx_coordinator: rx,
            piece_picker,
            next_peer_transfer_idx: 0,
            download_file_name: torrent.info.name.clone(),
        };
    }
}

pub struct PeerTransferState {
    pub transfer_idx: usize,
    pub client_bitfield: Bitfield,
    pub peer_bitfield: Bitfield,
    pub client_is_choked: bool,
    pub peer_is_choked: bool,
    pub client_is_interested: bool,
    pub peer_is_interested: bool,
    pub ongoing_requests: HashSet<(usize, usize, usize)>,
    pub piece_picker: Arc<Mutex<PiecePicker>>,
    pub download_file_name: String,
}

pub fn register_new_peer_transfer(coordinator_state: &mut CoordinatorTransferState)
                                  -> (PeerTransferState, (Sender<CoordinatorInput>, Receiver<CoordinatorEvent>)) {
    // create channel for coordinator task -> peer transfer task communication
    let (tx, rx) = mpsc::channel::<CoordinatorEvent>(512);
    let peer_transfer_state = PeerTransferState {
        transfer_idx: 0,
        client_bitfield: coordinator_state.bitfield.clone(),
        peer_bitfield: Bitfield::init(coordinator_state.pieces_count),
        client_is_choked: true,
        peer_is_choked: true,
        client_is_interested: false,
        peer_is_interested: false,
        ongoing_requests: HashSet::new(),
        piece_picker: coordinator_state.piece_picker.clone(),
        download_file_name: coordinator_state.download_file_name.clone(),
    };
    coordinator_state.txs_to_peers.insert(coordinator_state.next_peer_transfer_idx, tx);
    coordinator_state.next_peer_transfer_idx += 1;

    return (peer_transfer_state, (coordinator_state.tx_to_coordinator.clone(), rx));
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