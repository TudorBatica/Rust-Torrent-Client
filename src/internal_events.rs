use crate::core_models::BlockPosition;

pub enum CoordinatorEvent {
    BlockAcquired(BlockPosition),
    PieceAcquired(usize),
}

pub enum CoordinatorInput {
    DASEvent(DownloadAssemblerEvent),
    P2PEvent(P2PEvent),
}

pub enum DownloadAssemblerEvent {
    BlockAcquired(BlockPosition),
    PieceAcquired(usize),
    DownloadComplete,
}

pub enum P2PEvent {
    DownloadMetric(usize),
    BlockUploaded(usize),
}

