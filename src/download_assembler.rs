use std::collections::{HashMap, HashSet};
use std::io::SeekFrom;
use std::sync::{Arc};
use sha1::{Digest, Sha1};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use crate::core_models::{BlockPosition, TorrentLayout};
use crate::internal_events::{CoordinatorInput, DownloadAssemblerEvent};
use crate::transfer::piece_picker::PiecePicker;

pub async fn run(piece_picker: Arc<Mutex<PiecePicker>>,
                 piece_hashes: Vec<Vec<u8>>,
                 layout: TorrentLayout,
                 mut file: File,
                 mut rx: Receiver<(BlockPosition, Vec<u8>)>,
                 tx: Sender<CoordinatorInput>,
) {
    let mut written_data: HashMap<usize, HashSet<BlockPosition>> = (0..layout.pieces).into_iter()
        .map(|piece_idx| (piece_idx, HashSet::new()))
        .collect();
    let mut acquired_pieces = 0;

    while let Some((block, data)) = rx.recv().await {
        // check if block has not already been written
        let blocks = written_data.get_mut(&block.piece_idx).unwrap();
        if !blocks.insert(block.clone()) {
            continue;
        }

        // write block
        let block_absolute_offset = block.piece_idx * layout.head_pieces_length + block.offset;
        file.seek(SeekFrom::Start(block_absolute_offset as u64)).await.unwrap();
        file.write_all(&*data).await.unwrap();

        // check if piece is complete
        if blocks.len() < layout.blocks_in_piece(block.piece_idx) {
            println!("Block {} {} acquired!", block.piece_idx, block.offset);
            {
                let mut picker = piece_picker.lock().await;
                picker.remove_block(&block);
            }
            tx.send(CoordinatorInput::DASEvent(DownloadAssemblerEvent::BlockAcquired(block)));
            continue;
        }

        // check if piece is corrupt
        if !piece_hash_matches(&mut file, &piece_hashes, block.piece_idx, &layout).await {
            println!("Piece {} corrupt!", block.piece_idx);
            {
                let mut picker = piece_picker.lock().await;
                picker.reinsert_piece(block.piece_idx);
            }
            written_data.insert(block.piece_idx, HashSet::new());
        }

        // handle piece acquired
        println!("Piece {} acquired!", block.piece_idx);
        acquired_pieces += 1;
        {
            let mut picker = piece_picker.lock().await;
            picker.remove_block(&block);
        }
        let piece_idx = block.piece_idx.clone();
        tx.send(CoordinatorInput::DASEvent(DownloadAssemblerEvent::BlockAcquired(block)));
        tx.send(CoordinatorInput::DASEvent(DownloadAssemblerEvent::PieceAcquired(piece_idx)));

        // check if download is complete
        if acquired_pieces == piece_hashes.len() {
            println!("Download complete!");
            tx.send(CoordinatorInput::DASEvent(DownloadAssemblerEvent::DownloadComplete));
            break;
        }
    }
}

async fn piece_hash_matches(file: &mut File, piece_hashes: &Vec<Vec<u8>>,
                            piece_idx: usize, layout: &TorrentLayout) -> bool {
    let piece_offset = piece_idx * layout.head_pieces_length;
    let piece_len = layout.piece_length(piece_idx);

    let mut piece_buff = vec![0u8; piece_len];
    file.seek(SeekFrom::Start(piece_offset as u64)).await.unwrap();
    file.read_exact(&mut piece_buff).await.unwrap();

    let mut hasher = Sha1::new();
    hasher.update(piece_buff);
    let piece_hash = hasher.finalize().into_iter().collect::<Vec<u8>>();
    return piece_hash.cmp(&piece_hashes[piece_idx]).is_eq();
}
