use std::path::Path;
use tokio::fs::{OpenOptions};
use tokio::net::{TcpListener};
use tokio::task::JoinHandle;
use crate::config::Config;
use crate::metadata::Torrent;
use crate::tracker::TrackerResponse;
use crate::transfer::{peer_transfer, state};
use crate::transfer::peer_connection::{PeerConnection};
use crate::transfer::state::{CoordinatorTransferState};

pub async fn run(torrent: Torrent, tracker_response: TrackerResponse, client_config: &Config) {
    start_listener(client_config).await;

    // create output file
    let path = Path::new(torrent.info.name.as_str());
    let mut file = OpenOptions::new().create(true).read(true).write(true).open(path).await.expect("Could not open file");

    let mut transfer_state = CoordinatorTransferState::init(&torrent, file);

    // spawn p2p tasks
    let p2p_transfer_tasks: Vec<JoinHandle<()>> = tracker_response.peers.into_iter().map(|peer| {
        let client_id = client_config.client_id.clone();
        let info_hash = torrent.info_hash.clone();
        let peer_transfer_state = state::register_new_peer_transfer(&mut transfer_state);

        return tokio::spawn(async move {
            let conn = match PeerConnection::start_connection(peer, info_hash, client_id).await {
                Ok(conn) => conn,
                Err(err) => {
                    println!("P2P transfer failed: {:?}", err);
                    return;
                }
            };
            match peer_transfer::run_transfer(peer_transfer_state, conn).await {
                Ok(_) => { println!("P2P transfer finished successfully!"); }
                Err(err) => { println!("P2P transfer failed: {:?}", err); }
            }
        });
    }).collect();

    for p2p_task in p2p_transfer_tasks {
        let _ = p2p_task.await;
    }
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

