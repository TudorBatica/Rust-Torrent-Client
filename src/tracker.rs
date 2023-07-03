use form_urlencoded::byte_serialize;
use crate::config::Config;
use reqwest::{Client, Url};
use crate::metadata::Torrent;

pub async fn announce(torrent: &Torrent, peer_config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", &torrent.info_hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>());

    let port = peer_config.listening_port.to_string();
    let info_hash = byte_serialize(&torrent.info_hash).collect::<String>();

    let query_params = [
        ("info_hash", info_hash.as_ref()),
        ("peer_id", peer_config.peer_id.as_ref()),
        ("port", port.as_ref()),
        ("compact", "1"),
        ("event", "started"),
        ("downloaded", "0"),
        ("uploaded", "0"),
    ];

    let query_params = query_params.into_iter().enumerate()
        .map(|(param_idx, (param, value))| {
            if param_idx == 0 {
                format!("{}={}", param, value)
            } else {
                format!("&{}={}", param, value)
            }
        })
        .collect::<String>();

    let url = format!("{}?{}", torrent.announce.as_ref().unwrap(), query_params);
    println!("url is : {}", url);

    let tracker_response = reqwest::get(url).await?;

    println!("{:?}", tracker_response);
    println!("text: {:?}", tracker_response.text().await?);

    return Ok(());
}