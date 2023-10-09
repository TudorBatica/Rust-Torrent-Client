use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sha1::{Digest, Sha1};
use tokio::sync::{mpsc};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use crate::dependency_provider::TransferDeps;
use crate::core_models::entities::{Block, DataBlock, TorrentLayout};
use crate::core_models::events::InternalEvent;
use crate::file_provider::{FileProv};

pub fn spawn(deps: Arc<dyn TransferDeps>) -> (JoinHandle<()>, Sender<DataBlock>) {
    let (tx_to_self, rx) = mpsc::channel::<DataBlock>(1024);
    let handle = tokio::spawn(async move {
        run(deps.clone(), rx).await;
    });

    return (handle, tx_to_self);
}

async fn run(deps: Arc<dyn TransferDeps>, mut rx: Receiver<DataBlock>) {
    let tx = deps.output_tx();
    let layout = deps.torrent_layout();
    let hashes = deps.piece_hashes();
    let mut file_prov = deps.file_provider();
    let picker = deps.piece_picker();

    file_prov.open_read_write_instance().await;
    let mut acquired_pieces = 0;
    let mut written_data: HashMap<usize, HashSet<Block>> = (0..layout.pieces).into_iter()
        .map(|piece_idx| (piece_idx, HashSet::new()))
        .collect();

    while let Some(data_block) = rx.recv().await {
        let blocks = written_data.get_mut(&data_block.piece_idx).unwrap();
        if !blocks.insert(data_block.to_block()) {
            continue;
        }
        file_prov.write(data_block.piece_idx, data_block.offset, &data_block.data).await;

        if piece_incomplete(data_block.piece_idx, &layout, blocks.len()) {
            {
                let mut picker = picker.lock().await;
                picker.remove_block(&data_block.to_block());
            }
            tx.send(InternalEvent::BlockStored(data_block.to_block())).await.unwrap();
        } else if piece_corrupt(data_block.piece_idx, &mut file_prov, &hashes).await {
            println!("Data Collector :: piece corrupt -> {}", data_block.piece_idx);
            {
                let mut picker = picker.lock().await;
                picker.reinsert_piece(data_block.piece_idx);
            }
            written_data.insert(data_block.piece_idx, HashSet::new());
        } else {
            acquired_pieces += 1;
            {
                let mut picker = picker.lock().await;
                picker.remove_block(&data_block.to_block());
            }
            let piece_idx = data_block.piece_idx.clone();
            tx.send(InternalEvent::BlockStored(data_block.to_block())).await.unwrap();
            tx.send(InternalEvent::PieceStored(piece_idx)).await.unwrap();
            println!("Data Collector :: piece complete -> {}, {} out of {}", data_block.piece_idx, acquired_pieces, layout.pieces);
        }
        if acquired_pieces == layout.pieces {
            println!("Data Collector :: download complete");
            tx.send(InternalEvent::DownloadComplete).await.unwrap();
            break;
        }
    }
}

fn piece_incomplete(piece_idx: usize, layout: &TorrentLayout, stored_blocks_in_piece: usize) -> bool {
    return stored_blocks_in_piece < layout.blocks_in_piece(piece_idx);
}

async fn piece_corrupt(piece_idx: usize, file: &mut Box<dyn FileProv>, hashes: &Vec<Vec<u8>>) -> bool {
    // compute piece hash
    let piece = file.read_piece(piece_idx).await;
    let mut hasher = Sha1::new();
    hasher.update(piece);
    let piece_hash = hasher.finalize().into_iter().collect::<Vec<u8>>();

    return piece_hash.cmp(&hashes[piece_idx]).is_ne();
}