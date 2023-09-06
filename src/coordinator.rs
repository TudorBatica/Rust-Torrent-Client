use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use crate::{config, data_collector, p2p, state};
use crate::config::Config;
use crate::coordinator::TransferError::TransferInitializationError;
use crate::core_models::entities::Torrent;
use crate::core_models::entities::{Block, TorrentLayout};
use crate::file_provider::TokioFileProvider;
use crate::p2p::transfer;
use crate::tracker::TrackerResponse;
use crate::piece_picker::RarestPiecePicker;
use crate::state::CoordinatorTransferState;

pub enum TransferError {
    TransferInitializationError(String)
}

pub async fn run(torrent: Torrent, tracker_response: TrackerResponse, client_config: &Config) -> Result<(), TransferError> {
    start_listener(client_config).await;
    let layout = TorrentLayout::from_torrent(&torrent);
    let picker = Arc::new(Mutex::new(RarestPiecePicker::init(&layout)));
    let mut transfer_state = CoordinatorTransferState::init(&torrent, picker);

    // initialize ipc channels
    let (tx_to_assembler, rx_assembler) = mpsc::channel::<(Block, Vec<u8>)>(128);

    //todo: create cancellation tokens
    let assembler_task = spawn_data_collector(
        &torrent,
        layout.clone(),
        &transfer_state,
        rx_assembler,
    ).await?;

    // spawn choke/unchoke scheduler


    // spawn tracker updates task

    // spawn p2p tasks
    let p2p_transfer_tasks: Vec<JoinHandle<()>> = tracker_response.peers.into_iter().map(|peer| {
        let client_id = client_config.client_id.clone();
        let info_hash = torrent.info_hash.clone();
        let file_name = torrent.info.name.clone();
        let layout = layout.clone();
        let (
            peer_transfer_state, channel
        ) = state::register_new_peer_transfer(&mut transfer_state, layout);

        return tokio::spawn(async move {
            let (read_conn, write_conn) = match p2p::connection::connect(peer, info_hash, client_id).await {
                Ok(conn) => conn,
                Err(err) => {
                    println!("P2P p2p failed: {:?}", err);
                    return;
                }
            };
            let file = OpenOptions::new()
                .write(false)
                .create(false)
                .truncate(false)
                .open(file_name.as_str())
                .await
                .unwrap();
            let file_provider = TokioFileProvider::new(file);
            match transfer::run(
                peer_transfer_state,
                Box::new(file_provider),
                Box::new(read_conn),
                Box::new(write_conn),
                channel.0,
                channel.1,
            ).await {
                Ok(_) => { println!("P2P p2p finished successfully!"); }
                Err(err) => { println!("P2P p2p failed: {:?}", err); }
            }
        });
    }).collect();


    for p2p_task in p2p_transfer_tasks {
        let _ = p2p_task.await;
    }

    return Ok(());
}

async fn spawn_data_collector(torrent: &Torrent,
                              torrent_layout: TorrentLayout,
                              transfer_state: &CoordinatorTransferState,
                              rx: Receiver<(Block, Vec<u8>)>) -> Result<JoinHandle<()>, TransferError> {
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

    let file_provider = Box::new(TokioFileProvider::new(file));
    let layout = torrent_layout.clone();
    let hashes = torrent.piece_hashes.clone();
    let picker = transfer_state.piece_picker.clone();
    let tx = transfer_state.tx_to_coordinator.clone();

    let handle = tokio::spawn(async {
        data_collector::run(
            picker, file_provider, hashes, layout, rx, tx,
        ).await;
    });

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

