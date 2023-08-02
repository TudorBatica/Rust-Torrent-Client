use serde_derive::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::fs;
use sha1::{Digest, Sha1};

#[derive(Debug, Deserialize, Serialize)]
pub struct File {
    length: u64,
    path: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Info {
    #[serde(default)]
    pub files: Option<Vec<File>>,
    #[serde(default)]
    pub length: Option<u64>,
    pub name: String,
    #[serde(default)]
    pub path: Option<Vec<String>>,
    pub pieces: ByteBuf,
    #[serde(rename = "piece length")]
    pub piece_length: u64,
    #[serde(default)]
    pub private: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct Torrent {
    pub info: Info,
    #[serde(default)]
    pub announce: String,
    #[serde(default)]
    #[serde(rename = "announce-list")]
    pub announce_list: Option<Vec<Vec<String>>>,
    #[serde(default)]
    #[serde(rename = "creation date")]
    pub creation_date: Option<u64>,
    #[serde(rename = "comment")]
    pub comment: Option<String>,
    #[serde(default)]
    #[serde(rename = "created by")]
    pub created_by: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub info_hash: Vec<u8>,
}

impl Torrent {
    pub fn get_pieces_hashes(&self) -> Vec<Vec<u8>> {
        return self.info.pieces.as_ref().chunks(20).map(|array| array.to_owned()).collect();
    }
}

pub fn parse_torrent(file_path: &str) -> Result<Torrent, Box<dyn std::error::Error>> {
    let file = fs::read(file_path)?;
    let mut torrent = serde_bencode::de::from_bytes::<Torrent>(&file)?;
    let mut hasher = Sha1::new();
    hasher.update(serde_bencode::ser::to_bytes(&torrent.info)?);
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
        let metadata = parse_torrent("test_resources/ubuntu-18.04.6-desktop-amd64.iso.torrent").unwrap();
        assert_eq!(metadata.info_hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
                   "bc26c6bc83d0ca1a7bf9875df1ffc3fed81ff555");
        assert_eq!(metadata.announce, "https://torrent.ubuntu.com/announce");
        assert_eq!(metadata.announce_list, Some(expected_trackers));
        assert_eq!(metadata.get_pieces_hashes().len(), 9591);
    }
}
