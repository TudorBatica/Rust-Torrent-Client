use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::config;
use crate::core_models::entities::{Bitfield, BlockPosition, Message};
use crate::core_models::events::InternalEvent;
use crate::file_provider::FileProvider;
use crate::p2p::connection::{PeerReceiver, PeerSender};
use crate::piece_picker::{PickResult, PiecePicker};
use crate::state::PeerTransferState;

const MAX_CLIENT_ONGOING_REQUESTS: usize = 10;

#[derive(Debug)]
pub enum P2PTransferError {}

// Events that can be received by a p2p transfer task
pub enum InboundEvent {
    BlockAcquired(BlockPosition),
    PieceAcquired(usize),
}

// Helper enum that models all the possible incoming messages
enum InboundData {
    InternalEvent(InboundEvent),
    PeerMessage(Message),
}

pub async fn run(mut state: PeerTransferState,
                 mut file_provider: Box<dyn FileProvider>,
                 read_conn: Box<dyn PeerReceiver>,
                 mut write_conn: Box<dyn PeerSender>,
                 mut events_tx: Sender<InternalEvent>,
                 events_rx: Receiver<InboundEvent>) -> Result<(), P2PTransferError> {
    let (inbound_data_tx, mut inbound_data_rx) = mpsc::channel::<InboundData>(128);
    let _ = tokio::spawn(recv_peer_messages(read_conn, inbound_data_tx.clone()));
    let _ = tokio::spawn(recv_events(events_rx, inbound_data_tx.clone()));

    while let Some(data) = inbound_data_rx.recv().await {
        match data {
            InboundData::InternalEvent(event) => handle_event(
                event, &mut state, &mut write_conn,
            ).await,
            InboundData::PeerMessage(message) => handle_peer_message(
                message, &mut state, &mut write_conn, &mut events_tx, &mut file_provider,
            ).await
        };
    }

    return Ok(());
}

async fn recv_peer_messages(mut conn: Box<dyn PeerReceiver>, tx: Sender<InboundData>) {
    loop {
        let message = conn.receive().await;
        let _ = match message {
            Ok(msg) => tx.send(InboundData::PeerMessage(msg)).await.unwrap(),
            Err(err) => {
                println!("Problem occurred in p2p task: {:?} dropping connection...", err);
                //todo: implement connection dropping
                break;
            }
        };
    }
}

async fn recv_events(mut events_rx: Receiver<InboundEvent>, tx: Sender<InboundData>) {
    while let Some(event) = events_rx.recv().await {
        tx.send(InboundData::InternalEvent(event)).await.unwrap();
    }
}

async fn handle_peer_message(message: Message,
                             state: &mut PeerTransferState,
                             write_conn: &mut Box<dyn PeerSender>,
                             events_tx: &mut Sender<InternalEvent>,
                             file_provider: &mut Box<dyn FileProvider>) {
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
            request_blocks(state, write_conn, events_tx).await;
        }
        Message::Interested => {
            println!("Received INTERESTED message");
            state.peer_is_interested = true;
            if state.peer_is_choked {
                state.peer_is_choked = false;
                write_conn.send(Message::Unchoke).await.unwrap();
            }
        }
        Message::NotInterested => {
            println!("Received NOT INTERESTED message");
            state.peer_is_interested = false;
        }
        Message::Have(piece_idx) => {
            println!("Received HAVE message: {}", piece_idx);
            state.peer_bitfield.piece_acquired(piece_idx);
            update_clients_interested_status(state, write_conn).await;
            request_blocks(state, write_conn, events_tx).await;
        }
        Message::Bitfield(bitfield) => {
            println!("Received BITFIELD message");
            state.peer_bitfield = Bitfield::new(bitfield);
            update_clients_interested_status(state, write_conn).await;
            request_blocks(state, write_conn, events_tx).await;
        }
        Message::Request(piece_idx, begin, length) => {
            println!("Received REQUEST message: {} {} {}", piece_idx, begin, length);
            if length > config::BLOCK_SIZE_BYTES {
                println!("Received a REQUEST message with a length exceeding 16kb!");
                return;
            }
            if state.peer_is_choked || !state.peer_is_interested {
                println!("Received a bad REQUEST message: peer choked: {}, interested: {}", state.peer_is_choked, state.peer_is_interested);
                return;
            }
            let offset = state.torrent_layout.head_pieces_length * piece_idx + begin;
            let data = file_provider.read(offset, length).await;
            write_conn.send(Message::Piece(piece_idx, begin, data)).await.unwrap();
        }
        Message::Piece(piece_idx, begin, bytes) => {
            println!("Received PIECE message: {} {} ", piece_idx, begin);
            state.ongoing_requests.remove(&(piece_idx, begin, bytes.len()));
            let block = BlockPosition {
                piece_idx,
                offset: begin,
                length: bytes.len(),
            };
            events_tx.send(InternalEvent::BlockDownloaded(block, bytes)).await.unwrap();
            update_clients_interested_status(state, write_conn).await;
            request_blocks(state, write_conn, events_tx).await;
        }
        Message::Cancel(piece_idx, begin, length) => {
            // the client serves the `REQUEST` messages as soon as it gets them, so nothing
            // needs to be done here
            println!("Received CANCEL message: {} {} {}", piece_idx, begin, length);
        }
        Message::Port(port) => {
            println!("Received PORT message: {}", port);
        }
    }
}

async fn handle_event(event: InboundEvent, state: &mut PeerTransferState, write_conn: &mut Box<dyn PeerSender>) {
    match event {
        InboundEvent::BlockAcquired(block) => {
            state.ongoing_requests.remove(&(block.piece_idx, block.offset, block.length));
            write_conn.send(Message::Cancel(block.piece_idx, block.offset, block.length)).await.unwrap();
        }
        InboundEvent::PieceAcquired(piece_idx) => {
            state.client_bitfield.piece_acquired(piece_idx);
            write_conn.send(Message::Have(piece_idx)).await.unwrap();
            update_clients_interested_status(state, write_conn).await;
        }
    }
}

async fn update_clients_interested_status(state: &mut PeerTransferState, write_conn: &mut Box<dyn PeerSender>) {
    let peer_has_needed_data = state.peer_bitfield.has_any_missing_pieces_from(&state.client_bitfield);
    if peer_has_needed_data && state.client_is_interested || (!peer_has_needed_data && !state.client_is_interested) {
        return;
    } else if peer_has_needed_data && !state.client_is_interested {
        state.client_is_interested = true;
        write_conn.send(Message::Interested).await.unwrap();
    } else {
        state.client_is_interested = false;
        write_conn.send(Message::NotInterested).await.unwrap()
    }
}

async fn request_blocks(state: &mut PeerTransferState,
                        write_conn: &mut Box<dyn PeerSender>,
                        events_tx: &mut Sender<InternalEvent>) {
    let blocks_to_request = MAX_CLIENT_ONGOING_REQUESTS - state.ongoing_requests.len();
    if blocks_to_request < 1 || state.client_is_choked || !state.client_is_interested {
        return;
    }
    let pick_result: Option<PickResult> = {
        let mut picker = state.piece_picker.lock().await;
        picker.pick(&state.peer_bitfield, blocks_to_request)
    };
    if let Some(pick) = pick_result {
        state.ongoing_requests.extend(pick.blocks.iter());
        for block in pick.blocks {
            write_conn.send(Message::Request(block.0, block.1, block.2)).await.unwrap();
        }
        if pick.end_game_mode_enabled {
            events_tx.send(InternalEvent::EndGameEnabled).await.unwrap();
        }
    }
}


