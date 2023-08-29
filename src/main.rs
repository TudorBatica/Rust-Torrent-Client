use crate::config::Config;

mod config;
mod metadata;
mod tracker;
pub mod piece_picker;
pub mod coordinator;
pub mod state;
mod file_provider;
mod mocks;

mod core_models {
    pub mod entities;
    pub mod internal_events;
}

mod p2p {
    pub mod connection;
    pub mod transfer;
}

mod data_collector {
    pub mod runner;
    mod handler;
    mod state;
}

#[tokio::main]
async fn main() {
    let config = Config::init();
    let torrent = metadata::parse_torrent("test_resources/debian-12.0.0-amd64-netinst.iso.torrent").unwrap();
    let tracker_resp = tracker::announce(&torrent, &config).await.unwrap();
    let transfer_result = coordinator::run(torrent, tracker_resp, &config).await;
}
