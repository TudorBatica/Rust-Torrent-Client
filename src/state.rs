use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use crate::core_models::entities::{Bitfield, Block, TorrentLayout};
use crate::core_models::events::InternalEvent;
use crate::core_models::entities::Torrent;
use crate::p2p::transfer::InboundEvent;
use crate::piece_picker::RarestPiecePicker;

pub struct CoordinatorTransferState {
    pub bitfield: Bitfield,
    pub pieces_count: usize,
    pub txs_to_peers: HashMap<usize, Sender<InboundEvent>>,
    pub tx_to_coordinator: Sender<InternalEvent>,
    pub rx_coordinator: Receiver<InternalEvent>,
    pub piece_picker: Arc<Mutex<RarestPiecePicker>>,
    pub next_peer_transfer_idx: usize,
    pub download_file_name: String,
}

impl CoordinatorTransferState {
    pub fn init(torrent: &Torrent, piece_picker: Arc<Mutex<RarestPiecePicker>>) -> Self {
        let (tx_to_self, rx) = mpsc::channel::<InternalEvent>(512);
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
    pub torrent_layout: TorrentLayout,
    pub client_bitfield: Bitfield,
    pub peer_bitfield: Bitfield,
    pub client_is_choked: bool,
    pub peer_is_choked: bool,
    pub client_is_interested: bool,
    pub peer_is_interested: bool,
    pub ongoing_requests: HashSet<Block>,
    pub piece_picker: Arc<Mutex<RarestPiecePicker>>,
}

pub fn register_new_peer_transfer(coordinator_state: &mut CoordinatorTransferState, layout: TorrentLayout)
                                  -> (PeerTransferState, (Sender<InternalEvent>, Receiver<InboundEvent>)) {
    // create channel for coordinator task -> peer p2p task communication
    let (tx, rx) = mpsc::channel::<InboundEvent>(512);
    let peer_transfer_state = PeerTransferState {
        transfer_idx: 0,
        torrent_layout: layout,
        client_bitfield: coordinator_state.bitfield.clone(),
        peer_bitfield: Bitfield::init(coordinator_state.pieces_count),
        client_is_choked: true,
        peer_is_choked: true,
        client_is_interested: false,
        peer_is_interested: false,
        ongoing_requests: HashSet::new(),
        piece_picker: coordinator_state.piece_picker.clone(),
    };
    coordinator_state.txs_to_peers.insert(coordinator_state.next_peer_transfer_idx, tx);
    coordinator_state.next_peer_transfer_idx += 1;

    return (peer_transfer_state, (coordinator_state.tx_to_coordinator.clone(), rx));
}