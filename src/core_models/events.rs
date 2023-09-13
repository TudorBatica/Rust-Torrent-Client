use tokio::sync::mpsc::Sender;
use crate::core_models::entities::Block;

#[derive(Debug)]
pub enum InternalEvent {
    BlockDownloaded(Block, Vec<u8>),
    BlockStored(Block),
    DataCollectorStarted(Sender<(Block, Vec<u8>)>),
    DownloadComplete,
    EndGameEnabled,
    PieceStored(usize),
}

impl InternalEvent {
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