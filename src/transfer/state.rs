use std::collections::HashSet;
use tokio::fs::File;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::metadata::Torrent;

pub struct InternalEvent {}

pub struct CoordinatorTransferState {
    pub acquired_pieces: u64,
    pub bitfield: Bitfield,
    pub file: File,
    pub piece_hashes: Vec<Vec<u8>>,
    pub pieces_to_download: HashSet<usize>,
    pub outgoing_senders: Vec<Sender<InternalEvent>>,
    pub incoming_base_sender: Sender<InternalEvent>,
    pub incoming_receiver: Receiver<InternalEvent>,
}

impl CoordinatorTransferState {
    pub fn init(torrent: &Torrent, file: File) -> Self {
        let piece_hashes = torrent.get_pieces_hashes();
        let bitfield = Bitfield::init(piece_hashes.len());
        let pieces_to_download: HashSet<usize> = (0..piece_hashes.len()).collect();
        let (incoming_tx, incoming_rx) = mpsc::channel::<InternalEvent>(512);

        return CoordinatorTransferState {
            acquired_pieces: 0,
            bitfield,
            file,
            pieces_to_download,
            piece_hashes,
            outgoing_senders: vec![],
            incoming_base_sender: incoming_tx,
            incoming_receiver: incoming_rx,
        };
    }
}

pub struct PeerTransferState {
    pub client_bitfield: Bitfield,
    pub peer_bitfield: Bitfield,
    pub pieces_to_download: HashSet<usize>,
    pub piece_hashes: Vec<Vec<u8>>,
    pub client_is_choked: bool,
    pub peer_is_choked: bool,
    pub client_is_interested: bool,
    pub peer_is_interested: bool,
    pub incoming_receiver: Receiver<InternalEvent>,
    pub outgoing_sender: Sender<InternalEvent>,
}

pub fn register_new_peer_transfer(coordinator_state: &mut CoordinatorTransferState) -> PeerTransferState {
    // create channel for coordinator task -> peer transfer task communication
    let (coord_to_peer_tx, coord_to_peer_rx) = mpsc::channel::<InternalEvent>(512);
    coordinator_state.outgoing_senders.push(coord_to_peer_tx);

    return PeerTransferState {
        client_bitfield: coordinator_state.bitfield.clone(),
        peer_bitfield: Bitfield::init(coordinator_state.piece_hashes.len()),
        pieces_to_download: coordinator_state.pieces_to_download.clone(),
        piece_hashes: coordinator_state.piece_hashes.clone(),
        client_is_choked: true,
        peer_is_choked: true,
        client_is_interested: false,
        peer_is_interested: false,
        outgoing_sender: coordinator_state.incoming_base_sender.clone(),
        incoming_receiver: coord_to_peer_rx,
    };
}

//todo: might need to move Bitfield somewhere else
#[derive(Clone)]
pub struct Bitfield {
    content: Vec<u8>,
}

impl Bitfield {
    pub fn init(num_of_pieces: usize) -> Self {
        let length_in_bytes = (num_of_pieces as f64 / 8.0).ceil() as usize;
        return Bitfield { content: vec![0u8; length_in_bytes] };
    }

    pub fn from(pieces: Vec<bool>) -> Self {
        let length = (pieces.len() as f64 / 8.0).ceil() as usize;
        let mut content = vec![0u8; length];
        for (piece_idx, &piece_available) in pieces.iter().enumerate() {
            if piece_available {
                let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
                let mask = 1 << (7 - bit_idx);
                content[byte_idx] |= mask;
            }
        }
        Bitfield { content }
    }

    pub fn piece_acquired(&mut self, piece_idx: u64) {
        let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
        let mask = 1 << (7 - bit_idx);
        self.content[byte_idx as usize] |= mask;
    }

    pub fn has_piece(&self, piece_idx: u64) -> bool {
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