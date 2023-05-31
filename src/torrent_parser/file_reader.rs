use std::fs::File;
use std::io::{BufReader, Bytes, Read};

pub struct FileReader {
    pub bytes_read: u64,
    bytes: Bytes<BufReader<File>>,
}

impl FileReader {
    pub fn new(file: File) -> FileReader {
        return FileReader {
            bytes_read: 0,
            bytes: BufReader::new(file).bytes(),
        };
    }

    pub fn next(&mut self) -> Option<u8> {
        self.bytes_read += 1;
        return match self.bytes.next() {
            None => { None }
            Some(byte) => { Some(byte.expect("")) }
        };
    }
}