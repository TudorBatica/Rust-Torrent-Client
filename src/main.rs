use std::collections::HashMap;

mod torrent_parser;

fn main() {
    torrent_parser::parse("test.torrent").expect("");
}
