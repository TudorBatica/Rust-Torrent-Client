use std::fs::OpenOptions;
use std::sync::Arc;
use tokio::sync::mpsc;
use rust_torrent_client::{torrent_parser};
use rust_torrent_client::config::Config;
use rust_torrent_client::dependency_provider::DependencyProvider;
use rust_torrent_client::core_models::entities::{TorrentLayout};

#[tokio::main]
async fn main() {
    // Retrieve .torrent file path arg
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-torrent-file>", args[0]);
        std::process::exit(1);
    }
    let torrent_file_path = &args[1];

    // initialize client
    let config = Config::init();

    // parse metadata and prepare output files
    let torrent = torrent_parser::parse_torrent(torrent_file_path).unwrap();
    let layout = TorrentLayout::from_torrent(&torrent);

    create_output_files(&layout);

    // prepare shared dependencies
    let (coordinator_tx, coordinator_rx) = mpsc::channel(1024);
    let deps = DependencyProvider::init(config, torrent, layout, coordinator_tx);

    let _ = rust_torrent_client::coordinator::task::run(Arc::new(deps), coordinator_rx).await;
}

fn create_output_files(layout: &TorrentLayout) {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(layout.output_file_path.as_str())
        .unwrap();
    file.set_len(layout.output_file_length as u64).unwrap();
}
