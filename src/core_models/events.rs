use crate::core_models::entities::{Block, DataBlock};

pub type TransferIdx = usize;

#[derive(Debug, Eq, PartialEq)]
pub enum InternalEvent {
    BlockDownloaded(usize, DataBlock),
    BlockStored(Block),
    ChokePeer(usize),
    DownloadComplete,
    PieceStored(usize),
    P2PTransferTerminated(usize),
    UnchokePeer(usize),
    ClientInterestedInPeer(usize, bool),
    PeerInterestedInClient(usize, bool),
}

impl InternalEvent {
    pub fn is_block_downloaded(&self) -> bool {
        return match self {
            InternalEvent::BlockDownloaded(..) => true,
            _ => false
        };
    }
    pub fn is_block_stored(&self) -> bool {
        return match self {
            InternalEvent::BlockStored(_) => true,
            _ => false
        };
    }
    pub fn is_piece_stored(&self) -> bool {
        return match self {
            InternalEvent::PieceStored(_) => true,
            _ => false
        };
    }
    pub fn is_download_complete(&self) -> bool {
        return match self {
            InternalEvent::DownloadComplete => true,
            _ => false
        };
    }
}