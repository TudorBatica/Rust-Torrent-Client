use rand::distributions::Alphanumeric;
use rand::Rng;

const CLIENT_ID_PREFIX: &str = "XX0001x";
pub const BLOCK_SIZE_BYTES: usize = 16384;

#[derive(Clone)]
pub struct Config {
    pub listening_port: u16,
    pub client_id: String,
}

impl Config {
    pub fn init() -> Self {
        return Config {
            listening_port: 42000,
            client_id: Config::generate_client_id(),
        };
    }

    fn generate_client_id() -> String {
        let suffix: String = rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(20 - CLIENT_ID_PREFIX.len())
            .map(char::from)
            .collect();

        return format!("{}{}", CLIENT_ID_PREFIX, suffix);
    }
}
