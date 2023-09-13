use std::sync::Arc;
use tokio::sync::mpsc::channel;
use rust_torrent_client::core_models::events::InternalEvent;
use rust_torrent_client::data_collector;
use rust_torrent_client::mocks::{MockDepsProvider, MockTorrent};

#[tokio::test]
async fn test_complete_download() {
    let (output_tx, mut output_rx) = channel::<InternalEvent>(64);
    let torrent = MockTorrent::generate(5, 10, 8);
    let deps = MockDepsProvider::new(torrent.clone(), output_tx.clone());

    let (handle, tx) = data_collector::spawn(Arc::new(deps)).await;

    // send blocks
    for piece_idx in 0..torrent.layout.pieces {
        // send bad data first for the last piece
        if piece_idx == torrent.layout.pieces - 1 {
            let blocks_count = torrent.layout.blocks_in_piece(piece_idx);
            for block_idx in 0..blocks_count {
                let data_block = torrent.data_block(piece_idx, block_idx);
                tx.send(data_block).await.unwrap();
            }
        }

        let blocks_count = torrent.layout.blocks_in_piece(piece_idx);
        for block_idx in 0..blocks_count {
            let data_block = torrent.data_block(piece_idx, block_idx);
            tx.send(data_block).await.unwrap();
        }
    }

    handle.await.unwrap();

    // recv data and check results
    let mut output_events: Vec<InternalEvent> = Vec::new();
    while let Some(event) = output_rx.recv().await {
        output_events.push(event);
    }

    println!("{:?}", output_events);

    assert!(output_events.pop().unwrap().is_download_complete());
    for piece_idx in (0..torrent.layout.pieces).rev() {
        assert!(output_events.pop().unwrap().is_piece_stored());
        let mut blocks = torrent.layout.blocks_in_piece(piece_idx);
        // for the last piece we sent bad data at first, so we should have twice the number of blocks sent
        if piece_idx == torrent.layout.pieces - 1 {
            blocks = blocks * 2 - 1;
        }
        for _ in 0..blocks {
            assert!(output_events.pop().unwrap().is_block_stored());
        }
    }
    assert!(output_events.is_empty());
}
