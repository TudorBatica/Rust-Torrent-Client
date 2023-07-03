use serde_derive::{Deserialize, Serialize};
use serde_bencode::{de, ser};
use serde_bytes::ByteBuf;
use std::fs;
use sha1::{Digest, Sha1};

#[derive(Debug, Deserialize)]
struct Node(String, i64);

#[derive(Debug, Deserialize, Serialize)]
struct File {
    path: Vec<String>,
    length: i64,
    #[serde(default)]
    md5sum: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Info {
    name: String,
    pieces: ByteBuf,
    #[serde(rename = "piece length")]
    piece_length: i64,
    #[serde(default)]
    md5sum: Option<String>,
    #[serde(default)]
    length: Option<i64>,
    #[serde(default)]
    files: Option<Vec<File>>,
    #[serde(default)]
    private: Option<u8>,
    #[serde(default)]
    path: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "root hash")]
    root_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Torrent {
    info: Info,
    #[serde(default)]
    pub announce: Option<String>,
    #[serde(default)]
    nodes: Option<Vec<Node>>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub httpseeds: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "announce-list")]
    pub announce_list: Option<Vec<Vec<String>>>,
    #[serde(default)]
    #[serde(rename = "creation date")]
    pub creation_date: Option<i64>,
    #[serde(rename = "comment")]
    pub comment: Option<String>,
    #[serde(default)]
    #[serde(rename = "created by")]
    pub created_by: Option<String>,
    #[serde(default)]
    pub info_hash: Vec<u8>,
}

pub fn parse_torrent(file_path: &str) -> Result<Torrent, Box<dyn std::error::Error>> {
    let file = fs::read(file_path)?;
    let mut torrent = de::from_bytes::<Torrent>(&file)?;
    let mut hasher = Sha1::new();
    hasher.update(ser::to_bytes(&torrent.info)?);
    torrent.info_hash = hasher.finalize().into_iter().collect();
    return Ok(torrent);
}

#[cfg(test)]
mod tests {
    use crate::metadata::parse_torrent;

    #[test]
    pub fn parse_test() {
        let expected_trackers = vec![
            vec!["https://torrent.ubuntu.com/announce".to_string()],
            vec!["https://ipv6.torrent.ubuntu.com/announce".to_string()],
        ];

        let metadata = parse_torrent(
            "test_resources/ubuntu-18.04.6-desktop-amd64.iso.torrent"
        ).unwrap();
        assert_eq!(metadata.info_hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
                   "bc26c6bc83d0ca1a7bf9875df1ffc3fed81ff555");
        assert_eq!(metadata.announce, Some("https://torrent.ubuntu.com/announce".to_string()));
        assert_eq!(metadata.announce_list, Some(expected_trackers));
    }
}
