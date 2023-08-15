use std::collections::{HashMap, HashSet};
use rand::prelude::IteratorRandom;
use crate::transfer::state::Bitfield;

struct PieceDownloadState {
    // hashsets contains tuples of (block_offset, block_length)
    blocks_unrequested: HashSet<(usize, usize)>,
    blocks_in_transfer: HashSet<(usize, usize)>,
}

impl PieceDownloadState {
    fn init(piece_length: usize, block_size: usize) -> Self {
        let mut piece = PieceDownloadState {
            blocks_unrequested: HashSet::new(),
            blocks_in_transfer: HashSet::new(),
        };

        let blocks = (piece_length as f64 / (block_size as f64)).ceil() as usize;
        let last_block_len = piece_length as usize - (blocks - 1) * block_size;
        for block_idx in 0..blocks {
            let len = if block_idx < blocks - 1 { block_size } else { last_block_len };
            piece.blocks_unrequested.insert((block_idx * block_size, len));
        }

        return piece;
    }
}

pub struct PickResult {
    // vector of (piece_idx, block_offset, block_length)
    blocks: Vec<(usize, usize, usize)>,
    end_game_mode_enabled: bool,
}


pub struct PiecePicker {
    // vec of (piece_idx, inverse_priority)
    priority_sorted_pieces: Vec<(usize, i32)>,
    // key: piece idx, value: index of the piece in the priority sorted pieces vector
    piece_lookup_table: HashMap<usize, usize>,
    // download state for each piece
    piece_download_state: HashMap<usize, PieceDownloadState>,

}

impl PiecePicker {
    pub fn init(num_of_pieces: usize, piece_length: usize, last_piece_length: usize, block_size: usize) -> Self {
        let priority_sorted_pieces: Vec<(usize, i32)> = (0..num_of_pieces).map(|idx| (idx, 0)).collect();
        let piece_lookup_table: HashMap<usize, usize> = (0..num_of_pieces).map(|idx| (idx, idx)).collect();
        let mut piece_download_state: HashMap<usize, PieceDownloadState> = HashMap::new();
        for piece_idx in 0..num_of_pieces {
            let piece_size = if piece_idx < num_of_pieces - 1 { piece_length } else { last_piece_length };
            let piece = PieceDownloadState::init(piece_size, block_size);
            piece_download_state.insert(piece_idx, piece);
        }

        return PiecePicker { priority_sorted_pieces, piece_lookup_table, piece_download_state };
    }

    // For now, it only chooses from one piece. This needs to be changed
    // vector of (piece_idx, block_offset, block_length) + boolean flag = end-game mode enabled
    pub fn pick(&mut self, peer_bitfield: &Bitfield, num_of_blocks: usize) -> Option<PickResult> {
        // find the first piece owned by the peer
        let (piece_idx, _) = self.priority_sorted_pieces.iter()
            .find(|(piece_idx, _)| peer_bitfield.has_piece(*piece_idx))?
            .to_owned();

        // pick blocks from it
        let had_unrequested_blocks = self.piece_has_unrequested_blocks(piece_idx);
        let picks = Some(self.pick_blocks(piece_idx, num_of_blocks));

        // if this piece now has all the blocks requested(not acquired), update its priority
        if had_unrequested_blocks && self.piece_has_unrequested_blocks(piece_idx) {
            self.update_priority(piece_idx, 100);
        }

        return picks;
    }

    pub fn increase_availability_for_piece(&mut self, piece_idx: usize) {
        self.update_priority(piece_idx, 1);
    }

    pub fn increase_availability_for_pieces(&mut self, piece_idxs: Vec<usize>) {
        for piece_idx in piece_idxs {
            self.increase_availability_for_piece(piece_idx);
        }
    }

    pub fn decrease_availability_for_pieces(&mut self, piece_idxs: Vec<usize>) {
        for piece_idx in piece_idxs {
            self.update_priority(piece_idx, -1);
        }
    }

    fn pick_blocks(&mut self, piece_idx: usize, num_of_blocks: usize) -> PickResult {
        let piece = self.piece_download_state.get_mut(&piece_idx).unwrap();
        let blocks: Vec<(usize, usize)> = piece.blocks_unrequested.drain().take(num_of_blocks).collect();
        if !blocks.is_empty() {
            piece.blocks_in_transfer.extend(blocks.clone());
            return PickResult {
                blocks: blocks.iter().map(|(offset, length)| (piece_idx, *offset, *length)).collect(),
                end_game_mode_enabled: false,
            };
        }
        let block = piece.blocks_in_transfer.iter().choose(&mut rand::thread_rng()).unwrap();
        return PickResult {
            blocks: vec![(piece_idx, block.0, block.1)],
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

#[cfg(test)]
mod tests {
    use crate::transfer::piece_picker::PiecePicker;
    use crate::transfer::state::Bitfield;

    #[test]
    fn test_picker_init() {
        let piece_picker = PiecePicker::init(5, 16384, 8192, 16);

        assert_eq!(piece_picker.priority_sorted_pieces.len(), 5);
        assert_eq!(piece_picker.piece_lookup_table.len(), 5);
        assert_eq!(piece_picker.piece_download_state.len(), 5);
    }

    #[test]
    fn test_pick_no_blocks_available() {
        let mut piece_picker = PiecePicker::init(3, 8192, 4096, 16);
        let peer_bitfield = Bitfield::init(2); // initialize an all 0 bitfield
        let pick_result = piece_picker.pick(&peer_bitfield, 2);

        assert!(pick_result.is_none());
    }

    #[test]
    fn test_pick_some_blocks_available() {
        let mut piece_picker = PiecePicker::init(3, 8192, 4096, 16);
        let mut peer_bitfield = Bitfield::init(3);
        peer_bitfield.piece_acquired(0);
        let pick_result = piece_picker.pick(&peer_bitfield, 10).unwrap();

        assert_eq!(pick_result.end_game_mode_enabled, false);
        assert_eq!(pick_result.blocks.len(), 10);
        assert_eq!(pick_result.blocks[0].0, 0);
    }

    fn test_pick_end_game_mode() {
        let mut piece_picker = PiecePicker::init(3, 8192, 4096, 16);

        let mut peer_bitfield = Bitfield::init(3);
        peer_bitfield.piece_acquired(0);
        peer_bitfield.piece_acquired(1);
        let pick_result = piece_picker.pick(&peer_bitfield, 2);

        assert!(pick_result.is_some());
        let pick_result = pick_result.unwrap();
        assert_eq!(pick_result.blocks.len(), 1);
        assert!(pick_result.end_game_mode_enabled);
    }
}