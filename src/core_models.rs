//todo: replace all usages of (usize, usize, usize) w/ block

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct BlockPosition {
    pub piece_idx: usize,
    pub offset: usize,
    pub length: usize,
}

#[derive(Clone)]
pub struct TorrentLayout {
    pub pieces: usize,
    pub head_pieces_length: usize,
    pub last_piece_length: usize,
    pub usual_block_length: usize,
    pub head_pieces_last_block_length: usize,
    pub last_piece_last_block_length: usize,
    pub blocks_in_head_pieces: usize,
    pub blocks_in_last_piece: usize,
}

impl TorrentLayout {
    pub fn blocks_in_piece(&self, piece_idx: usize) -> usize {
        return if piece_idx == self.pieces - 1 {
            self.last_piece_length
        } else {
            self.head_pieces_length
        };
    }

    pub fn piece_length(&self, piece_idx: usize) -> usize {
        return if piece_idx == self.pieces - 1 {
            self.last_piece_length
        } else {
            self.head_pieces_length
        };
    }
}