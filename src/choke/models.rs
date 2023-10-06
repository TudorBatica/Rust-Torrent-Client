pub enum ChokeEvent {
    UnchokePeers,
    OptimisticUnchoke,
    ClientInterestedInPeer(usize, bool),
    PeerInterestedInClient(usize, bool),
    BlockDownloadedFromPeer(usize),
    UnregisterPeer(usize),
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct PeerState {
    pub idx: usize,
    pub peer_choked_by_client: bool,
    pub client_interested_in_peer: bool,
    pub peer_interested_in_client: bool,
    pub downloaded_blocks: usize,
}

impl PeerState {
    pub fn new(peer_idx: usize) -> Self {
        return PeerState {
            idx: peer_idx,
            peer_choked_by_client: true,
            client_interested_in_peer: false,
            peer_interested_in_client: false,
            downloaded_blocks: 0,
        };
    }

    pub fn is_unchokeable(&self) -> bool {
        return self.peer_choked_by_client && self.peer_interested_in_client && self.client_interested_in_peer;
    }

    pub fn block_downloaded(&mut self) {
        self.downloaded_blocks += 1;
    }
    
}
