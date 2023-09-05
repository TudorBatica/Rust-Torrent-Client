use crate::core_models::entities::BlockPosition;

#[derive(Debug, PartialEq)]
pub enum InternalEvent {
    BlockStored(BlockPosition),
    PieceStored(usize),
    DownloadComplete,
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