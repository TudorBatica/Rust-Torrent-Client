use std::collections::{HashMap, HashSet};
use rand::prelude::IteratorRandom;
use crate::core_models::entities::{Block, TorrentLayout};
use crate::core_models::entities::Bitfield;
use mockall::automock;

// Penalty applied to pieces with all blocks picked
const ALL_BLOCKS_PICKED_PENALTY: i32 = 5000;
// Penalty applied to pieces with all the blocks removed
const ALL_BLOCKS_REMOVED_PENALTY: i32 = 50000;
// Starting score for all pieces
const PIECE_BASE_SCORE: i32 = 1000;
// Bonus applied to pieces with some blocks picked; used to prioritize piece completion
const SOME_BLOCKS_PICKED_BONUS: i32 = -1000;

enum PickEvent {
    FirstPickedBlocks,
    AllBlocksPicked,
}

struct PickResult {
    picked_blocks: Vec<Block>,
    pick_event: Option<PickEvent>,
}

struct PieceDownloadState {
    // hashsets contains tuples of (block_offset, block_length)
    blocks_unpicked: HashSet<(usize, usize)>,
    blocks_picked: HashSet<(usize, usize)>,
    had_blocks_picked_from_it: bool,
}

impl PieceDownloadState {
    fn init(piece_idx: usize, layout: &TorrentLayout) -> Self {
        let mut piece = PieceDownloadState {
            blocks_unpicked: HashSet::new(),
            blocks_picked: HashSet::new(),
            had_blocks_picked_from_it: false,
        };

        let blocks = layout.blocks_in_piece(piece_idx);
        let last_block_len = layout.last_block_length_for_piece(piece_idx);

        for block_idx in 0..blocks {
            let len = if block_idx < blocks - 1 { layout.usual_block_length } else { last_block_len };
            piece.blocks_unpicked.insert((block_idx * layout.usual_block_length, len));
        }

        return piece;
    }

    fn all_blocks_removed(&self) -> bool {
        return self.blocks_picked.is_empty() && self.blocks_unpicked.is_empty();
    }
}

#[automock]
pub trait PiecePicker: Send {
    fn pick(&mut self, peer_bitfield: &Bitfield, num_of_blocks: usize) -> Vec<Block>;
    fn increase_availability_for_piece(&mut self, piece_idx: usize);
    fn increase_availability_for_pieces(&mut self, piece_idxs: Vec<usize>);
    fn decrease_availability_for_pieces(&mut self, piece_idxs: Vec<usize>);
    fn remove_block(&mut self, block: &Block);
    fn reinsert_piece(&mut self, piece_idx: usize);
}

pub struct RarestPiecePicker {
    layout: TorrentLayout,
    // vec of (piece_idx, inverse_priority)
    priority_score_sorted_pieces: Vec<(usize, i32)>,
    // key: piece idx, value: index of the piece in the priority sorted pieces vector
    piece_lookup_table: HashMap<usize, usize>,
    // download state for each piece
    piece_download_state: HashMap<usize, PieceDownloadState>,

}

impl RarestPiecePicker {
    pub fn init(layout: TorrentLayout) -> Self {
        let priority_sorted_pieces: Vec<(usize, i32)> = (0..layout.pieces).map(|idx| (idx, PIECE_BASE_SCORE)).collect();
        let piece_lookup_table: HashMap<usize, usize> = (0..layout.pieces).map(|idx| (idx, idx)).collect();
        let mut piece_download_state: HashMap<usize, PieceDownloadState> = HashMap::new();
        for piece_idx in 0..layout.pieces {
            let piece = PieceDownloadState::init(piece_idx, &layout);
            piece_download_state.insert(piece_idx, piece);
        }

        return RarestPiecePicker { layout, priority_score_sorted_pieces: priority_sorted_pieces, piece_lookup_table, piece_download_state };
    }

    fn pick_blocks(&mut self, piece_idx: usize, num_of_blocks: usize) -> PickResult {
        let piece = self.piece_download_state.get_mut(&piece_idx).unwrap();

        // when a piece has no unpicked blocks, we re-pick an already picked block at random
        if piece.blocks_unpicked.is_empty() {
            let block = piece.blocks_picked.iter()
                .choose(&mut rand::thread_rng())
                .unwrap();
            return PickResult { picked_blocks: vec![Block::new(piece_idx, block.0, block.1)], pick_event: None };
        }

        // for pieces with unpicked blocks, pick at most num_of_blocks
        let blocks: Vec<(usize, usize)> = piece.blocks_unpicked.iter()
            .take(num_of_blocks)
            .map(|(offset, length)| (*offset, *length))
            .collect();
        blocks.iter().for_each(|block| {
            piece.blocks_unpicked.remove(block);
            piece.blocks_picked.insert(*block);
        });

        let blocks = blocks.iter()
            .map(|(offset, length)| Block::new(piece_idx, *offset, *length))
            .collect();

        if !piece.had_blocks_picked_from_it {
            piece.had_blocks_picked_from_it = true;
            return PickResult { picked_blocks: blocks, pick_event: Some(PickEvent::FirstPickedBlocks) };
        }

        if piece.blocks_unpicked.is_empty() {
            return PickResult { picked_blocks: blocks, pick_event: Some(PickEvent::AllBlocksPicked) };
        }

        return PickResult { picked_blocks: blocks, pick_event: None };
    }

    fn update_priority(&mut self, piece_idx: usize, update: i32) {
        // update inverse priority
        let mut array_idx = self.piece_lookup_table.get(&piece_idx).unwrap().to_owned() as i32;
        self.priority_score_sorted_pieces[array_idx as usize].1 += update;

        // move the piece to where it now needs to be in the priority sorted array
        let step: i32 = if update > 0 { 1 } else { -1 };
        let condition = |curr: i32, next: i32| if update > 0 { curr <= next } else { curr >= next };
        let array_len = self.priority_score_sorted_pieces.len();
        let priority = self.priority_score_sorted_pieces[array_idx as usize].1;
        loop {
            let next_idx = array_idx + step;
            if next_idx >= array_len as i32 || next_idx < 0 {
                break;
            }
            let (next_piece, next_priority) = self.priority_score_sorted_pieces.get(next_idx as usize).unwrap().to_owned();
            if condition(priority, next_priority) {
                break;
            }
            self.priority_score_sorted_pieces.swap(array_idx as usize, next_idx as usize);
            self.piece_lookup_table.insert(next_piece, array_idx as usize);
            self.piece_lookup_table.insert(piece_idx, next_idx as usize);
            array_idx = next_idx;
        }
    }
}

impl PiecePicker for RarestPiecePicker {
    fn pick(&mut self, peer_bitfield: &Bitfield, num_of_blocks: usize) -> Vec<Block> {
        // find the first owned piece with available blocks
        let piece_idx = self.priority_score_sorted_pieces
            .iter()
            .find(|(piece_idx, score)| {
                score < &ALL_BLOCKS_REMOVED_PENALTY && peer_bitfield.has_piece(*piece_idx)
            })
            .map(|(piece_idx, _score)| *piece_idx);

        let blocks = piece_idx.map_or(vec![], |piece_idx| {
            let pick_result = self.pick_blocks(piece_idx, num_of_blocks);
            if let Some(event) = pick_result.pick_event {
                match event {
                    PickEvent::FirstPickedBlocks => {
                        println!("Piece Picker :: started picking from new piece {}", piece_idx);
                        self.update_priority(piece_idx, SOME_BLOCKS_PICKED_BONUS)
                    },
                    PickEvent::AllBlocksPicked => self.update_priority(piece_idx, ALL_BLOCKS_PICKED_PENALTY - SOME_BLOCKS_PICKED_BONUS),
                }
            }
            pick_result.picked_blocks
        });

        return blocks;
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
        piece_state.blocks_picked.remove(&(block.offset, block.length));
        if piece_state.all_blocks_removed() {
            self.update_priority(block.piece_idx, ALL_BLOCKS_REMOVED_PENALTY);
        }
    }

    fn reinsert_piece(&mut self, piece_idx: usize) {
        let fresh_state = PieceDownloadState::init(piece_idx, &self.layout);
        self.piece_download_state.insert(piece_idx, fresh_state);
        self.update_priority(piece_idx, -ALL_BLOCKS_REMOVED_PENALTY + PIECE_BASE_SCORE);
    }
}


#[cfg(test)]
mod tests {
    use crate::piece_picker::{PiecePicker, RarestPiecePicker};
    use crate::core_models::entities::{Bitfield};
    use crate::{mocks};

    #[test]
    fn test_piece_pick_no_pieces_available() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let peer_bitfield = Bitfield::init(2);
        let blocks = piece_picker.pick(&peer_bitfield, 2);

        assert!(blocks.is_empty());
    }

    #[test]
    fn test_piece_pick() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer_bitfield = Bitfield::init(2);

        peer_bitfield.piece_acquired(0);
        let blocks = piece_picker.pick(&peer_bitfield, 2);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].piece_idx, blocks[1].piece_idx);
    }

    #[test]
    fn test_pick_from_already_picked_blocks() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer_bitfield = Bitfield::init(2);

        peer_bitfield.piece_acquired(0);
        piece_picker.pick(&peer_bitfield, 2);

        let blocks = piece_picker.pick(&peer_bitfield, 2);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].piece_idx, 0);
    }

    #[test]
    fn test_rarest_piece_prioritized() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer_bitfield = Bitfield::init(2);

        peer_bitfield.piece_acquired(0);
        peer_bitfield.piece_acquired(1);
        piece_picker.increase_availability_for_piece(0);
        let blocks = piece_picker.pick(&peer_bitfield, 2);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].piece_idx, 1);
        assert_eq!(blocks[1].piece_idx, 1);
    }

    #[test]
    fn test_piece_with_some_picked_blocks_prioritized() {
        let layout = mocks::generate_mock_layout(2, 4, 4);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer_bitfield = Bitfield::init(2);

        peer_bitfield.piece_acquired(0);
        peer_bitfield.piece_acquired(1);
        // pick blocks from either piece
        let piece_idx = piece_picker.pick(&peer_bitfield, 2)[0].piece_idx;

        // increase availability for the previously picked piece
        piece_picker.increase_availability_for_piece(piece_idx);

        // on the second pick, the previously picked piece should be again picked, even though it is less rare
        let blocks = piece_picker.pick(&peer_bitfield, 2);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].piece_idx, piece_idx);
        assert_eq!(blocks[1].piece_idx, piece_idx);
    }

    #[test]
    fn test_remove_blocks_in_piece() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer = Bitfield::init(2);

        peer.piece_acquired(0);
        let blocks = piece_picker.pick(&peer, 2);
        blocks.iter().for_each(|block| piece_picker.remove_block(block));

        let blocks = piece_picker.pick(&peer, 2);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_reinsert_piece() {
        let layout = mocks::generate_mock_layout(2, 2, 2);
        let mut piece_picker = RarestPiecePicker::init(layout);
        let mut peer = Bitfield::init(2);

        peer.piece_acquired(0);
        let blocks = piece_picker.pick(&peer, 2);
        piece_picker.remove_block(&blocks[0]);
        piece_picker.remove_block(&blocks[1]);

        piece_picker.reinsert_piece(0);
        let blocks = piece_picker.pick(&peer, 2);
        assert_eq!(blocks.len(), 2);
    }
}