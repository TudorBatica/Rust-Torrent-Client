mod bencode;
mod file_reader;
mod metadata;

use std::fs::{File};
use std::io::{Read, Seek, SeekFrom};
use sha1::{Digest, Sha1};

use bencode::BencodeElement;
use file_reader::FileReader;
use metadata::{TorrentMeta, Info};

pub fn parse(file_path: &str) -> TorrentMeta {
    let file = File::open(file_path).expect("Could not open file");
    let mut reader = FileReader::new(file);
    if reader.next().expect("") != b'd' {
        panic!("Not a valid .torrent file, it is not a bencoded dict");
    }

    let (mut torrent_meta, info_dict_start_idx, info_dict_end_idx) = parse_metadata(reader);
    let file = File::open(file_path).expect("could not reopen file");
    let info_hash = compute_info_hash(file, info_dict_start_idx, info_dict_end_idx);
    torrent_meta.info_hash = info_hash;

    return torrent_meta;
}

fn parse_metadata(mut reader: FileReader) -> (TorrentMeta, u64, u64) {
    let mut torrent_meta = TorrentMeta::default();
    let mut info_dict_start_index: u64 = Default::default();
    let mut info_dict_end_index: u64 = Default::default();

    loop {
        match reader.next() {
            None => { break; }
            Some(b) => {
                if b == b'e' {
                    break;
                }
                let key = bencode::decode_next_element(b, &mut reader);
                let key: String = match key {
                    BencodeElement::Str(str) => { str }
                    _ => { panic!("Not a valid .torrent file, all top-lvl dict keys should be strings") }
                };

                if key == "info" {
                    info_dict_start_index = reader.bytes_read;
                    torrent_meta.info = parse_info_dict(&mut reader);
                    info_dict_end_index = reader.bytes_read;
                } else {
                    let value = bencode::decode_next_element(reader.next().expect(""), &mut reader);
                    metadata::map_metadata_field(key, value, &mut torrent_meta);
                }
            }
        }
    }

    return (torrent_meta, info_dict_start_index, info_dict_end_index);
}

fn parse_info_dict(reader: &mut FileReader) -> Info {
    let first_byte = reader.next().unwrap();
    if first_byte != b'd' {
        panic!("Invalid .torrent file, Info field is not a dictionary");
    }
    let mut info = Info::default();

    loop {
        let next_byte = reader.next().expect("");
        if next_byte == b'e' {
            break;
        }
        let key = match bencode::decode_next_element(next_byte, reader) {
            BencodeElement::Str(str) => { str }
            _ => { panic!("Not a valid .torrent file. All info dict keys should be of type str") }
        };
        let value = bencode::decode_next_element(reader.next().expect(""), reader);
        metadata::map_info_field(key, value, &mut info);
    }

    return info;
}

fn compute_info_hash(mut file: File, start_idx: u64, end_idx: u64) -> String {
    file.seek(SeekFrom::Start(start_idx)).expect("could not seek");
    let mut buffer = vec![0; (end_idx - start_idx) as usize];
    file.read_exact(&mut buffer).expect("could not read :(");

    let mut hasher = Sha1::new();
    hasher.update(buffer);
    let info_hash = hasher.finalize();

    return info_hash.iter().map(|byte| format!("{:02x}", byte)).collect::<String>();
}

#[cfg(test)]
mod tests {
    use crate::torrent_parser::parse;

    #[test]
    pub fn parse_torrent() {
        let expected_trackers = vec![
            vec!["https://torrent.ubuntu.com/announce".to_string()],
            vec!["https://ipv6.torrent.ubuntu.com/announce".to_string()]
        ];

        let metadata = parse("test_resources/ubuntu-18.04.6-desktop-amd64.iso.torrent");
        assert_eq!(metadata.info_hash, "bc26c6bc83d0ca1a7bf9875df1ffc3fed81ff555");
        assert_eq!(metadata.announce, "https://torrent.ubuntu.com/announce".to_string());
        assert_eq!(metadata.announce_list, expected_trackers);
    }
}
