mod config;
mod metadata;
mod tracker;

#[tokio::main]
async fn main() {
    let config = config::Config::init();
    let torrent = metadata::parse_torrent("test_resources/debian-12.0.0-amd64-netinst.iso.torrent").unwrap();
    let tracker_resp = tracker::announce(&torrent, &config).await.unwrap();

    println!("tracker response: {:?}", tracker_resp);
}
