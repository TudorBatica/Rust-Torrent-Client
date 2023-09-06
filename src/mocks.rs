use sha1::{Digest, Sha1};
use crate::config;
use crate::core_models::entities::{Block, TorrentLayout};

pub fn generate_mock_layout(num_of_pieces: usize, blocks_in_head_pieces: usize, blocks_in_last_piece: usize) -> TorrentLayout {
    let piece_len = config::BLOCK_SIZE_BYTES * blocks_in_head_pieces;
    let last_piece_len = if num_of_pieces > 1 { config::BLOCK_SIZE_BYTES * blocks_in_last_piece } else { piece_len };
    return TorrentLayout {
        pieces: num_of_pieces,
        head_pieces_length: piece_len,
        last_piece_length: last_piece_len,
        usual_block_length: config::BLOCK_SIZE_BYTES,
        head_pieces_last_block_length: config::BLOCK_SIZE_BYTES,
        last_piece_last_block_length: config::BLOCK_SIZE_BYTES,
        blocks_in_head_pieces,
        blocks_in_last_piece,
    };
}

#[derive(Debug)]
pub struct MockTorrent {
    pub pieces: Vec<Vec<Block>>,
    pub pieces_data: Vec<Vec<u8>>,
    pub piece_hashes: Vec<Vec<u8>>,
    pub layout: TorrentLayout,
}

impl MockTorrent {
    pub fn generate(num_of_pieces: usize, blocks_in_head_pieces: usize, blocks_in_last_piece: usize) -> Self {
        let mut piece_hashes = Vec::new();
        let mut pieces_data = Vec::new();
        let mut pieces: Vec<Vec<Block>> = Vec::new();
        let layout = generate_mock_layout(num_of_pieces, blocks_in_head_pieces, blocks_in_last_piece);

        for piece_idx in 0..layout.pieces {
            // add block positions for this piece
            let mut blocks: Vec<Block> = Vec::new();
            for block_idx in 0..layout.blocks_in_piece(piece_idx) {
                blocks.push(
                    Block {
                        piece_idx,
                        offset: layout.usual_block_length * block_idx,
                        length: layout.block_length(piece_idx, block_idx),
                    }
                );
            }
            pieces.push(blocks);

            // add piece data & hash
            let piece_len = layout.piece_length(piece_idx);
            let piece_data = vec![piece_idx as u8; piece_len]; // Mock piece data
            let mut hasher = Sha1::new();
            hasher.update(&piece_data);
            piece_hashes.push(hasher.finalize().to_vec());
            pieces_data.push(piece_data);
        }

        return MockTorrent {
            pieces,
            pieces_data,
            piece_hashes,
            layout,
        };
    }

    pub fn block_data(&self, piece_idx: usize, block_idx: usize) -> Vec<u8> {
        let block_len = self.layout.block_length(piece_idx, block_idx);
        let block_offset = self.layout.usual_block_length * block_idx;
        return self.pieces_data[piece_idx][block_offset..(block_offset + block_len)].to_vec();
    }

    pub fn total_length(&self) -> usize {
        return config::BLOCK_SIZE_BYTES *
            ((self.layout.pieces - 1) * self.layout.blocks_in_head_pieces + self.layout.blocks_in_last_piece);
    }
}
