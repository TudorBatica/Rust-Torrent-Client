use std::sync::Arc;
use tokio::sync::Mutex;
use crate::config;
use crate::core_models::entities::{Bitfield, DataBlock, Message};
use crate::core_models::events::InternalEvent;
use crate::file_provider::FileProv;
use crate::p2p::state::{FunnelMsg, P2PInboundEvent, P2PState};
use crate::piece_picker::{PiecePicker};

const MAX_CLIENT_ONGOING_REQUESTS: usize = 5;

pub struct HandlerResult {
    pub internal_events: Vec<InternalEvent>,
    pub messages_for_peer: Vec<Message>,
}

impl HandlerResult {
    pub fn new() -> Self {
        return HandlerResult {
            internal_events: Vec::new(),
            messages_for_peer: Vec::new(),
        };
    }
    pub fn event(&mut self, event: InternalEvent) {
        self.internal_events.push(event);
    }
    pub fn msg(&mut self, message: Message) {
        self.messages_for_peer.push(message);
    }
}

pub async fn handle(msg: FunnelMsg, state: &mut P2PState, fp: &mut Box<dyn FileProv>, picker: &Arc<Mutex<dyn PiecePicker>>) -> HandlerResult {
    return match msg {
        FunnelMsg::InternalEvent(event) => handle_event(event, state).await,
        FunnelMsg::PeerMessage(message) => handle_peer_message(message, state, fp, picker).await
    };
}

async fn handle_event(event: P2PInboundEvent, state: &mut P2PState) -> HandlerResult {
    let mut result = HandlerResult::new();

    match event {
        P2PInboundEvent::BlockStored(block) => {
            if state.ongoing_requests.remove(&block) {
                result.msg(Message::Cancel(block));
            }
        }
        P2PInboundEvent::PieceStored(piece_idx) => {
            state.client_bitfield.piece_acquired(piece_idx);
            update_clients_interested_status(state, &mut result);
        }
        P2PInboundEvent::SendKeepAlive => {
            result.msg(Message::KeepAlive);
        }
        P2PInboundEvent::PeerConnFailed => {}
    };

    return result;
}

async fn handle_peer_message(message: Message, state: &mut P2PState, fp: &mut Box<dyn FileProv>, picker: &Arc<Mutex<dyn PiecePicker>>) -> HandlerResult {
    let mut result = HandlerResult::new();

    match message {
        Message::KeepAlive => {}
        Message::Choke => {
            state.client_is_choked = true;
        }
        Message::Unchoke => {
            state.client_is_choked = false;
            pick_blocks(state, &mut result, &picker).await;
        }
        Message::Interested => {
            state.peer_is_interested = true;
        }
        Message::NotInterested => {
            state.peer_is_interested = false;
        }
        Message::Have(piece_idx) => {
            {
                let mut picker = picker.lock().await;
                picker.increase_availability_for_piece(piece_idx);
            }
            state.peer_bitfield.piece_acquired(piece_idx);
            update_clients_interested_status(state, &mut result);
            pick_blocks(state, &mut result, &picker).await;
        }
        Message::Bitfield(bitfield_vec) => {
            state.peer_bitfield = Bitfield::new(bitfield_vec);
            {
                let mut picker = picker.lock().await;
                picker.increase_availability_for_pieces(state.peer_bitfield.to_available_pieces_vec());
            }
            update_clients_interested_status(state, &mut result);
            pick_blocks(state, &mut result, &picker).await;
        }
        Message::Request(block) => {
            if block.length > config::BLOCK_SIZE_BYTES {
                println!("Received a REQUEST message with a length exceeding 16kb!");
            } else if state.peer_is_choked || !state.peer_is_interested {
                println!("Received a bad REQUEST message: peer choked: {}, interested: {}", state.peer_is_choked, state.peer_is_interested);
            } else if !state.client_bitfield.has_piece(block.piece_idx) {
                println!("Received a REQUEST message for a piece {} which is not currently owned! ", block.piece_idx);
            } else {
                let data = fp.read_block(&block).await;
                result.msg(Message::Piece(DataBlock::new(block.piece_idx, block.offset, data)));
            }
        }
        Message::Piece(data_block) => {
            let block = data_block.to_block();
            state.ongoing_requests.remove(&block);
            result.event(InternalEvent::BlockDownloaded(data_block));
            update_clients_interested_status(state, &mut result);
            pick_blocks(state, &mut result, &picker).await;
        }
        Message::Cancel(_) => {
            // the client serves the `REQUEST` messages as soon as it gets them, so nothing
            // needs to be done here
        }
        Message::Port(_) => {}
    };

    return result;
}

fn update_clients_interested_status(state: &mut P2PState, result: &mut HandlerResult) {
    let peer_has_needed_data = state.peer_bitfield.has_any_missing_pieces_from(&state.client_bitfield);
    if peer_has_needed_data && !state.client_is_interested {
        state.client_is_interested = true;
        result.msg(Message::Interested);
    } else if !peer_has_needed_data && state.client_is_interested {
        state.client_is_interested = false;
        result.msg(Message::NotInterested);
    }
}

async fn pick_blocks(state: &mut P2PState, result: &mut HandlerResult, picker: &Arc<Mutex<dyn PiecePicker>>) {
    let blocks_to_request = MAX_CLIENT_ONGOING_REQUESTS - state.ongoing_requests.len();
    if blocks_to_request < 1 || state.client_is_choked || !state.client_is_interested {
        return;
    }
    let blocks = {
        let mut picker = picker.lock().await;
        picker.pick(&state.peer_bitfield, blocks_to_request)
    };

    state.ongoing_requests.extend(blocks.clone().into_iter());
    blocks.into_iter().for_each(|block| result.msg(Message::Request(block)));
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Arc};
    use tokio::sync::Mutex;
    use crate::config;
    use crate::core_models::entities::{Bitfield, Block, DataBlock, Message};
    use crate::file_provider::{FileProv, MockFileProv};
    use crate::p2p::handlers::{handle, HandlerResult, pick_blocks, update_clients_interested_status};
    use crate::p2p::state::P2PState;
    use crate::p2p::state::FunnelMsg::PeerMessage;
    use crate::piece_picker::{MockPiecePicker, PiecePicker};

    #[test]
    fn client_interested_status_update_when_uninterested_and_peer_has_needed_data_test() {
        let mut result = HandlerResult::new();
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.client_is_interested = false;
        state.peer_bitfield.piece_acquired(0);

        update_clients_interested_status(&mut state, &mut result);

        assert!(state.client_is_interested);
        assert!(result.internal_events.is_empty());
        assert_eq!(result.messages_for_peer.len(), 1);
        assert!(result.messages_for_peer[0].is_interested())
    }

    #[test]
    fn client_interested_status_update_when_uninterested_and_peer_has_no_needed_data_test() {
        let mut result = HandlerResult::new();
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.client_is_interested = false;

        update_clients_interested_status(&mut state, &mut result);

        assert!(!state.client_is_interested);
        assert!(result.internal_events.is_empty());
        assert!(result.messages_for_peer.is_empty());
    }

    #[test]
    fn client_interested_status_update_when_interested_and_peer_has_needed_data_test() {
        let mut result = HandlerResult::new();
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.client_is_interested = true;
        state.peer_bitfield.piece_acquired(0);

        update_clients_interested_status(&mut state, &mut result);

        assert!(state.client_is_interested);
        assert!(result.internal_events.is_empty());
        assert!(result.messages_for_peer.is_empty());
    }

    #[test]
    fn client_interested_status_update_when_interested_and_peer_has_no_needed_data_test() {
        let mut result = HandlerResult::new();
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.client_is_interested = true;

        update_clients_interested_status(&mut state, &mut result);

        assert!(!state.client_is_interested);
        assert!(result.internal_events.is_empty());
        assert_eq!(result.messages_for_peer.len(), 1);
        assert!(result.messages_for_peer[0].is_not_interested());
    }

    #[tokio::test]
    async fn pick_blocks_test() {
        let mut result = HandlerResult::new();
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.client_is_interested = true;
        state.client_is_choked = false;
        state.ongoing_requests = HashSet::new();

        let picked_blocks = vec![Block::new(0, 0, 0)];
        let picked = picked_blocks.clone();
        let mut picker = MockPiecePicker::new();
        picker.expect_pick().returning(move |_, _| picked_blocks.clone());
        let picker = Arc::new(Mutex::new(picker));

        pick_blocks(&mut state, &mut result, &(picker as Arc<Mutex<dyn PiecePicker>>)).await;

        assert!(result.internal_events.is_empty());
        assert_eq!(state.ongoing_requests.len(), picked.len());
        assert_eq!(result.messages_for_peer.len(), picked.len());
        assert!(result.messages_for_peer.iter().all(|msg| msg.is_request()));
    }

    #[tokio::test]
    async fn pick_blocks_when_client_choked_test() {
        let mut result = HandlerResult::new();
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.client_is_choked = true;
        state.client_is_interested = true;
        let initial_state = state.clone();
        let picker = Arc::new(Mutex::new(MockPiecePicker::new()));

        pick_blocks(&mut state, &mut result, &(picker as Arc<Mutex<dyn PiecePicker>>)).await;

        assert!(result.internal_events.is_empty());
        assert!(result.messages_for_peer.is_empty());
        assert_eq!(state, initial_state);
    }

    #[tokio::test]
    async fn pick_blocks_when_client_not_interested_test() {
        let mut result = HandlerResult::new();
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.client_is_choked = false;
        state.client_is_interested = false;
        let initial_state = state.clone();
        let picker = Arc::new(Mutex::new(MockPiecePicker::new()));

        pick_blocks(&mut state, &mut result, &(picker as Arc<Mutex<dyn PiecePicker>>)).await;

        assert!(result.internal_events.is_empty());
        assert!(result.messages_for_peer.is_empty());
        assert_eq!(state, initial_state);
    }

    #[tokio::test]
    async fn handle_choke_message_test() {
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        let (picker, mut fp) = prepare_mocks();
        state.client_is_choked = false;

        let msg = PeerMessage(Message::Choke);
        let _result = handle(msg, &mut state, &mut fp, &picker).await;

        assert!(state.client_is_choked);
    }

    #[tokio::test]
    async fn handle_unchoke_message_test() {
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        let (picker, mut fp) = prepare_mocks();
        state.client_is_choked = true;

        let msg = PeerMessage(Message::Unchoke);
        let _result = handle(msg, &mut state, &mut fp, &picker).await;

        assert!(!state.client_is_choked);
    }

    #[tokio::test]
    async fn handle_interested_message_test() {
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        let (picker, mut fp) = prepare_mocks();
        state.peer_is_interested = false;

        let msg = PeerMessage(Message::Interested);
        let _result = handle(msg, &mut state, &mut fp, &picker).await;

        assert!(state.peer_is_interested);
    }

    #[tokio::test]
    async fn handle_not_interested_message_test() {
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        let (picker, mut fp) = prepare_mocks();
        state.peer_is_interested = true;

        let msg = PeerMessage(Message::NotInterested);
        let _result = handle(msg, &mut state, &mut fp, &picker).await;

        assert!(!state.peer_is_interested);
    }

    #[tokio::test]
    async fn handle_have_message_test() {
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        let (picker, mut fp) = prepare_mocks();

        let msg = PeerMessage(Message::Have(2));
        let _result = handle(msg, &mut state, &mut fp, &picker).await;

        assert!(state.peer_bitfield.has_piece(2));
    }

    #[tokio::test]
    async fn handle_bitfield_message_test() {
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        let (picker, mut fp) = prepare_mocks();

        let msg = PeerMessage(Message::Bitfield(vec![1]));
        let _result = handle(msg, &mut state, &mut fp, &picker).await;

        assert_eq!(state.peer_bitfield.content, vec![1]);
    }

    #[tokio::test]
    async fn handle_request_message_test() {
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.peer_is_choked = false;
        state.peer_is_interested = true;
        state.client_bitfield.piece_acquired(0);
        let (picker, mut fp) = prepare_mocks();

        let msg = PeerMessage(Message::Request(Block::new(0, 0, config::BLOCK_SIZE_BYTES)));
        let result = handle(msg, &mut state, &mut fp, &picker).await;

        assert!(result.messages_for_peer.iter().any(|msg| msg.is_piece()));
    }

    #[tokio::test]
    async fn handle_piece_message_test() {
        let mut state = P2PState::new(0, Bitfield::init(5), 5);
        state.peer_is_choked = false;
        state.peer_is_interested = true;
        state.client_bitfield.piece_acquired(0);
        state.ongoing_requests.insert(Block::new(0, 0, 0));
        let (picker, mut fp) = prepare_mocks();

        let msg = PeerMessage(Message::Piece(DataBlock::new(0, 0, vec![])));
        let result = handle(msg, &mut state, &mut fp, &picker).await;

        assert!(state.ongoing_requests.is_empty());
        assert!(result.internal_events.iter().any(|msg| msg.is_block_downloaded()));
    }

    fn prepare_mocks() -> (Arc<Mutex<dyn PiecePicker>>, Box<dyn FileProv>) {
        let mut picker = MockPiecePicker::new();
        picker.expect_increase_availability_for_pieces().returning(|_| ());
        picker.expect_pick().returning(move |_, _| vec![]);
        picker.expect_increase_availability_for_piece().returning(|_| ());

        let mut fp = MockFileProv::new();
        fp.expect_read_block().returning(|_| vec![]);

        return (Arc::new(Mutex::new(picker)), Box::new(fp));
    }
}
