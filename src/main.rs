use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use config::Config;
use crate::metadata::Torrent;
use crate::p2p::P2PError;

mod config;
mod metadata;
mod tracker;
mod p2p;
mod messages;

#[tokio::main]
async fn main() {
    let config = Config::init();
    let torrent = metadata::parse_torrent("test_resources/debian-12.0.0-amd64-netinst.iso.torrent").unwrap();
    run(&torrent, &config).await;
}

async fn run(torrent: &Torrent, config: &Config) {
    // get info from tracker
    let tracker_resp = tracker::announce(&torrent, &config).await.unwrap();

    // start listener
    let listener = TcpListener::bind(("0.0.0.0", config.listening_port)).await.unwrap();
    println!("Started listening on port {} ...", config.listening_port);
    tokio::spawn(async move {
        loop {
            let conn = listener.accept().await;
            match conn {
                Ok((_socket, addr)) => println!("Received new connection from {}", addr),
                Err(e) => println!("TCP Listener encountered an error: {}", e)
            }
        }
    });

    let peer_tasks: Vec<JoinHandle<Result<(), P2PError>>> = tracker_resp.peers.into_iter().map(|peer| {
        let client_id = config.client_id.clone();
        let info_hash = torrent.info_hash.clone();
        return tokio::spawn(async move {
            return p2p::talk_to_peer(peer, info_hash, client_id).await;
        });
    }).collect();

    for task in peer_tasks {
        let result = task.await;
        match result {
            Ok(_) => println!("Tokio peer task finished without unexpected errors"),
            Err(_) => println!("Tokio peer task failed")
        }
    }
}

