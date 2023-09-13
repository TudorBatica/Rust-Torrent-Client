use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use crate::config;
use crate::dependency_provider::TransferDeps;
use crate::core_models::entities::{Bitfield, Block, Message};
use crate::core_models::events::InternalEvent;
use crate::file_provider::{FileProv};
use crate::piece_picker::{PickResult, PiecePicker};
use crate::core_models::entities::Peer;
use crate::p2p_conn::{PeerReceiver, PeerSender};

const MAX_CLIENT_ONGOING_REQUESTS: usize = 10;

#[derive(Debug)]
pub enum P2PTransferError {}

// Events that can be received by a p2p transfer task
pub enum InboundEvent {
    BlockAcquired(Block),
    PieceAcquired(usize),
}

// Helper enum that models all the possible incoming messages for a p2p transfer task
enum FunnelMsg {
    InternalEvent(InboundEvent),
    PeerMessage(Message),
}

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
    fn new(transfer_idx: usize, client_bitfield: Bitfield, num_of_pieces: usize) -> Self {
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


pub async fn spawn(peer: Peer,
                   transfer_idx: usize,
                   client_bitfield: Bitfield,
                   deps: Arc<dyn TransferDeps>,
) -> (JoinHandle<Result<(), P2PTransferError>>, Sender<InboundEvent>) {
    let (tx_to_self, rx) = mpsc::channel::<InboundEvent>(128);
    let state = P2PState::new(transfer_idx, client_bitfield, deps.torrent_layout().pieces);

    let handle = tokio::spawn(async move {
        return run(peer, deps, state, rx).await;
    });

    return (handle, tx_to_self);
}

async fn run(peer: Peer,
             deps: Arc<dyn TransferDeps>,
             mut state: P2PState,
             rx: Receiver<InboundEvent>) -> Result<(), P2PTransferError> {
    // connect to peer
    let client_id = deps.client_config().client_id;
    let info_hash = deps.info_hash();
    let connector = deps.peer_connector();
    let (read_conn, mut write_conn) = connector.connect_to(peer, info_hash, client_id).await.unwrap();

    // initialize needed dependencies
    let mut output_tx = deps.output_tx();
    let picker = deps.piece_picker();
    let mut file_provider = deps.file_provider();
    file_provider.open_read_only_instance().await;

    // start p2p and internal events listeners + inbound funnel
    let (funnel_tx, mut funnel_rx) = mpsc::channel::<FunnelMsg>(128);
    let _ = tokio::spawn(recv_peer_messages(read_conn, funnel_tx.clone()));
    let _ = tokio::spawn(recv_events(rx, funnel_tx.clone()));

    while let Some(data) = funnel_rx.recv().await {
        match data {
            FunnelMsg::InternalEvent(event) => handle_event(
                event, &mut state, &mut write_conn,
            ).await,
            FunnelMsg::PeerMessage(message) => handle_peer_message(
                message, &mut state, &picker, &mut write_conn, &mut output_tx, &mut file_provider,
            ).await
        };
    }

    return Ok(());
}

async fn recv_peer_messages(mut conn: Box<dyn PeerReceiver>, tx: Sender<FunnelMsg>) {
    loop {
        let message = conn.receive().await;
        let _ = match message {
            Ok(msg) => tx.send(FunnelMsg::PeerMessage(msg)).await.unwrap(),
            Err(err) => {
                println!("Problem occurred in p2p task: {:?} dropping connection...", err);
                //todo: implement connection dropping
                break;
            }
        };
    }
}

async fn recv_events(mut events_rx: Receiver<InboundEvent>, tx: Sender<FunnelMsg>) {
    while let Some(event) = events_rx.recv().await {
        tx.send(FunnelMsg::InternalEvent(event)).await.unwrap();
    }
}

async fn handle_peer_message(message: Message,
                             state: &mut P2PState,
                             picker: &Arc<Mutex<dyn PiecePicker>>,
                             write_conn: &mut Box<dyn PeerSender>,
                             events_tx: &mut Sender<InternalEvent>,
                             file_provider: &mut Box<dyn FileProv>) {
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
            request_blocks(state, picker, write_conn, events_tx).await;
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
            request_blocks(state, picker, write_conn, events_tx).await;
        }
        Message::Bitfield(bitfield) => {
            println!("Received BITFIELD message");
            state.peer_bitfield = Bitfield::new(bitfield);
            update_clients_interested_status(state, write_conn).await;
            request_blocks(state, picker, write_conn, events_tx).await;
        }
        Message::Request(block) => {
            println!("Received REQUEST message: {} {} {}", block.piece_idx, block.offset, block.length);
            if block.length > config::BLOCK_SIZE_BYTES {
                println!("Received a REQUEST message with a length exceeding 16kb!");
                return;
            }
            if state.peer_is_choked || !state.peer_is_interested {
                println!("Received a bad REQUEST message: peer choked: {}, interested: {}", state.peer_is_choked, state.peer_is_interested);
                return;
            }
            let data = file_provider.read_block(&block).await;
            write_conn.send(Message::Piece(block.piece_idx, block.offset, data)).await.unwrap();
        }
        Message::Piece(piece_idx, begin, bytes) => {
            println!("Received PIECE message: {} {} ", piece_idx, begin);
            state.ongoing_requests.remove(&Block::new(piece_idx, begin, bytes.len()));
            let block = Block {
                piece_idx,
                offset: begin,
                length: bytes.len(),
            };
            events_tx.send(InternalEvent::BlockDownloaded(block, bytes)).await.unwrap();
            update_clients_interested_status(state, write_conn).await;
            request_blocks(state, picker, write_conn, events_tx).await;
        }
        Message::Cancel(block) => {
            // the client serves the `REQUEST` messages as soon as it gets them, so nothing
            // needs to be done here
            println!("Received CANCEL message: {:?}", block);
        }
        Message::Port(port) => {
            println!("Received PORT message: {}", port);
        }
    }
}

async fn handle_event(event: InboundEvent, state: &mut P2PState, write_conn: &mut Box<dyn PeerSender>) {
    match event {
        InboundEvent::BlockAcquired(block) => {
            state.ongoing_requests.remove(&block);
            write_conn.send(Message::Cancel(block)).await.unwrap();
        }
        InboundEvent::PieceAcquired(piece_idx) => {
            state.client_bitfield.piece_acquired(piece_idx);
            write_conn.send(Message::Have(piece_idx)).await.unwrap();
            update_clients_interested_status(state, write_conn).await;
        }
    }
}

async fn update_clients_interested_status(state: &mut P2PState, write_conn: &mut Box<dyn PeerSender>) {
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

async fn request_blocks(state: &mut P2PState,
                        picker: &Arc<Mutex<dyn PiecePicker>>,
                        write_conn: &mut Box<dyn PeerSender>,
                        events_tx: &mut Sender<InternalEvent>) {
    let blocks_to_request = MAX_CLIENT_ONGOING_REQUESTS - state.ongoing_requests.len();
    if blocks_to_request < 1 || state.client_is_choked || !state.client_is_interested {
        return;
    }
    let pick_result: Option<PickResult> = {
        let mut picker = picker.lock().await;
        picker.pick(&state.peer_bitfield, blocks_to_request)
    };
    if let Some(pick) = pick_result {
        state.ongoing_requests.extend(pick.blocks.clone().into_iter());
        for block in pick.blocks {
            write_conn.send(Message::Request(block)).await.unwrap();
        }
        if pick.end_game_mode_enabled {
            events_tx.send(InternalEvent::EndGameEnabled).await.unwrap();
        }
    }
}

