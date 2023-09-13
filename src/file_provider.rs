use std::io::{Read, Seek, SeekFrom, Write};
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use crate::core_models::entities::{Block, TorrentLayout};

#[async_trait]
pub trait FileProv: Send {
    async fn open_read_write_instance(&mut self);
    async fn open_read_only_instance(&mut self);
    async fn read_block(&mut self, block: &Block) -> Vec<u8>;
    async fn read_piece(&mut self, piece_idx: usize) -> Vec<u8>;
    async fn write(&mut self, piece_idx: usize, offset_in_piece: usize, data: &Vec<u8>);
}

pub struct TokioFileProv {
    file: Option<tokio::fs::File>,
    layout: TorrentLayout,
}

impl TokioFileProv {
    pub fn new(layout: TorrentLayout) -> Self {
        return TokioFileProv { file: None, layout };
    }
}

#[async_trait]
impl FileProv for TokioFileProv {
    async fn open_read_write_instance(&mut self) {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.layout.output_file_path.as_str())
            .await
            .unwrap();
        self.file = Some(file);
    }

    async fn open_read_only_instance(&mut self) {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .open(self.layout.output_file_path.as_str())
            .await
            .unwrap();
        self.file = Some(file);
    }

    async fn read_block(&mut self, block: &Block) -> Vec<u8> {
        let offset = block.piece_idx * self.layout.head_pieces_length + block.offset;
        let mut piece_buff = vec![0u8; block.length];
        self.file.as_mut().unwrap().seek(SeekFrom::Start(offset as u64)).await.unwrap();
        self.file.as_mut().unwrap().read_exact(&mut piece_buff).await.unwrap();
        return piece_buff;
    }

    async fn read_piece(&mut self, piece_idx: usize) -> Vec<u8> {
        let offset = piece_idx * self.layout.head_pieces_length;
        let length = self.layout.piece_length(piece_idx);
        let mut piece_buff = vec![0u8; length];
        self.file.as_mut().unwrap().seek(SeekFrom::Start(offset as u64)).await.unwrap();
        self.file.as_mut().unwrap().read_exact(&mut piece_buff).await.unwrap();
        return piece_buff;
    }

    async fn write(&mut self, piece_idx: usize, offset_in_piece: usize, data: &Vec<u8>) {
        let offset = piece_idx * self.layout.head_pieces_length + offset_in_piece;
        self.file.as_mut().unwrap().seek(SeekFrom::Start(offset as u64)).await.unwrap();
        self.file.as_mut().unwrap().write_all(&*data).await.unwrap();
    }
}

pub struct TempFileProv {
    file: Option<std::fs::File>,
    layout: TorrentLayout,
}

impl TempFileProv {
    pub fn new(layout: TorrentLayout) -> Self {
        return TempFileProv { file: None, layout };
    }
}

#[async_trait]
impl FileProv for TempFileProv {
    async fn open_read_write_instance(&mut self) {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.layout.output_file_path.as_str())
            .unwrap();
        self.file = Some(file);
    }

    async fn open_read_only_instance(&mut self) {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(self.layout.output_file_path.as_str())
            .unwrap();
        self.file = Some(file);
    }

    async fn read_block(&mut self, block: &Block) -> Vec<u8> {
        let offset = block.piece_idx * self.layout.head_pieces_length + block.offset;
        let mut piece_buff = vec![0u8; block.length];
        self.file.as_mut().unwrap().seek(SeekFrom::Start(offset as u64)).unwrap();
        self.file.as_mut().unwrap().read_exact(&mut piece_buff).unwrap();
        return piece_buff;
    }

    async fn read_piece(&mut self, piece_idx: usize) -> Vec<u8> {
        let offset = piece_idx * self.layout.head_pieces_length;
        let length = self.layout.piece_length(piece_idx);
        let mut piece_buff = vec![0u8; length];
        self.file.as_mut().unwrap().seek(SeekFrom::Start(offset as u64)).unwrap();
        self.file.as_mut().unwrap().read_exact(&mut piece_buff).unwrap();
        return piece_buff;
    }

    async fn write(&mut self, piece_idx: usize, offset_in_piece: usize, data: &Vec<u8>) {
        let offset = piece_idx * self.layout.head_pieces_length + offset_in_piece;
        self.file.as_mut().unwrap().seek(SeekFrom::Start(offset as u64)).unwrap();
        self.file.as_mut().unwrap().write_all(&*data).unwrap();
    }
}
