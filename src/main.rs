mod config;
mod metadata;
mod tracker;

#[tokio::main]
async fn main() {
    let config = config::Config::init();
    let torrent: metadata::Torrent = metadata::parse_torrent("test_resources/debian-12.0.0-amd64-netinst.iso.torrent").unwrap();
    tracker::announce(&torrent, &config).await.expect("TODO: panic message");
}
