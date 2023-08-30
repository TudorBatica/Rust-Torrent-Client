use sha1::{Digest, Sha1};
use crate::data_collector::state::DataCollectorState;
use crate::core_models::entities::{BlockPosition};

#[derive(Debug, PartialEq)]
pub enum DataCollectionResult {
    BlockStored,
    PieceAcquired,
    DownloadComplete,
    NoUpdate,
}

pub async fn handle_block(block: &BlockPosition, block_data: Vec<u8>, state: &mut DataCollectorState) -> DataCollectionResult {
    // check if this block has already been written
    let blocks = state.written_data.get_mut(&block.piece_idx).unwrap();
    if !blocks.insert(block.clone()) {
        return DataCollectionResult::NoUpdate;
    }

    // write the new block
    let block_absolute_offset = block.piece_idx * state.layout.head_pieces_length + block.offset;
    state.file_provider.write(block_absolute_offset, block_data).await;

    // check if we have a complete piece
    if blocks.len() < state.layout.blocks_in_piece(block.piece_idx) {
        {
            let mut picker = state.piece_picker.lock().await;
            picker.remove_block(&block);
        }
        return DataCollectionResult::BlockStored;
    }

    // compute piece hash
    let piece_offset = block.piece_idx * state.layout.head_pieces_length;
    let piece_len = state.layout.piece_length(block.piece_idx);
    let piece_buff = state.file_provider.read(piece_offset, piece_len).await;
    let mut hasher = Sha1::new();
    hasher.update(piece_buff);
    let piece_hash = hasher.finalize().into_iter().collect::<Vec<u8>>();

    // if piece is corrupt, reinsert it for download
    if piece_hash.cmp(&state.piece_hashes[block.piece_idx]).is_ne() {
        {
            let mut picker = state.piece_picker.lock().await;
            picker.reinsert_piece(block.piece_idx);
        }
        return DataCollectionResult::NoUpdate;
    }

    // new piece acquired! :)
    state.acquired_pieces += 1;
    {
        let mut picker = state.piece_picker.lock().await;
        picker.remove_block(&block);
    }

    return if state.acquired_pieces == state.piece_hashes.len() {
        DataCollectionResult::DownloadComplete
    } else {
        DataCollectionResult::PieceAcquired
    };
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use crate::core_models::entities::{BlockPosition, TorrentLayout};
    use crate::data_collector::handler::{DataCollectionResult, handle_block};
    use crate::data_collector::state::DataCollectorState;
    use crate::file_provider::FileProvider;
    use crate::mocks;
    use crate::mocks::MockTorrent;
    use crate::piece_picker::PiecePicker;

    #[tokio::test]
    async fn test_block_stored() {
        let torrent = mocks::generate_mock_torrent(3);
        let mut file_provider = crate::file_provider::MockFileProvider::new();
        file_provider.expect_read().returning(|_, _| vec![]);
        file_provider.expect_write().returning(|_, _| ());
        let mut state = init_state(&torrent, Box::new(file_provider));

        let block_pos: &BlockPosition = &torrent.pieces[0][0];
        let block_data = torrent.get_block_data(0, 0);

        let result = handle_block(block_pos, block_data, &mut state).await;

        assert_eq!(result, DataCollectionResult::BlockStored);
        assert_eq!(state.acquired_pieces, 0);
        assert!(state.written_data.get(&0).is_some());
        assert!(state.written_data.get(&0).unwrap().contains(block_pos));
    }

    #[tokio::test]
    async fn test_piece_acquired() {
        let torrent = mocks::generate_mock_torrent(3);

        let piece_idx = 0 as usize;
        let piece = torrent.pieces_data[piece_idx.clone()].to_vec();
        let mut file_provider = crate::file_provider::MockFileProvider::new();
        file_provider.expect_write().returning(|_, _| ());
        file_provider.expect_read().returning(move |_, _| piece.clone());

        let mut state = init_state(&torrent, Box::new(file_provider));
        let mut result: DataCollectionResult = DataCollectionResult::BlockStored;

        for block_idx in 0..torrent.layout.blocks_in_piece(piece_idx) {
            let block_pos: &BlockPosition = &torrent.pieces[piece_idx][block_idx];
            let block_data = torrent.get_block_data(piece_idx, block_idx);
            result = handle_block(block_pos, block_data, &mut state).await;
        }

        assert_eq!(result, DataCollectionResult::PieceAcquired);
        assert_eq!(state.acquired_pieces, 1);
    }

    fn init_state(torrent: &MockTorrent, file_provider: Box<dyn FileProvider>) -> DataCollectorState {
        let piece_picker = PiecePicker::init(
            torrent.layout.pieces,
            torrent.layout.head_pieces_length,
            torrent.layout.last_piece_length,
            torrent.layout.usual_block_length,
        );
        let piece_picker = Arc::new(Mutex::new(piece_picker));
        return DataCollectorState::init(
            file_provider, torrent.piece_hashes.clone(), torrent.layout.clone(), piece_picker,
        );
    }
}

