const PROTOCOL: &'static str = "BitTorrent protocol";

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
