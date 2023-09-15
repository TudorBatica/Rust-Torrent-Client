use std::sync::Arc;
use tokio::sync::mpsc::channel;
use rust_torrent_client::core_models::events::InternalEvent;
use rust_torrent_client::data_collector;
use rust_torrent_client::mocks::{MockDepsProvider, MockTorrent};

#[tokio::test]
async fn test_data_collection() {
    let blocks_in_piece_1 = 5;
    let blocks_in_piece_2 = 3;

    // spawn data collector task
    let (output_tx, mut output_rx) = channel::<InternalEvent>(64);
    let torrent = MockTorrent::generate(2, blocks_in_piece_1, blocks_in_piece_2);
    let deps = MockDepsProvider::new(torrent.clone(), output_tx.clone());
    let (handle, tx) = data_collector::spawn(Arc::new(deps)).await;

    // send blocks for first piece
    // we should get back `blocks_in_piece_1 - 1` BlockStored events
    for block_idx in 0..blocks_in_piece_1 {
        let data_block = torrent.data_block(0, block_idx);
        tx.send(data_block).await.unwrap();
        let event = output_rx.recv().await.unwrap();
        assert!(event.is_block_stored());
    }
    let event = output_rx.recv().await.unwrap();
    assert!(event.is_piece_stored());

    // for the second piece, send corrupt blocks
    // we should get back `blocks_in_piece_2 - 2` BlockStored events
    for block_idx in 0..blocks_in_piece_2 {
        let mut data_block = torrent.data_block(0, block_idx);
        data_block.piece_idx = 1;
        tx.send(data_block).await.unwrap();
        if block_idx < blocks_in_piece_2 - 1 {
            let event = output_rx.recv().await.unwrap();
            assert!(event.is_block_stored());
        }
    }

    // now, send correct data for the second piece
    // we should get back `blocks_in_piece_2 - 1` BlockStored events
    for block_idx in 0..blocks_in_piece_2 {
        let data_block = torrent.data_block(1, block_idx);
        tx.send(data_block).await.unwrap();
        let event = output_rx.recv().await.unwrap();
        assert!(event.is_block_stored());
    }
    let event = output_rx.recv().await.unwrap();
    assert!(event.is_piece_stored());
    let event = output_rx.recv().await.unwrap();
    assert!(event.is_download_complete());
}
