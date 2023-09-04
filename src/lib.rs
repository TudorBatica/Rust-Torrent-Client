pub mod config;
pub mod metadata;
pub mod tracker;
pub mod piece_picker;
pub mod coordinator;
pub mod state;
pub mod file_provider;
pub mod mocks; //todo: might have to move to tests/

pub mod core_models {
    pub mod entities;
    pub mod events;
    pub mod internal_events;
}

pub mod data_collector {
    pub mod new_runner;
    pub mod runner;
    mod handler;
    mod state;
}

pub mod p2p {
    pub mod connection;
    pub mod transfer;
}