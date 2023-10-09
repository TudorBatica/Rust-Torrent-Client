pub mod choke {
    pub mod handler;
    pub mod models;
    pub mod task;
}

pub mod coordinator {
    pub mod ipc;
    pub mod task;
}

pub mod core_models {
    pub mod entities;
    pub mod events;
}

pub mod p2p {
    pub mod conn;
    pub mod handlers;
    pub mod models;
    pub mod task;
}

pub mod tracker {
    pub mod client;
    pub mod task;
}

pub mod config;
pub mod data_collector;
pub mod dependency_provider;
pub mod file_provider;
pub mod mocks;
pub mod piece_picker;
pub mod torrent_parser;







