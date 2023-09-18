use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use crate::config::Config;
use crate::core_models::entities::{Torrent, TorrentLayout};
use crate::core_models::events::InternalEvent;
use crate::file_provider::{FileProv, TokioFileProv};
use crate::p2p::conn::{PeerConnector, TCPPeerConnector};
use crate::piece_picker::{PiecePicker, RarestPiecePicker};
use crate::tracker::{TorrentTrackerClient, TrackerClient};

pub trait TransferDeps: Send + Sync {
    fn announce_url(&self) -> String;
    fn client_config(&self) -> Config;
    fn file_provider(&self) -> Box<dyn FileProv>;
    fn info_hash(&self) -> Vec<u8>;
    fn output_tx(&self) -> Sender<InternalEvent>;
    fn peer_connector(&self) -> Box<dyn PeerConnector>;
    fn piece_hashes(&self) -> Vec<Vec<u8>>;
    fn piece_picker(&self) -> Arc<Mutex<dyn PiecePicker>>;
    fn torrent_layout(&self) -> TorrentLayout;
    fn tracker_client(&self) -> Box<dyn TrackerClient>;
}

pub struct DependencyProvider {
    client_config: Config,
    torrent: Torrent,
    layout: TorrentLayout,
    tx_to_coordinator: Sender<InternalEvent>,
    piece_picker: Arc<Mutex<dyn PiecePicker>>,
}

impl DependencyProvider {
    pub fn init(client_config: Config,
                torrent: Torrent, layout: TorrentLayout,
                tx_to_coordinator: Sender<InternalEvent>) -> Self {
        let picker = RarestPiecePicker::init(layout.clone());

        return DependencyProvider {
            client_config,
            torrent,
            layout,
            tx_to_coordinator,
            piece_picker: Arc::new(Mutex::new(picker)),
        };
    }
}

impl TransferDeps for DependencyProvider {
    fn announce_url(&self) -> String {
        return self.torrent.announce.clone();
    }

    fn client_config(&self) -> Config {
        return self.client_config.clone();
    }

    fn file_provider(&self) -> Box<dyn FileProv> {
        return Box::new(TokioFileProv::new(self.layout.clone()));
    }

    fn info_hash(&self) -> Vec<u8> {
        return self.torrent.info_hash.clone();
    }

    fn output_tx(&self) -> Sender<InternalEvent> {
        return self.tx_to_coordinator.clone();
    }

    fn peer_connector(&self) -> Box<dyn PeerConnector> {
        return Box::new(TCPPeerConnector {});
    }

    fn piece_hashes(&self) -> Vec<Vec<u8>> {
        return self.torrent.piece_hashes.clone();
    }

    fn piece_picker(&self) -> Arc<Mutex<dyn PiecePicker>> {
        return self.piece_picker.clone();
    }

    fn torrent_layout(&self) -> TorrentLayout {
        return self.layout.clone();
    }

    fn tracker_client(&self) -> Box<dyn TrackerClient> {
        let client = TorrentTrackerClient {
            announce_url: self.torrent.announce.clone(),
            client_config: self.client_config.clone(),
            info_hash: self.torrent.info_hash.clone(),
        };
        return Box::new(client);
    }
}
