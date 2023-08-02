use config::Config;
use crate::transfer::coordinator;

mod config;
mod metadata;
mod tracker;
mod messages;

mod transfer {
    pub mod coordinator;
    pub mod peer_connection;
    pub mod peer_transfer;
    pub mod state;
}

#[tokio::main]
async fn main() {
    let config = Config::init();
    let torrent = metadata::parse_torrent("test_resources/debian-12.0.0-amd64-netinst.iso.torrent").unwrap();
    let tracker_resp = tracker::announce(&torrent, &config).await.unwrap();
    coordinator::run(torrent, tracker_resp, &config).await;
}

