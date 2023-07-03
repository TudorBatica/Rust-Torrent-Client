use rand::distributions::Alphanumeric;
use rand::Rng;

const PEER_ID_PREFIX: &str = "XX0001x";

pub struct Config {
    pub listening_port: u16,
    pub peer_id: String,
}

impl Config {
    pub fn init() -> Self {
        return Config {
            listening_port: 6882,
            peer_id: Config::generate_peer_id(),
        };
    }

    fn generate_peer_id() -> String {
        let peer_id_suffix: String = rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(20 - PEER_ID_PREFIX.len())
            .map(char::from)
            .collect();

        return format!("{}{}", PEER_ID_PREFIX, peer_id_suffix);
    }
}
