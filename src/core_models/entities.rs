//todo: replace all usages of (usize, usize, usize) w/ block

#[derive(Clone, Default, Debug, Eq, Hash, PartialEq)]
pub struct BlockPosition {
    pub piece_idx: usize,
    pub offset: usize,
    pub length: usize,
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
}

impl TorrentLayout {
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
    Request(usize, usize, usize),
    Piece(usize, usize, Vec<u8>),
    Cancel(usize, usize, usize),
    Port(usize),
}

//todo: add tests
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
                let index = Self::usize_from_be_bytes(bytes[1..5].to_vec());
                let begin = Self::usize_from_be_bytes(bytes[5..9].to_vec());
                let length = Self::usize_from_be_bytes(bytes[9..13].to_vec());
                return Some(Message::Request(index, begin, length));
            }
            7 => {
                let index = Self::usize_from_be_bytes(bytes[1..5].to_vec());
                let begin = Self::usize_from_be_bytes(bytes[5..9].to_vec());
                return Some(Message::Piece(index, begin, bytes[9..].to_vec()));
            }
            8 => {
                let index = Self::usize_from_be_bytes(bytes[1..5].to_vec());
                let begin = Self::usize_from_be_bytes(bytes[5..9].to_vec());
                let length = Self::usize_from_be_bytes(bytes[9..13].to_vec());
                return Some(Message::Cancel(index, begin, length));
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
            Message::Request(index, offset, length) => {
                bytes.push(6);
                bytes.append(&mut Self::usize_to_four_be_bytes(*index));
                bytes.append(&mut Self::usize_to_four_be_bytes(*offset));
                bytes.append(&mut Self::usize_to_four_be_bytes(*length));
            }
            Message::Piece(index, offset, block) => {
                bytes.push(7);
                bytes.append(&mut Self::usize_to_four_be_bytes(*index));
                bytes.append(&mut Self::usize_to_four_be_bytes(*offset));
                bytes.extend(block.into_iter());
            }
            Message::Cancel(index, offset, length) => {
                bytes.push(8);
                bytes.append(&mut Self::usize_to_four_be_bytes(*index));
                bytes.append(&mut Self::usize_to_four_be_bytes(*offset));
                bytes.append(&mut Self::usize_to_four_be_bytes(*length));
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

//todo: might need to move Bitfield somewhere else
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
}
