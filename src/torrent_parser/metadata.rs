use std::path::Path;
use crate::torrent_parser::bencode::BencodeElement;

#[derive(Debug, Default)]
pub struct TorrentMeta {
    pub announce: String,
    pub created_by: String,
    pub creation_date: u32,
    pub comment: String,
    pub encoding: String,
    pub info: Info,
    pub info_hash: String,
}

#[derive(Debug, Default)]
pub struct Info {
    piece_length: u64,
    pieces: String,
    name: String,
    length: Option<u64>,
    files: Option<Vec<(String, u64)>>,
}

pub fn map_info_field(key: String, value: BencodeElement, info: &mut Info) {
    if key == "name" {
        match value {
            BencodeElement::Str(value) => { info.name = value }
            _ => { panic!("Not a valid value for info.name field") }
        };
    } else if key == "piece length" {
        match value {
            BencodeElement::Int(value) => { info.piece_length = value }
            _ => { panic!("Not a valid value for info.value by field") }
        };
    } else if key == "pieces" {
        match value {
            BencodeElement::Str(value) => { info.pieces = value }
            _ => { panic!("Not a valid value for info.pieces field") }
        };
    } else if key == "length" {
        match value {
            BencodeElement::Int(value) => { info.length = Some(value) }
            _ => { panic!("Not a valid value for info.length field") }
        };
    } else if key == "files" {
        let files_list = match value {
            BencodeElement::List(list) => { list }
            _ => { panic!("Not a valid value for info.files field") }
        };

        let mut files_vec: Vec<(String, u64)> = Vec::new();

        for dict in files_list {
            let dict = match dict {
                BencodeElement::Dict(dict) => { dict }
                _ => { panic!("Info.files list contains non-dictionary elements") }
            };

            let mut file_path_and_size: (String, u64) = Default::default();

            for (key, value) in dict {
                let key = match key {
                    BencodeElement::Str(key) => { key }
                    _ => { panic!("Info.files contains a dictionary with non-string key") }
                };
                if key == "length" {
                    match value {
                        BencodeElement::Int(length) => { file_path_and_size.1 = length }
                        _ => { panic!("Invalid value for info.files.length") }
                    }
                } else if key == "path" {
                    let path_list = match value {
                        BencodeElement::List(list) => { list }
                        _ => { panic!("Invalid value for info.files.path") }
                    };
                    let mut path = Path::new(".").to_path_buf();
                    path_list.into_iter()
                        .for_each(|p|
                            match p {
                                BencodeElement::Str(str) => { path = path.join(Path::new(&str)); }
                                _ => { panic!("Invalud value for info.files.path") }
                            }
                        );
                    match path.to_str() {
                        None => panic!("new path is not a valid UTF-8 sequence"),
                        Some(s) => file_path_and_size.0 = s.to_string(),
                    }
                }
            }

            files_vec.push(file_path_and_size);
        }

        info.files = Some(files_vec);
    }
}

pub fn map_metadata_field(key: String, value: BencodeElement, metadata: &mut TorrentMeta) {
    if key == "announce" {
        let value = match value {
            BencodeElement::Str(value) => { value }
            _ => { panic!("Not a valid value for announce field") }
        };
        metadata.announce = value;
    } else if key == "created by" {
        let value = match value {
            BencodeElement::Str(value) => { value }
            _ => { panic!("Not a valid value for created by field") }
        };
        metadata.created_by = value;
    } else if key == "comment" {
        let value = match value {
            BencodeElement::Str(value) => { value }
            _ => { panic!("Not a valid value for comment field") }
        };
        metadata.comment = value;
    } else if key == "creation date" {
        let value = match value {
            BencodeElement::Int(value) => { value }
            _ => { panic!("Not a valid value for creation date field") }
        };
        metadata.creation_date = value as u32;
    } else if key == "encoding" {
        let value = match value {
            BencodeElement::Str(value) => { value }
            _ => { panic!("Not a valid value for encoding field") }
        };
        metadata.encoding = value;
    }
}
