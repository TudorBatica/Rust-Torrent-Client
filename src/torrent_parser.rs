use std::collections::HashMap;
use std::ffi::c_ushort;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufReader, Bytes, Read};

#[derive(Debug)]
enum TorrentNode {
    Int(u32),
    Str(String),
    List(Vec<TorrentNode>),
    Dict(HashMap<TorrentNode, TorrentNode>),
}


impl PartialEq for TorrentNode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TorrentNode::Int(i1), TorrentNode::Int(i2)) => i1 == i2,
            (TorrentNode::Str(s1), TorrentNode::Str(s2)) => s1 == s2,
            (TorrentNode::List(l1), TorrentNode::List(l2)) => l1 == l2,
            (TorrentNode::Dict(d1), TorrentNode::Dict(d2)) => d1 == d2,
            _ => false,
        }
    }
}

impl Eq for TorrentNode {}

impl Hash for TorrentNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            TorrentNode::Int(i) => {
                i.hash(state);
            }
            TorrentNode::Str(s) => {
                s.hash(state);
            }
            TorrentNode::List(l) => {
                l.len().hash(state);
                for item in l {
                    item.hash(state);
                }
            }
            TorrentNode::Dict(d) => {
                d.len().hash(state);
                for (k, v) in d {
                    k.hash(state);
                    v.hash(state);
                }
            }
        }
    }
}

pub fn parse(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let mut bytes = BufReader::new(file).bytes();

    loop {
        match bytes.next() {
            None => { break; }
            Some(b) => {
                let node = parse_next_node(b?, &mut bytes);
                println!("{:?}", node);
            }
        }
    }

    return Ok(());
}

fn parse_next_node(start_byte: u8, bytes: &mut Bytes<BufReader<File>>) -> TorrentNode {
    let start_char = start_byte as char;

    return if start_char == 'd' {
        parse_dict(bytes)
    } else if start_char == 'l' {
        parse_list(bytes)
    } else if start_char == 'i' {
        parse_int(bytes)
    } else {
        parse_str(start_char, bytes)
    };
}

fn parse_int(bytes: &mut Bytes<BufReader<File>>) -> TorrentNode {
    let mut num: u32 = 0;
    loop {
        let byte = bytes.next().expect("Should have a byte").expect("No err expected");

        if byte == b'e' {
            break;
        }

        num = num * 10 + (byte as char).to_digit(10).expect("");
    }
    return TorrentNode::Int(num);
}

fn parse_str(start: char, bytes: &mut Bytes<BufReader<File>>) -> TorrentNode {
    let mut str_len = start.to_digit(10).expect("Should have had a digit");
    loop {
        let byte = bytes.next().expect("Should have a byte").expect("No err expected");

        if byte == b':' {
            break;
        }

        str_len = str_len * 10 + (byte as char).to_digit(10).expect("");
    }

    let mut str = String::new();
    while str_len > 0 {
        let char = bytes.next().expect("Should have a byte").expect("No err expected");
        str.push(char as char);
        str_len -= 1;
    }

    return TorrentNode::Str(str);
}

fn parse_list(bytes: &mut Bytes<BufReader<File>>) -> TorrentNode {
    let mut list_elements: Vec<TorrentNode> = Vec::new();

    loop {
        let next_byte = bytes.next().expect("Should have a byte").expect("No err expected");
        if next_byte == b'e' {
            break;
        }
        list_elements.push(parse_next_node(next_byte, bytes));
    }

    return TorrentNode::List(list_elements);
}

fn parse_dict(bytes: &mut Bytes<BufReader<File>>) -> TorrentNode {
    let mut map = HashMap::new();

    loop {
        let next_byte = bytes.next().expect("Should have a byte").expect("No err expected");

        if next_byte == b'e' {
            break;
        }

        let key = parse_next_node(next_byte, bytes);
        let next_byte = bytes.next().expect("Should have a byte").expect("No err expected");
        let value = parse_next_node(next_byte, bytes);

        map.insert(key, value);
    }

    return TorrentNode::Dict(map);
}

