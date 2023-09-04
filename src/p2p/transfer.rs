use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::core_models::internal_events::{CoordinatorEvent};
use crate::core_models::entities::{Bitfield, Message};
use crate::core_models::events::InternalEvent;
use crate::p2p::connection::{PeerReceiver, PeerSender};
use crate::state::PeerTransferState;

const MAX_CONCURRENT_REQUESTS: usize = 10;

#[derive(Debug)]
pub enum P2PTransferError {}

enum InboundData {
    CoordinatorEvent(CoordinatorEvent),
    PeerMessage(Message),
}

pub async fn run_transfer(mut state: PeerTransferState,
                          read_conn: Box<dyn PeerReceiver>,
                          write_conn: Box<dyn PeerSender>,
                          events_tx: Sender<InternalEvent>,
                          events_rx: Receiver<CoordinatorEvent>) -> Result<(), P2PTransferError> {
    let (inbound_data_tx, mut inbound_data_rx) = mpsc::channel::<InboundData>(128);
    let _ = tokio::spawn(recv_peer_messages(read_conn, inbound_data_tx.clone()));
    let _ = tokio::spawn(recv_events());

    while let Some(data) = inbound_data_rx.recv().await {
        match data {
            InboundData::CoordinatorEvent(event) => handle_event(),
            InboundData::PeerMessage(message) => handle_peer_messages(message, &mut state)
        };
    }

    return Ok(());
}

async fn recv_peer_messages(mut conn: Box<dyn PeerReceiver>, tx: Sender<InboundData>) {
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


async fn recv_events() {}

fn handle_event() {}

