use sha1::{Digest, Sha1};
use crate::config;
use crate::core_models::entities::TorrentLayout;

pub fn generate_mock_torrent(num_of_pieces: usize) -> (Vec<Vec<u8>>, Vec<Vec<u8>>, TorrentLayout) {
    let mut piece_hashes = Vec::new();
    let mut pieces = Vec::new();
    let piece_len = config::BLOCK_SIZE_BYTES * 5;
    let last_piece_len = config::BLOCK_SIZE_BYTES * 3;

    for i in 0..num_of_pieces {
        let piece_len = if i == num_of_pieces - 1 { last_piece_len } else { piece_len };
        let piece_data = vec![i as u8; piece_len]; // Mock piece data

        let mut hasher = Sha1::new();
        hasher.update(&piece_data);
        let piece_hash = hasher.finalize().to_vec();

        pieces.push(piece_data);
        piece_hashes.push(piece_hash);
    }

    let layout = TorrentLayout {
        pieces: num_of_pieces,
        head_pieces_length: piece_len * (num_of_pieces - 1),
        last_piece_length: last_piece_len,
        usual_block_length: config::BLOCK_SIZE_BYTES,
        head_pieces_last_block_length: config::BLOCK_SIZE_BYTES,
        last_piece_last_block_length: last_piece_len % config::BLOCK_SIZE_BYTES,
        blocks_in_head_pieces: 5,
        blocks_in_last_piece: 3,
    };

    return (pieces, piece_hashes, layout);
}
