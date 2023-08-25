mod config;
mod metadata;
mod tracker;
mod download_assembler;
mod internal_events;
mod core_models;

mod transfer {
    pub mod coordinator;
    pub mod peer_connection;
    pub mod peer_message;
    pub mod peer_transfer;
    pub mod piece_picker;
    pub mod state;
}

// #[tokio::main]
// async fn main() {
//     let config = Config::init();
//     let torrent = metadata::parse_torrent("test_resources/debian-12.0.0-amd64-netinst.iso.torrent").unwrap();
//     let tracker_resp = tracker::announce(&torrent, &config).await.unwrap();
//     coordinator::run(torrent, tracker_resp, &config).await;
// }

use tokio::fs::{OpenOptions};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::time;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Open the file for writing
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("shared_file.bin")
        .await?;

    // Initialize the bitfield vector with all false values
    let bitfield = Arc::new(Mutex::new(vec![false; 4]));

    // Spawn the writing task
    let write_task_bitfield = bitfield.clone();
    let write_task = tokio::spawn(async move {
        let data_to_write = vec![0xABu8; 16 * 1024]; // Example data
        let mut file_offset = 0;

        for _ in 0..4 {
            file.write_all(&data_to_write).await.unwrap();
            file.sync_data().await.unwrap();
            {
                // Acquire the bitfield mutex and mark the piece as complete
                let mut bitfield = write_task_bitfield.lock().unwrap();
                bitfield[file_offset / (16 * 1024)] = true;
            }
            file_offset += 16 * 1024;
            time::sleep(Duration::from_secs(1)).await;
        }
    });

    // Spawn the reading tasks
    let read_task_bitfield = bitfield.clone();
    let read_task_1 = tokio::spawn(async move {
        println!("spawned task 1");
        let mut file = File::open("shared_file.bin").await.unwrap();
        let mut buffer = vec![0u8; 16 * 1024];

        for piece_index in 0..4 {
            loop {
                {
                    // Acquire the bitfield mutex to check if the piece can be read
                    let bitfield = read_task_bitfield.lock().unwrap();
                    if bitfield[piece_index] {
                        break;
                    }
                }
                time::sleep(Duration::from_secs(1)).await;
            }

            file.seek(io::SeekFrom::Start(piece_index as u64 * (16 * 1024)))
                .await
                .unwrap();
            file.read_exact(&mut buffer).await.unwrap();

            // Compare the read data with expected data (example: 0xAB)
            let expected_data = vec![0xABu8; 16 * 1024];
            assert_eq!(buffer, expected_data);
            println!("read correct data, task 1");
        }
    });

    let read_task_bitfield = bitfield.clone();
    let read_task_2 = tokio::spawn(async move {
        println!("spawned task 2");
        let mut file = File::open("shared_file.bin").await.unwrap();
        let mut buffer = vec![0u8; 16 * 1024];

        for piece_index in 0..4 {
            loop {
                {
                    // Acquire the bitfield mutex to check if the piece can be read
                    let bitfield = read_task_bitfield.lock().unwrap();
                    if bitfield[piece_index] {
                        break;
                    }
                }
                time::sleep(Duration::from_secs(1)).await;
            }

            file.seek(io::SeekFrom::Start(piece_index as u64 * (16 * 1024)))
                .await
                .unwrap();
            file.read_exact(&mut buffer).await.unwrap();

            // Compare the read data with expected data (example: 0xAB)
            let expected_data = vec![0xABu8; 16 * 1024];
            assert_eq!(buffer, expected_data);
            println!("read correct data, task 2");
        }
    });

    // Await the completion of the tasks
    write_task.await.unwrap();
    read_task_1.await.unwrap();
    read_task_2.await.unwrap();

    Ok(())
}

