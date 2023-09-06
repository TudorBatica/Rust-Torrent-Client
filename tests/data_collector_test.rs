use rust_torrent_client;

use std::sync::Arc;
use tokio::sync::mpsc::{channel};
use tokio::sync::Mutex;
use rust_torrent_client::core_models::entities::{Block};
use rust_torrent_client::core_models::events::InternalEvent;
use rust_torrent_client::file_provider::StdFileProvider;
use rust_torrent_client::mocks::MockTorrent;

#[tokio::test]
async fn test_complete_download() {
    // init mocks
    let torrent = MockTorrent::generate(3, 4, 3);
    let temp_file = tempfile::tempfile().unwrap();
    temp_file.set_len(torrent.total_length() as u64).unwrap();
    let file_provider = StdFileProvider::new(temp_file);
    let mut piece_picker = rust_torrent_client::piece_picker::MockPiecePicker::new();
    piece_picker.expect_remove_block().returning(|_| ());
    piece_picker.expect_reinsert_piece().returning(|_| ());

    // init channels
    let (in_tx, in_rx) = channel::<(Block, Vec<u8>)>(32);
    let (out_tx, mut out_rx) = channel::<InternalEvent>(32);

    // spawn task
    let piece_hashes = torrent.piece_hashes.clone();
    let layout = torrent.layout.clone();
    let handle = tokio::spawn(async move {
        rust_torrent_client::data_collector::run(
            Arc::new(Mutex::new(piece_picker)),
            Box::new(file_provider),
            piece_hashes,
            layout,
            in_rx,
            out_tx,
        ).await;
    });

    // send blocks
    for piece_idx in 0..torrent.layout.pieces {
        // send bad data first for the last piece
        if piece_idx == torrent.layout.pieces - 1 {
            let blocks_count = torrent.layout.blocks_in_piece(piece_idx);
            for block_idx in 0..blocks_count {
                let block_pos: Block = torrent.pieces[piece_idx][block_idx].clone();
                let block_data = torrent.block_data(0, block_idx);
                in_tx.send((block_pos, block_data)).await.unwrap();
            }
        }

        let blocks_count = torrent.layout.blocks_in_piece(piece_idx);
        for block_idx in 0..blocks_count {
            let block_pos: Block = torrent.pieces[piece_idx][block_idx].clone();
            let block_data = torrent.block_data(piece_idx, block_idx);
            in_tx.send((block_pos, block_data)).await.unwrap();
        }
    }
    drop(in_tx);
    handle.await.unwrap();

    // recv data and check results
    let mut output_events: Vec<InternalEvent> = Vec::new();
    while let Some(event) = out_rx.recv().await {
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
