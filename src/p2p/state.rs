use std::collections::HashSet;
use crate::core_models::entities::{Bitfield, Block, Message};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2PState {
    pub transfer_idx: usize,
    pub client_bitfield: Bitfield,
    pub peer_bitfield: Bitfield,
    pub client_is_choked: bool,
    pub peer_is_choked: bool,
    pub client_is_interested: bool,
    pub peer_is_interested: bool,
    pub ongoing_requests: HashSet<Block>,
}

impl P2PState {
    pub fn new(transfer_idx: usize, client_bitfield: Bitfield, num_of_pieces: usize) -> Self {
        return P2PState {
            transfer_idx,
            client_bitfield,
            peer_bitfield: Bitfield::init(num_of_pieces),
            client_is_choked: true,
            peer_is_choked: true,
            client_is_interested: false,
            peer_is_interested: false,
            ongoing_requests: HashSet::new(),
        };
    }
}

#[derive(Debug)]
pub enum P2PTransferError {
    PeerConnFailed
}

// Events that can be received by a p2p transfer task
#[derive(Clone)]
pub enum P2PInboundEvent {
    BlockStored(Block),
    PieceStored(usize),
    SendKeepAlive,
    PeerConnFailed,
}

// Helper enum that models all the possible incoming messages for a p2p transfer task
#[derive(Clone)]
pub enum FunnelMsg {
    InternalEvent(P2PInboundEvent),
    PeerMessage(Message),
    //todo: add TerminationEvent!
}
