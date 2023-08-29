use sha1::{Digest, Sha1};
use crate::data_collector::state::DataCollectorState;
use crate::core_models::entities::{BlockPosition};

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
    use crate::mocks;



    #[tokio::test]
    async fn test_block_acquired() {
        let (pieces, hashes, layout) = mocks::generate_mock_torrent(3);
        // let

    }
}

