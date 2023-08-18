use std::path::Path;
use std::sync::Arc;
use tokio::fs::{OpenOptions};
use tokio::net::{TcpListener};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use crate::config;
use crate::config::Config;
use crate::metadata::Torrent;
use crate::tracker::TrackerResponse;
use crate::transfer::{peer_connection, peer_transfer, state};
use crate::transfer::piece_picker::PiecePicker;
use crate::transfer::state::{CoordinatorTransferState};

pub async fn run(torrent: Torrent, tracker_response: TrackerResponse, client_config: &Config) {
    start_listener(client_config).await;

    // create output file
    let path = Path::new(torrent.info.name.as_str());
    let mut file = OpenOptions::new().create(true).read(true).write(true).open(path).await.expect("Could not open file");

    // create piece picker
    let last_piece_len = torrent.info.length.expect("only single file torrents supported") - (torrent.info.piece_length * torrent.piece_hashes.len() as u64 - 1);
    let picker = PiecePicker::init(
        torrent.piece_hashes.len(),
        torrent.info.piece_length as usize,
        last_piece_len as usize,
        config::BLOCK_SIZE_BYTES,
    );
    let picker = Arc::new(Mutex::new(picker));
    let mut transfer_state = CoordinatorTransferState::init(&torrent, picker);


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
}

// fn init_piece_picker(torrent: &Torrent) -> Mutex<PiecePicker> {
//     torrent.
// }

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

