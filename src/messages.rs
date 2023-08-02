const PROTOCOL: &'static str = "BitTorrent protocol";

//todo: move into transfer directory

pub struct Handshake {
    pub content: Vec<u8>,
}

impl Handshake {
    pub fn new(info_hash: &Vec<u8>, client_id: &str) -> Self {
        let mut content: Vec<u8> = Vec::with_capacity(49 + PROTOCOL.len());
        //pstrlen
        content.push(PROTOCOL.len() as u8);
        //pstr
        content.extend(PROTOCOL.bytes());
        //reserved bytes
        content.extend(vec![0; 8].into_iter());
        //info hash of desired torrent
        content.extend(info_hash);
        //client id
        content.extend(client_id.bytes());

        return Handshake { content };
    }
}