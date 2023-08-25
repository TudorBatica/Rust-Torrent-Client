use std::sync::Arc;
use std::sync::mpsc::Sender;
use tokio::fs::{OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener};
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use crate::{config, download_assembler};
use crate::config::Config;
use crate::core_models::BlockPosition;
use crate::internal_events::{CoordinatorInput, DownloadAssemblerEvent};
use crate::metadata::Torrent;
use crate::tracker::TrackerResponse;
use crate::transfer::{peer_connection, peer_transfer, state};
use crate::transfer::coordinator::TransferError::TransferInitializationError;
use crate::transfer::piece_picker::PiecePicker;
use crate::transfer::state::{CoordinatorTransferState};

pub enum TransferError {
    TransferInitializationError(String)
}

pub async fn run(torrent: Torrent, tracker_response: TrackerResponse, client_config: &Config) -> Result<(), TransferError> {
    start_listener(client_config).await;
    //todo: compute torrent layout and send it to components which require it
    let picker = init_piece_picker(&torrent);
    let mut transfer_state = CoordinatorTransferState::init(&torrent, picker);

    // initialize ipc channels
    let (tx_to_assembler, rx_assembler) = mpsc::channel::<(BlockPosition, Vec<u8>)>(128);

    //todo: create cancellation tokens
    let assembler_task = spawn_download_assembler_task(&torrent, &transfer_state, rx_assembler).await?;

    // spawn choke/unchoke scheduler


    // spawn tracker updates task

    // spawn p2p tasks
    let p2p_transfer_tasks: Vec<JoinHandle<()>> = tracker_response.peers.into_iter().map(|peer| {
        let client_id = client_config.client_id.clone();
        let info_hash = torrent.info_hash.clone();
        let (peer_transfer_state, channel) = state::register_new_peer_transfer(&mut transfer_state);

        return tokio::spawn(async move {
            let (read_conn, write_conn) = match peer_connection::connect(peer, info_hash, client_id).await {
                Ok(conn) => conn,
                Err(err) => {
                    println!("P2P transfer failed: {:?}", err);
                    return;
                }
            };
            match peer_transfer::run_transfer(peer_transfer_state, read_conn, write_conn, channel.0, channel.1).await {
                Ok(_) => { println!("P2P transfer finished successfully!"); }
                Err(err) => { println!("P2P transfer failed: {:?}", err); }
            }
        });
    }).collect();


    for p2p_task in p2p_transfer_tasks {
        let _ = p2p_task.await;
    }

    return Ok(());
}

fn init_piece_picker(torrent: &Torrent) -> Arc<Mutex<PiecePicker>> {
    let last_piece_len = torrent.info.length.expect("only single file torrents supported")
        - (torrent.info.piece_length * torrent.piece_hashes.len() as u64 - 1);
    let picker = PiecePicker::init(
        torrent.piece_hashes.len(),
        torrent.info.piece_length as usize,
        last_piece_len as usize,
        config::BLOCK_SIZE_BYTES,
    );
    return Arc::new(Mutex::new(picker));
}

async fn spawn_download_assembler_task(torrent: &Torrent,
                                       transfer_state: &CoordinatorTransferState,
                                       rx: Receiver<(BlockPosition, Vec<u8>)>) -> Result<JoinHandle<()>, TransferError> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(torrent.info.name.as_str())
        .await
        .map_err(|_| TransferInitializationError("Could not open output file".to_string()))?;

    file.set_len(torrent.info.length.expect("Only single-file torrents currently supported") as u64)
        .await
        .map_err(|_| TransferInitializationError("Could not set file length".to_string()))?;

    let handle = tokio::spawn(
        download_assembler::run(
            transfer_state.piece_picker.clone(),
            torrent.piece_hashes.clone(),
            file,
            rx,
            transfer_state.tx_to_coordinator.clone())
    );

    return Ok(handle);
}

//todo: move outside of coordinator
async fn start_listener(client_config: &Config) {
    let listener = TcpListener::bind(("0.0.0.0", client_config.listening_port)).await.unwrap();
    println!("Started listening on port {} ...", client_config.listening_port);
    tokio::spawn(async move {
        loop {
            let conn = listener.accept().await;
            match conn {
                Ok((_socket, addr)) => println!("Received new connection from {}", addr),
                Err(e) => println!("TCP Listener encountered an error: {}", e)
            }
        }
    });
}

