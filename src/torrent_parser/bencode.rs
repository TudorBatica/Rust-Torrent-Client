use crate::torrent_parser::file_reader::FileReader;

#[derive(Clone, Debug)]
pub enum BencodeElement {
    Int(u64),
    Str(String),
    List(Vec<BencodeElement>),
    Dict(Vec<(BencodeElement, BencodeElement)>),
}

pub fn decode_next_element(start_byte: u8, reader: &mut FileReader) -> BencodeElement {
    let start_char = start_byte as char;

    return if start_char == 'd' {
        decode_dict(reader)
    } else if start_char == 'l' {
        decode_list(reader)
    } else if start_char == 'i' {
        decode_int(reader)
    } else {
        decode_str(start_char, reader)
    };
}

fn decode_int(reader: &mut FileReader) -> BencodeElement {
    let mut num: u64 = 0;
    loop {
        let char = reader.next().expect("") as char;
        if char == 'e' {
            break;
        }
        num = num * 10 + char.to_digit(10).expect("") as u64;
    }
    return BencodeElement::Int(num);
}

fn decode_str(start: char, reader: &mut FileReader) -> BencodeElement {
    let mut str_len = start.to_digit(10).expect("");
    loop {
        let char = reader.next().expect("") as char;
        if char == ':' {
            break;
        }
        str_len = str_len * 10 + char.to_digit(10).expect("");
    }

    let mut str = String::new();
    while str_len > 0 {
        let char = reader.next().expect("") as char;
        str.push(char);
        str_len -= 1;
    }

    return BencodeElement::Str(str);
}

fn decode_list(reader: &mut FileReader) -> BencodeElement {
    let mut list_elements: Vec<BencodeElement> = Vec::new();
    loop {
        let next_byte = reader.next().expect("");
        if next_byte == b'e' {
            break;
        }
        list_elements.push(decode_next_element(next_byte, reader));
    }

    return BencodeElement::List(list_elements);
}

fn decode_dict(reader: &mut FileReader) -> BencodeElement {
    let mut key_value: Vec<(BencodeElement, BencodeElement)> = Vec::new();
    loop {
        let next_byte = reader.next().expect("");
        if next_byte == b'e' {
            break;
        }
        let key = decode_next_element(next_byte, reader);
        let value = decode_next_element(reader.next().expect(""), reader);
        key_value.push((key, value));
    }

    return BencodeElement::Dict(key_value);
}