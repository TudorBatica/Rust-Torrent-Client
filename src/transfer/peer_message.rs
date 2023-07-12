pub struct Bitfield {
    content: Vec<u8>,
}

impl Bitfield {
    pub fn from(pieces: Vec<bool>) -> Self {
        let length = (pieces.len() as f64 / 8.0).ceil() as usize;
        let mut content = vec![0u8; length];
        for (piece_idx, &piece_available) in pieces.iter().enumerate() {
            if piece_available {
                let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
                let mask = 1 << (7 - bit_idx);
                content[byte_idx] |= mask;
            }
        }
        Bitfield { content }
    }

    pub fn piece_acquired(&mut self, piece_idx: u64) {
        let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
        let mask = 1 << (7 - bit_idx);
        self.content[byte_idx as usize] |= mask;
    }

    pub fn has_piece(&self, piece_idx: u64) -> bool {
        let (byte_idx, bit_idx) = (piece_idx / 8, piece_idx % 8);
        self.content[byte_idx as usize] & (1 << (7 - bit_idx)) != 0
    }
}

#[cfg(test)]
mod tests {
    use crate::transfer::peer_message::Bitfield;

    #[test]
    pub fn bitfield_initialization_test() {
        let pieces: Vec<bool> = vec![false, true, true, false];
        let bitfield: Bitfield = Bitfield::from(pieces);
        assert_eq!(bitfield.content.len(), 1); // 1 byte
        assert_eq!(bitfield.content[0], 96); // internal representation is 0110 0000
    }

    #[test]
    pub fn bitfield_has_test() {
        let pieces: Vec<bool> = vec![false, true, true, false];
        let bitfield: Bitfield = Bitfield::from(pieces);
        assert_eq!(bitfield.has_piece(1) && bitfield.has_piece(2), true);
        assert_eq!(!bitfield.has_piece(0) && !bitfield.has_piece(3), true);
    }

    #[test]
    pub fn bitfield_update_test() {
        let pieces: Vec<bool> = vec![false, true, true, false];
        let mut bitfield: Bitfield = Bitfield::from(pieces);
        bitfield.piece_acquired(3);
        assert_eq!(bitfield.content[0], 112);
    }
}
