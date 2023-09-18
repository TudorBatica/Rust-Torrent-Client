use crate::core_models::entities::{Block, DataBlock};

pub type TransferIdx = usize;

#[derive(Debug)]
pub enum InternalEvent {
    BlockDownloaded(DataBlock),
    BlockStored(Block),
    DownloadComplete,
    EndGameEnabled(TransferIdx),
    PieceStored(usize),
}

impl InternalEvent {
    pub fn is_block_downloaded(&self) -> bool {
        return match self {
            InternalEvent::BlockDownloaded(_) => true,
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
    pub fn is_end_game_enabled(&self) -> bool {
        return match self {
            InternalEvent::EndGameEnabled(_) => true,
            _ => false
        };
    }
}