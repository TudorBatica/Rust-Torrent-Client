use rust_torrent_client::{coordinator, parser, tracker};
use rust_torrent_client::config::Config;

#[tokio::main]
async fn main() {
    let config = Config::init();
    let torrent = parser::parse_torrent("test_resources/debian-12.0.0-amd64-netinst.iso.torrent").unwrap();
    let tracker_resp = tracker::announce(&torrent, &config).await.unwrap();
    let transfer_result = coordinator::run(torrent, tracker_resp, &config).await;
}
