use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use rand::prelude::IteratorRandom;
use crate::core_models::entities::{Block, TorrentLayout};
use crate::core_models::entities::Bitfield;
use async_trait::async_trait;
use mockall::automock;

const ALL_BLOCKS_IN_TRANSFER_PENALTY: i32 = 100;
const ALL_BLOCKS_REMOVED_PENALTY: i32 = 50000;

#[derive(Clone, Default)]
pub struct PickResult {
    pub blocks: Vec<Block>,
    pub end_game_mode_enabled: bool,
}

struct PieceDownloadState {
    // hashsets contains tuples of (block_offset, block_length)
    blocks_unrequested: HashSet<(usize, usize)>,
    blocks_in_transfer: HashSet<(usize, usize)>,
}

impl PieceDownloadState {
    fn init(piece_idx: usize, layout: &TorrentLayout) -> Self {
        let mut piece = PieceDownloadState {
            blocks_unrequested: HashSet::new(),
            blocks_in_transfer: HashSet::new(),
        };

        let blocks = layout.blocks_in_piece(piece_idx);
        let last_block_len = layout.last_block_length_for_piece(piece_idx);

        for block_idx in 0..blocks {
            let len = if block_idx < blocks - 1 { layout.usual_block_length } else { last_block_len };
            piece.blocks_unrequested.insert((block_idx * layout.usual_block_length, len));
        }

        return piece;
    }

    fn all_blocks_removed(&self) -> bool {
        return self.blocks_in_transfer.is_empty() && self.blocks_unrequested.is_empty();
    }
}

#[automock]
pub trait PiecePicker: Send {
    fn pick(&mut self, peer_bitfield: &Bitfield, num_of_blocks: usize) -> Option<PickResult>;
    fn increase_availability_for_piece(&mut self, piece_idx: usize);
    fn increase_availability_for_pieces(&mut self, piece_idxs: Vec<usize>);
    fn decrease_availability_for_pieces(&mut self, piece_idxs: Vec<usize>);
    fn remove_block(&mut self, block: &Block);
    fn reinsert_piece(&mut self, piece_idx: usize);
}

pub struct RarestPiecePicker {
    layout: TorrentLayout,
    // vec of (piece_idx, inverse_priority)
    priority_sorted_pieces: Vec<(usize, i32)>,
    // key: piece idx, value: index of the piece in the priority sorted pieces vector
    piece_lookup_table: HashMap<usize, usize>,
    // download state for each piece
    piece_download_state: HashMap<usize, PieceDownloadState>,

}

impl RarestPiecePicker {
    pub fn init(layout: TorrentLayout) -> Self {
        let priority_sorted_pieces: Vec<(usize, i32)> = (0..layout.pieces).map(|idx| (idx, 0)).collect();
        let piece_lookup_table: HashMap<usize, usize> = (0..layout.pieces).map(|idx| (idx, idx)).collect();
        let mut piece_download_state: HashMap<usize, PieceDownloadState> = HashMap::new();
        for piece_idx in 0..layout.pieces {
            let piece = PieceDownloadState::init(piece_idx, &layout);
            piece_download_state.insert(piece_idx, piece);
        }

        return RarestPiecePicker { layout, priority_sorted_pieces, piece_lookup_table, piece_download_state };
    }

    fn pick_blocks(&mut self, piece_idx: usize, num_of_blocks: usize) -> PickResult {
        let piece = self.piece_download_state.get_mut(&piece_idx).unwrap();

        let blocks: Vec<(usize, usize)> = piece.blocks_unrequested.iter()
            .take(num_of_blocks)
            .map(|(offset, length)| (*offset, *length))
            .collect();
        blocks.iter().for_each(|block| {
            piece.blocks_unrequested.remove(block);
        });

        if !blocks.is_empty() {
            piece.blocks_in_transfer.extend(blocks.clone());
            return PickResult {
                blocks: blocks.iter()
                    .map(|(offset, length)| Block::new(piece_idx, *offset, *length)).collect(),
                end_game_mode_enabled: false,
            };
        }
        // End Game scenario, at most one in-transfer block will be picked at random
        let blocks: Vec<Block> = piece.blocks_in_transfer.iter()
            .choose(&mut rand::thread_rng())
            .map_or_else(|| vec![],
                         |block| vec![Block::new(piece_idx, block.0, block.1)]);
        return PickResult {
            blocks,
            end_game_mode_enabled: true,
        };
    }

    fn update_priority(&mut self, piece_idx: usize, update: i32) {
        // update inverse priority
        let mut array_idx = self.piece_lookup_table.get(&piece_idx).unwrap().to_owned() as i32;
        self.priority_sorted_pieces[array_idx as usize].1 += update;

        // move the piece to where it now needs to be in the priority sorted array
        let step: i32 = if update > 0 { 1 } else { -1 };
        let condition = |curr: i32, next: i32| if update > 0 { curr <= next } else { curr >= next };
        let array_len = self.priority_sorted_pieces.len();
        let priority = self.priority_sorted_pieces[array_idx as usize].1;
        loop {
            let next_idx = array_idx + step;
            if next_idx >= array_len as i32 || next_idx < 0 {
                break;
            }
            let (next_piece, next_priority) = self.priority_sorted_pieces.get(next_idx as usize).unwrap().to_owned();
            if condition(priority, next_priority) {
                break;
            }
            self.priority_sorted_pieces.swap(array_idx as usize, next_idx as usize);
            self.piece_lookup_table.insert(next_piece, array_idx as usize);
            self.piece_lookup_table.insert(piece_idx, next_idx as usize);
            array_idx = next_idx;
        }
    }

    fn piece_has_unrequested_blocks(&self, piece_idx: usize) -> bool {
        return !self.piece_download_state.get(&piece_idx).unwrap().blocks_unrequested.is_empty();
    }
}

#[async_trait]
impl PiecePicker for RarestPiecePicker {
    // For now, it only chooses from one piece. This needs to be changed
    // vector of (piece_idx, block_offset, block_length) + boolean flag = end-game mode enabled
    fn pick(&mut self, peer_bitfield: &Bitfield, num_of_blocks: usize) -> Option<PickResult> {
        // find the first piece owned by the peer
        let (piece_idx, _) = self.priority_sorted_pieces.iter()
            .find(|(piece_idx, score)| {
                score < &ALL_BLOCKS_REMOVED_PENALTY && peer_bitfield.has_piece(*piece_idx)
            })
            ?.to_owned();

        // pick blocks from it
        let had_unrequested_blocks = self.piece_has_unrequested_blocks(piece_idx);
        let picks = Some(self.pick_blocks(piece_idx, num_of_blocks));

        // if this piece now has all the blocks requested(not acquired), update its priority
        if had_unrequested_blocks && self.piece_has_unrequested_blocks(piece_idx) {
            self.update_priority(piece_idx, ALL_BLOCKS_IN_TRANSFER_PENALTY);
        }

        return picks;
    }

    fn increase_availability_for_piece(&mut self, piece_idx: usize) {
        self.update_priority(piece_idx, 1);
    }

    fn increase_availability_for_pieces(&mut self, piece_idxs: Vec<usize>) {
        for piece_idx in piece_idxs {
            self.increase_availability_for_piece(piece_idx);
        }
    }

    fn decrease_availability_for_pieces(&mut self, piece_idxs: Vec<usize>) {
        for piece_idx in piece_idxs {
            self.update_priority(piece_idx, -1);
        }
    }

    fn remove_block(&mut self, block: &Block) {
        let piece_state = self.piece_download_state.get_mut(&block.piece_idx).unwrap();
        piece_state.blocks_in_transfer.remove(&(block.offset, block.length));
        if piece_state.all_blocks_removed() {
            println!("all blocks removed!!!!");
            self.update_priority(block.piece_idx, ALL_BLOCKS_REMOVED_PENALTY);
        }
    }

    fn reinsert_piece(&mut self, piece_idx: usize) {
        let fresh_state = PieceDownloadState::init(piece_idx, &self.layout);
        self.piece_download_state.insert(piece_idx, fresh_state);
        self.update_priority(piece_idx, -(ALL_BLOCKS_REMOVED_PENALTY + ALL_BLOCKS_IN_TRANSFER_PENALTY));
    }
}


#[cfg(test)]
mod tests {
    use crate::piece_picker::{PiecePicker, RarestPiecePicker};
    use crate::core_models::entities::{Bitfield, Block};
    use crate::{config, mocks};

    #[test]
    fn test_piece_pick_no_pieces_available() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let peer_bitfield = Bitfield::init(2);
        let pick_result = piece_picker.pick(&peer_bitfield, 2);

        assert!(pick_result.is_none());
    }

    #[test]
    fn test_piece_pick() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer_bitfield = Bitfield::init(2);

        peer_bitfield.piece_acquired(0);
        let pick_result = piece_picker.pick(&peer_bitfield, 2).unwrap();

        println!("{:?}", pick_result.blocks);
        assert_eq!(pick_result.blocks.len(), 2);
        assert_eq!(pick_result.blocks[0].piece_idx, pick_result.blocks[1].piece_idx);
        assert_eq!(pick_result.end_game_mode_enabled, false);
    }

    #[test]
    fn test_piece_pick_rarest_picked_first() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer_bitfield = Bitfield::init(2);

        peer_bitfield.piece_acquired(0);
        peer_bitfield.piece_acquired(1);
        piece_picker.increase_availability_for_piece(0);
        let pick_result = piece_picker.pick(&peer_bitfield, 2).unwrap();

        assert_eq!(pick_result.blocks.len(), 2);
        assert_eq!(pick_result.blocks[0].piece_idx, 1);
        assert_eq!(pick_result.blocks[1].piece_idx, 1);
    }

    #[test]
    fn test_piece_pick_end_game_mode() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer_1 = Bitfield::init(2);
        let mut peer_2 = Bitfield::init(2);

        peer_1.piece_acquired(0);
        piece_picker.pick(&peer_1, 2).unwrap();
        peer_2.piece_acquired(1);
        piece_picker.pick(&peer_1, 2).unwrap();

        peer_1.piece_acquired(1);
        let pick_result = piece_picker.pick(&peer_1, 2).unwrap();

        assert_eq!(pick_result.blocks.len(), 1);
        assert_eq!(pick_result.end_game_mode_enabled, true);
    }

    #[test]
    fn test_remove_blocks_in_piece() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer = Bitfield::init(2);

        peer.piece_acquired(0);
        let pick = piece_picker.pick(&peer, 1).unwrap();
        piece_picker.remove_block(&pick.blocks[0]);
        let pick = piece_picker.pick(&peer, 2).unwrap();
        assert_eq!(pick.blocks.len(), 1);
        assert_eq!(pick.end_game_mode_enabled, false);
        piece_picker.remove_block(&pick.blocks[0]);
        let pick = piece_picker.pick(&peer, 2);
        assert!(pick.is_none());
    }

    #[test]
    fn test_reinsert_piece() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer = Bitfield::init(2);

        peer.piece_acquired(0);
        let pick = piece_picker.pick(&peer, 2).unwrap();
        piece_picker.remove_block(&pick.blocks[0]);
        piece_picker.remove_block(&pick.blocks[1]);

        piece_picker.reinsert_piece(0);
        let pick = piece_picker.pick(&peer, 2).unwrap();
        assert_eq!(pick.blocks.len(), 2);
    }
}