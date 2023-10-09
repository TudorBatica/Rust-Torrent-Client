use std::error::Error;
use crate::config::Config;
use crate::core_models::entities::{Peer, Torrent};
use form_urlencoded::byte_serialize;
use serde::Deserialize;
use serde_derive::Deserialize;
use std::net::Ipv4Addr;
use async_trait::async_trait;
use mockall::automock;

#[async_trait]
#[automock]
pub trait TrackerClient {
    async fn announce(&self, event: TrackerRequestEvent) -> Result<TrackerResponse, Box<dyn Error>>;
}

pub enum TrackerRequestEvent {
    Started,
    Regular(usize, usize),
    Completed(usize, usize),
}

impl TrackerRequestEvent {
    fn name(&self) -> String {
        return match self {
            TrackerRequestEvent::Started => "started".to_string(),
            TrackerRequestEvent::Regular(_, _) => "".to_string(),
            TrackerRequestEvent::Completed(_, _) => "completed".to_string()
        };
    }

    fn downloaded(&self) -> String {
        return match self {
            TrackerRequestEvent::Started => "0".to_string(),
            TrackerRequestEvent::Regular(downloaded, _) => downloaded.to_string(),
            TrackerRequestEvent::Completed(downloaded, _) => downloaded.to_string()
        };
    }

    fn uploaded(&self) -> String {
        return match self {
            TrackerRequestEvent::Started => "0".to_string(),
            TrackerRequestEvent::Regular(_, uploaded) => uploaded.to_string(),
            TrackerRequestEvent::Completed(_, uploaded) => uploaded.to_string()
        };
    }
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

    return Ok(peers);
}

pub struct TorrentTrackerClient {
    pub announce_url: String,
    pub client_config: Config,
    pub info_hash: Vec<u8>,
}

impl TorrentTrackerClient {
    pub fn new(torrent: &Torrent, config: Config) -> Self {
        return TorrentTrackerClient {
            announce_url: torrent.announce.clone(),
            client_config: config,
            info_hash: torrent.info_hash.clone(),
        };
    }

    fn create_url(&self, event: TrackerRequestEvent) -> String {
        let query_params = [
            ("info_hash", byte_serialize(&self.info_hash).collect::<String>()),
            ("peer_id", self.client_config.client_id.clone()),
            ("port", self.client_config.listening_port.to_string()),
            ("compact", "1".to_string()),
            ("event", event.name()),
            ("downloaded", event.downloaded()),
            ("uploaded", event.uploaded()),
            ("numwant", "300".to_string())
        ];
        let query_params = query_params.into_iter()
            .filter(|(_key, value)| !value.is_empty())
            .enumerate()
            .map(|(param_idx, (param, value))| {
                if param_idx == 0 {
                    format!("{}={}", param, value)
                } else {
                    format!("&{}={}", param, value)
                }
            })
            .collect::<String>();

        return format!("{}?{}", self.announce_url, query_params);
    }
}

#[async_trait]
impl TrackerClient for TorrentTrackerClient {
    async fn announce(&self, event: TrackerRequestEvent) -> Result<TrackerResponse, Box<dyn Error>> {
        let url = self.create_url(event);
        let response = reqwest::get(url).await?.bytes().await?;
        let response = serde_bencode::de::from_bytes::<TrackerResponse>(&*response)?;
        return Ok(response);
    }
}