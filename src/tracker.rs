use crate::config::Config;
use crate::metadata::Torrent;
use form_urlencoded::byte_serialize;
use serde::Deserialize;
use serde_derive::Deserialize;
use std::net::Ipv4Addr;

#[derive(Debug, Deserialize)]
pub struct Peer {
    pub ip: Ipv4Addr,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct TrackerResponse {
    #[serde(default)]
    pub complete: u16,
    #[serde(default)]
    pub incomplete: u16,
    pub interval: u64,
    #[serde(deserialize_with = "deserialize_peers")]
    pub peers: Vec<Peer>,
    #[serde(default)]
    #[serde(rename = "tracker id")]
    pub tracker_id: Option<String>,
}

pub async fn announce(torrent: &Torrent, peer_config: &Config) -> Result<TrackerResponse, Box<dyn std::error::Error>> {
    let url = create_url(torrent, peer_config);
    let response = reqwest::get(url).await?.bytes().await?;
    let response = serde_bencode::de::from_bytes::<TrackerResponse>(&*response)?;
    return Ok(response);
}

fn create_url(torrent: &Torrent, peer_config: &Config) -> String {
    let port = peer_config.listening_port.to_string();
    let info_hash = byte_serialize(&torrent.info_hash).collect::<String>();
    let query_params = [
        ("info_hash", info_hash.as_ref()),
        ("peer_id", peer_config.client_id.as_ref()),
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

    return format!("{}?{}", torrent.announce, query_params);
}

fn deserialize_peers<'de, D>(deserializer: D) -> Result<Vec<Peer>, D::Error>
    where D: serde::Deserializer<'de>,
{
    let bytes = serde_bytes::ByteBuf::deserialize(deserializer)?;
    let mut iter = bytes.as_ref().chunks_exact(6);
    let mut peers = Vec::new();
    while let Some(chunk) = iter.next() {
        let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        let port = u16::from_be_bytes([chunk[4], chunk[5]]);
        peers.push(Peer { ip, port });
    }

    Ok(peers)
}