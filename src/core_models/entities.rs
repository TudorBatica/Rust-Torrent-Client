use serde_derive::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::net::Ipv4Addr;
use crate::config;

#[derive(Debug, Deserialize)]
pub struct Peer {
    pub ip: Ipv4Addr,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct File {
    length: u64,
    path: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize)]
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
    #[serde(skip)]
    pub piece_hashes: Vec<Vec<u8>>,
}

#[derive(Clone, Default, Debug, Eq, Hash, PartialEq)]
pub struct Block {
    pub piece_idx: usize,
    pub offset: usize,
    pub length: usize,
}

impl Block {
    pub fn new(piece_idx: usize, offset: usize, length: usize) -> Self {
        return Block { piece_idx, offset, length };
    }
}

#[derive(Clone, Default, Debug)]
pub struct DataBlock {
    pub piece_idx: usize,
    pub offset: usize,
    pub data: Vec<u8>,
}

impl DataBlock {
    pub fn new(piece_idx: usize, offset: usize, data: Vec<u8>) -> Self {
        return DataBlock { piece_idx, offset, data };
    }

    pub fn to_block(&self) -> Block {
        return Block {
            piece_idx: self.piece_idx,
            offset: self.offset,
            length: self.data.len(),
        };
    }
}

#[derive(Clone, Debug)]
pub struct TorrentLayout {
    pub pieces: usize,
    pub head_pieces_length: usize,
    pub last_piece_length: usize,
    pub blocks_in_head_pieces: usize,
    pub blocks_in_last_piece: usize,
    pub usual_block_length: usize,
    pub head_pieces_last_block_length: usize,
    pub last_piece_last_block_length: usize,
    pub output_file_path: String,
    pub output_file_length: usize,
}

impl TorrentLayout {
    pub fn from_torrent(torrent: &Torrent) -> Self {
        let total_length = torrent.info.length.expect("only single file torrents supported") as usize;
        let pieces = torrent.piece_hashes.len();
        let head_pieces_length = torrent.info.piece_length as usize;
        let last_piece_length = total_length - head_pieces_length * (pieces - 1);

        let usual_block_length = config::BLOCK_SIZE_BYTES;
        let blocks_in_head_pieces = (head_pieces_length as f64 / (usual_block_length as f64)).ceil() as usize;
        let blocks_in_last_piece = (last_piece_length as f64 / (usual_block_length as f64)).ceil() as usize;
        let head_pieces_last_block_length = head_pieces_length - (blocks_in_head_pieces - 1) * config::BLOCK_SIZE_BYTES;
        let last_piece_last_block_length = last_piece_length as usize - (blocks_in_last_piece as usize - 1) * config::BLOCK_SIZE_BYTES;

        return TorrentLayout {
            pieces,
            head_pieces_length,
            last_piece_length,
            blocks_in_head_pieces,
            blocks_in_last_piece,
            usual_block_length,
            head_pieces_last_block_length,
            last_piece_last_block_length,
            output_file_path: torrent.info.name.clone(),
            output_file_length: torrent.info.length.expect("only single file torrent supported") as usize,
        };
    }

    pub fn blocks_in_piece(&self, piece_idx: usize) -> usize {
        return if piece_idx == self.pieces - 1 {
            self.blocks_in_last_piece
        } else {
            self.blocks_in_head_pieces
        };
    }

    pub fn piece_length(&self, piece_idx: usize) -> usize {
        return if piece_idx == self.pieces - 1 {
            self.last_piece_length
        } else {
            self.head_pieces_length
        };
    }

    pub fn last_block_length_for_piece(&self, piece_idx: usize) -> usize {
        return if piece_idx == self.pieces - 1 {
            self.last_piece_last_block_length
        } else {
            self.head_pieces_last_block_length
        };
    }

    pub fn block_length(&self, piece_idx: usize, block_idx: usize) -> usize {
        return if block_idx == self.blocks_in_piece(piece_idx) - 1 {
            self.last_block_length_for_piece(piece_idx)
        } else {
            self.usual_block_length
        };
    }
}

#[derive(Clone)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(usize),
    Bitfield(Vec<u8>),
    Request(Block),
    Piece(usize, usize, Vec<u8>),
    Cancel(Block),
    Port(usize),
}

impl Message {
    pub fn deserialize(bytes: Vec<u8>) -> Option<Self> {
        match bytes[0] {
            0 => Some(Message::Choke),
            1 => Some(Message::Unchoke),
            2 => Some(Message::Interested),
            3 => Some(Message::NotInterested),
            4 => {
                let piece = Self::usize_from_be_bytes(bytes[1..].to_vec());
                return Some(Message::Have(piece));
            }
            5 => Some(Message::Bitfield(bytes[1..].to_vec())),
            6 => {
                let piece_idx = Self::usize_from_be_bytes(bytes[1..5].to_vec());
                let offset = Self::usize_from_be_bytes(bytes[5..9].to_vec());
                let length = Self::usize_from_be_bytes(bytes[9..13].to_vec());
                return Some(Message::Request(Block::new(piece_idx, offset, length)));
            }
            7 => {
                let index = Self::usize_from_be_bytes(bytes[1..5].to_vec());
                let begin = Self::usize_from_be_bytes(bytes[5..9].to_vec());
                return Some(Message::Piece(index, begin, bytes[9..].to_vec()));
            }
            8 => {
                let piece_idx = Self::usize_from_be_bytes(bytes[1..5].to_vec());
                let offset = Self::usize_from_be_bytes(bytes[5..9].to_vec());
                let length = Self::usize_from_be_bytes(bytes[9..13].to_vec());
                return Some(Message::Request(Block::new(piece_idx, offset, length)));
            }
            9 => {
                let port = Self::usize_from_be_bytes(bytes[1..].to_vec());
                return Some(Message::Port(port));
            }
            _ => None
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            Message::KeepAlive => {}
            Message::Choke => bytes.push(0),
            Message::Unchoke => bytes.push(1),
            Message::Interested => bytes.push(2),
            Message::NotInterested => bytes.push(3),
            Message::Have(index) => {
                bytes.push(4);
                bytes.append(&mut Self::usize_to_four_be_bytes(*index));
            }
            Message::Bitfield(bitfield) => {
                bytes.push(5);
                bytes.extend(bitfield.into_iter());
            }
            Message::Request(block) => {
                bytes.push(6);
                bytes.append(&mut Self::usize_to_four_be_bytes(block.piece_idx));
                bytes.append(&mut Self::usize_to_four_be_bytes(block.offset));
                bytes.append(&mut Self::usize_to_four_be_bytes(block.length));
            }
            Message::Piece(index, offset, block) => {
                bytes.push(7);
                bytes.append(&mut Self::usize_to_four_be_bytes(*index));
                bytes.append(&mut Self::usize_to_four_be_bytes(*offset));
                bytes.extend(block.into_iter());
            }
            Message::Cancel(block) => {
                bytes.push(8);
                bytes.append(&mut Self::usize_to_four_be_bytes(block.piece_idx));
                bytes.append(&mut Self::usize_to_four_be_bytes(block.offset));
                bytes.append(&mut Self::usize_to_four_be_bytes(block.length));
            }
            Message::Port(port) => {
                bytes.push(9);
                bytes.append(&mut Self::usize_to_four_be_bytes(*port));
            }
        }

        return bytes;
    }

    fn usize_from_be_bytes(bytes: Vec<u8>) -> usize {
        return bytes.into_iter()
            .rev().enumerate()
            .map(|(idx, byte)| 256usize.pow(idx as u32) * byte as usize)
            .sum();
    }

    fn usize_to_four_be_bytes(number: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(4);
        result[0] = ((number >> 24) & 0xFF) as u8;
        result[1] = ((number >> 16) & 0xFF) as u8;
        result[2] = ((number >> 8) & 0xFF) as u8;
        result[3] = (number & 0xFF) as u8;

        return result;
    }
}

#[derive(Clone)]
pub struct Bitfield {
    content: Vec<u8>,
}

impl Bitfield {
    pub fn new(bytes: Vec<u8>) -> Self {
        return Bitfield { content: bytes };
    }

    pub fn init(num_of_pieces: usize) -> Self {
        let length_in_bytes = (num_of_pieces as f64 / 8.0).ceil() as usize;
        return Bitfield { content: vec![0u8; length_in_bytes] };
    }

    pub fn piece_acquired(&mut self, piece_idx: usize) {
        let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
        let mask = 1 << (7 - bit_idx);
        self.content[byte_idx as usize] |= mask;
    }

    pub fn has_piece(&self, piece_idx: usize) -> bool {
        let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
        self.content[byte_idx as usize] & (1 << (7 - bit_idx)) != 0
    }

    // returns whether `self` has any pieces that `other` does not
    pub fn has_any_missing_pieces_from(&self, other: &Bitfield) -> bool {
        let other_not: Vec<u8> = other.content.clone()
            .into_iter()
            .map(|byte| !byte)
            .collect();

        for (other_byte, self_byte) in other_not.iter().zip(self.content.iter()) {
            if (other_byte & self_byte) != 0 {
                return true;
            }
        }

        return false;
    }
}


#[cfg(test)]
mod tests {
    use crate::core_models::entities::Bitfield;

    #[test]
    pub fn bitfield_initialization_test() {
        let bitfield: Bitfield = Bitfield::init(4);
        assert_eq!(bitfield.content.len(), 1); // 1 byte
        assert_eq!(bitfield.content[0], 0);
    }

    #[test]
    pub fn bitfield_update_test() {
        let mut bitfield: Bitfield = Bitfield::init(4);
        bitfield.piece_acquired(3);
        bitfield.piece_acquired(1);
        bitfield.piece_acquired(2);
        assert_eq!(bitfield.content[0], 112); // internal representation should be 01110000
    }

    #[test]
    pub fn bitfield_has_test() {
        let mut bitfield: Bitfield = Bitfield::init(4);
        bitfield.piece_acquired(1);
        bitfield.piece_acquired(2);
        assert_eq!(bitfield.has_piece(1) && bitfield.has_piece(2), true);
        assert_eq!(!bitfield.has_piece(0) && !bitfield.has_piece(3), true);
    }

    #[test]
    pub fn bitfield_has_any_missing_pieces_from_test() {
        let mut bitfield1 = Bitfield::init(4);
        let mut bitfield2 = Bitfield::init(4);
        assert_eq!(bitfield1.has_any_missing_pieces_from(&bitfield2), false);
        assert_eq!(bitfield2.has_any_missing_pieces_from(&bitfield1), false);

        bitfield2.piece_acquired(1);
        assert_eq!(bitfield1.has_any_missing_pieces_from(&bitfield2), false);
        assert_eq!(bitfield2.has_any_missing_pieces_from(&bitfield1), true);

        bitfield1.piece_acquired(1);
        assert_eq!(bitfield1.has_any_missing_pieces_from(&bitfield2), false);
        assert_eq!(bitfield2.has_any_missing_pieces_from(&bitfield1), false);

        bitfield1.piece_acquired(3);
        assert_eq!(bitfield1.has_any_missing_pieces_from(&bitfield2), true);
        assert_eq!(bitfield2.has_any_missing_pieces_from(&bitfield1), false);
    }
}
