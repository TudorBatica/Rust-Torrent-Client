use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::core_models::internal_events::{CoordinatorEvent, CoordinatorInput};
use crate::core_models::entities::{Bitfield, Message};
use crate::core_models::events::InternalEvent;
use crate::p2p::connection::{P2PConnError, PeerReadConn, PeerWriteConn};
use crate::state::PeerTransferState;

const MAX_CONCURRENT_REQUESTS: usize = 10;

#[derive(Debug)]
pub enum P2PTransferError {}

enum InboundData {
    CoordinatorEvent(CoordinatorEvent),
    PeerMessage(Message),
}

pub async fn run_transfer(mut state: PeerTransferState,
                          read_conn: PeerReadConn,
                          write_conn: PeerWriteConn,
                          events_tx: Sender<InternalEvent>,
                          events_rx: Receiver<CoordinatorEvent>) -> Result<(), P2PTransferError> {
    let (inbound_data_tx, mut inbound_data_rx) = mpsc::channel::<InboundData>(128);
    let _ = tokio::spawn(receive_peer_messages(read_conn, inbound_data_tx.clone()));
    let _ = tokio::spawn(receive_events_from_coordinator());

    while let Some(data) = inbound_data_rx.recv().await {
        match data {
            InboundData::CoordinatorEvent(event) => handle_internal_event(),
            InboundData::PeerMessage(message) => handle_peer_messages(message, &mut state)
        };
    }

    return Ok(());
}

async fn receive_peer_messages(mut conn: PeerReadConn, tx: Sender<InboundData>) {
    loop {
        let message = conn.receive().await;
        let _ = match message {
            Ok(msg) => tx.send(InboundData::PeerMessage(msg)).await,
            Err(err) => {
                println!("Problem occurred in p2p task: {:?} dropping connection...", err);
                //todo: implement connection dropping
                break;
            }
        };
    }
}

fn handle_peer_messages(message: Message, state: &mut PeerTransferState) {
    match message {
        Message::KeepAlive => {
            println!("Received KEEP ALIVE message");
        }
        Message::Choke => {
            println!("Received CHOKE message");
            state.peer_is_choked = true;
        }
        Message::Unchoke => {
            println!("Received UNCHOKE message");
            state.peer_is_choked = false;
        }
        Message::Interested => {
            println!("Received INTERESTED message");
            state.peer_is_interested = true;
        }
        Message::NotInterested => {
            println!("Received NOT INTERESTED message");
            state.peer_is_interested = false;
        }
        Message::Have(piece_idx) => {
            println!("Received HAVE message: {}", piece_idx);
            state.peer_bitfield.piece_acquired(piece_idx);
        }
        Message::Bitfield(bitfield) => {
            println!("Received BITFIELD message");
            state.peer_bitfield = Bitfield::new(bitfield);
        }
        Message::Request(piece_idx, begin, length) => {
            println!("Received REQUEST message: {} {} {}", piece_idx, begin, length);
        }
        Message::Piece(piece_idx, begin, bytes) => {
            println!("Received PIECE message: {} {} ", piece_idx, begin);
        }
        Message::Cancel(piece_idx, begin, length) => {
            println!("Received CANCEL message: {} {} {}", piece_idx, begin, length);
        }
        Message::Port(port) => {
            println!("Received PORT message: {}", port);
        }
    }
}

// BITFIELD + HAVE
async fn handle_peer_bitfield_update() {
    // if client not interested, try to select a piece
    // if piece found, update interested status
    // request blocks
}

async fn handle_unchoke() {
    // request blocks
}

async fn handle_block() {
    // if piece not complete, request more blocks
    // if piece complete, try to select another one
    // if no other piece found, update to not interested
}

async fn update_interested(state: &mut PeerTransferState, write_conn: &mut PeerWriteConn, client_interested: bool) -> Result<(), P2PConnError> {
    state.client_is_interested = client_interested;
    if client_interested {
        return write_conn.send(Message::Interested).await;
    } else {
        return write_conn.send(Message::NotInterested).await;
    }
}

// async fn request_blocks(state: &mut PeerTransferState) -> Result<(), P2PConnError> {
//     let pieces = state.pieces_to_download.lock().await;
//     // pieces.get()
// }

// async fn peer_is_interesting(state: &mut PeerTransferState) -> bool {
//     if state.piece_in_download.is_some() || !state.ongoing_requests.is_empty() {
//         return true;
//     }
//     let pieces_to_download = state.pieces_to_download.lock().await;
//     return pieces_to_download.keys()
//         .find(|piece_idx| { state.peer_bitfield.has_piece(**piece_idx) })
//         .is_some();
// }

async fn receive_events_from_coordinator() {}

fn handle_internal_event() {}

