use std::fs;
use sha1::{Digest, Sha1};
use crate::core_models::entities::Torrent;

pub fn parse_torrent(file_path: &str) -> Result<Torrent, Box<dyn std::error::Error>> {
    let file = fs::read(file_path)?;
    let mut torrent = serde_bencode::de::from_bytes::<Torrent>(&file)?;
    let mut hasher = Sha1::new();
    hasher.update(serde_bencode::ser::to_bytes(&torrent.info)?);
    torrent.info_hash = hasher.finalize().into_iter().collect();
    torrent.piece_hashes = torrent.info.pieces.as_ref().chunks(20).map(|array| array.to_owned()).collect();
    return Ok(torrent);
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_torrent;

    #[test]
    pub fn test_torrent_parse() {
        let expected_trackers = vec![
            vec!["https://torrent.ubuntu.com/announce".to_string()],
            vec!["https://ipv6.torrent.ubuntu.com/announce".to_string()],
        ];
        let metadata = parse_torrent("test_resources/ubuntu-18.04.6-desktop-amd64.iso.torrent").unwrap();
        assert_eq!(metadata.info_hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>(),
                   "bc26c6bc83d0ca1a7bf9875df1ffc3fed81ff555");
        assert_eq!(metadata.announce, "https://torrent.ubuntu.com/announce");
        assert_eq!(metadata.announce_list, Some(expected_trackers));
        assert_eq!(metadata.piece_hashes.len(), 9591);
    }
}
