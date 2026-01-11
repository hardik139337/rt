//! Piece storage module
//!
//! Manages individual piece storage and status.

use sha1::{Digest, Sha1};
use anyhow::Result;
use serde::{Serialize, Deserialize};

/// Status of a piece
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PieceStatus {
    /// Piece not downloaded
    Missing,
    /// Piece is being downloaded
    Downloading,
    /// Piece downloaded but not verified
    Downloaded,
    /// Piece verified and complete
    Complete,
}

impl Default for PieceStatus {
    fn default() -> Self {
        PieceStatus::Missing
    }
}

/// Represents a block within a piece
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Piece index this block belongs to
    pub piece_index: u32,
    /// Offset within the piece
    pub offset: u32,
    /// Length of the block
    pub length: u32,
    /// Block data
    pub data: Vec<u8>,
}

impl Block {
    /// Create a new block
    pub fn new(piece_index: u32, offset: u32, length: u32, data: Vec<u8>) -> Self {
        Self {
            piece_index,
            offset,
            length,
            data,
        }
    }

    /// Get the block data
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// Represents a piece of the torrent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Piece {
    /// Piece index
    pub index: u32,
    /// Piece data
    pub data: Vec<u8>,
    /// Expected SHA1 hash
    pub hash: [u8; 20],
    /// Whether piece is verified
    pub verified: bool,
    /// Blocks within piece (None for missing blocks)
    pub blocks: Vec<Option<Vec<u8>>>,
}

impl Piece {
    /// Create a new piece
    pub fn new(index: u32, piece_length: usize, expected_hash: [u8; 20]) -> Self {
        // Calculate number of blocks (default 16KB blocks)
        const DEFAULT_BLOCK_SIZE: usize = 16 * 1024;
        let num_blocks = (piece_length + DEFAULT_BLOCK_SIZE - 1) / DEFAULT_BLOCK_SIZE;
        
        Self {
            index,
            data: Vec::with_capacity(piece_length),
            hash: expected_hash,
            verified: false,
            blocks: vec![None; num_blocks],
        }
    }

    /// Add a block to the piece
    pub fn add_block(&mut self, offset: u32, data: Vec<u8>) -> Result<()> {
        let block_index = (offset as usize) / (16 * 1024);
        
        if block_index >= self.blocks.len() {
            return Err(anyhow::anyhow!("Block index {} out of range", block_index));
        }

        self.blocks[block_index] = Some(data);
        Ok(())
    }

    /// Check if all blocks are downloaded
    pub fn is_complete(&self) -> bool {
        self.blocks.iter().all(|b| b.is_some())
    }

    /// Verify the piece hash
    pub fn verify(&mut self) -> bool {
        // Combine all blocks into data
        self.data.clear();
        for block in &self.blocks {
            if let Some(block_data) = block {
                self.data.extend_from_slice(block_data);
            }
        }

        // Calculate SHA1 hash
        let mut hasher = Sha1::new();
        hasher.update(&self.data);
        let hash = hasher.finalize();

        self.verified = hash.as_slice() == self.hash;
        self.verified
    }

    /// Get blocks that are still needed
    pub fn get_missing_blocks(&self) -> Vec<(u32, u32)> {
        let mut missing = Vec::new();
        const DEFAULT_BLOCK_SIZE: u32 = 16 * 1024;

        for (i, block) in self.blocks.iter().enumerate() {
            if block.is_none() {
                let offset = (i as u32) * DEFAULT_BLOCK_SIZE;
                missing.push((offset, DEFAULT_BLOCK_SIZE));
            }
        }

        missing
    }

    /// Clear piece data
    pub fn clear(&mut self) {
        self.data.clear();
        self.verified = false;
        for block in &mut self.blocks {
            *block = None;
        }
    }

    /// Get the piece data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Check if piece is verified
    pub fn is_verified(&self) -> bool {
        self.verified
    }

    /// Get the number of blocks
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Get the number of downloaded blocks
    pub fn downloaded_blocks(&self) -> usize {
        self.blocks.iter().filter(|b| b.is_some()).count()
    }
}

/// Manages piece storage for a torrent
#[derive(Debug)]
pub struct PieceStorage {
    pieces: Vec<Piece>,
    piece_length: u32,
}

impl PieceStorage {
    /// Create a new piece storage
    pub fn new(piece_hashes: Vec<[u8; 20]>, piece_length: u32, total_size: u64) -> Self {
        let mut pieces = Vec::new();
        let num_pieces = piece_hashes.len();

        for (index, hash) in piece_hashes.into_iter().enumerate() {
            // Last piece may be smaller
            let length = if index == num_pieces - 1 {
                let remaining = total_size - (index as u64 * piece_length as u64);
                remaining as usize
            } else {
                piece_length as usize
            };

            pieces.push(Piece::new(index as u32, length, hash));
        }

        Self {
            pieces,
            piece_length,
        }
    }

    /// Get a piece by index
    pub fn get_piece(&self, index: usize) -> Option<&Piece> {
        self.pieces.get(index)
    }

    /// Get a mutable piece by index
    pub fn get_piece_mut(&mut self, index: usize) -> Option<&mut Piece> {
        self.pieces.get_mut(index)
    }

    /// Get all pieces
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Get the number of pieces
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    /// Get the piece length
    pub fn piece_length(&self) -> u32 {
        self.piece_length
    }

    /// Get the bitfield representation of completed pieces
    pub fn bitfield(&self) -> Vec<u8> {
        let mut bitfield = vec![0u8; (self.pieces.len() + 7) / 8];
        for (i, piece) in self.pieces.iter().enumerate() {
            if piece.is_verified() {
                let byte_index = i / 8;
                let bit_index = 7 - (i % 8);
                bitfield[byte_index] |= 1 << bit_index;
            }
        }
        bitfield
    }

    /// Get the number of completed pieces
    pub fn completed_count(&self) -> usize {
        self.pieces.iter().filter(|p| p.is_verified()).count()
    }

    /// Check if all pieces are complete
    pub fn is_complete(&self) -> bool {
        self.pieces.iter().all(|p| p.is_verified())
    }

    /// Get the download progress (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.pieces.is_empty() {
            0.0
        } else {
            self.completed_count() as f64 / self.pieces.len() as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_new() {
        let block = Block::new(0, 0, 1024, vec![1u8; 1024]);
        assert_eq!(block.piece_index, 0);
        assert_eq!(block.offset, 0);
        assert_eq!(block.length, 1024);
        assert_eq!(block.data.len(), 1024);
    }

    #[test]
    fn test_block_data() {
        let data = vec![1, 2, 3, 4];
        let block = Block::new(0, 0, 4, data.clone());
        assert_eq!(block.data(), data.as_slice());
    }

    #[test]
    fn test_piece_status_default() {
        assert_eq!(PieceStatus::default(), PieceStatus::Missing);
    }

    #[test]
    fn test_piece_new() {
        let hash = [1u8; 20];
        let piece = Piece::new(0, 1024, hash);
        assert_eq!(piece.index, 0);
        assert_eq!(piece.hash, hash);
        assert!(!piece.verified);
        assert_eq!(piece.block_count(), 1);
    }

    #[test]
    fn test_piece_new_multiple_blocks() {
        let hash = [1u8; 20];
        let piece = Piece::new(0, 32 * 1024, hash);
        assert_eq!(piece.block_count(), 2);
    }

    #[test]
    fn test_piece_add_block() {
        let hash = [1u8; 20];
        let mut piece = Piece::new(0, 32 * 1024, hash);
        let data = vec![1u8; 16 * 1024];

        assert!(piece.add_block(0, data.clone()).is_ok());
        assert_eq!(piece.downloaded_blocks(), 1);
    }

    #[test]
    fn test_piece_add_block_out_of_range() {
        let hash = [1u8; 20];
        let mut piece = Piece::new(0, 16 * 1024, hash);
        let data = vec![1u8; 16 * 1024];

        assert!(piece.add_block(32 * 1024, data).is_err());
    }

    #[test]
    fn test_piece_is_complete() {
        let hash = [1u8; 20];
        let mut piece = Piece::new(0, 16 * 1024, hash);
        assert!(!piece.is_complete());

        piece.blocks[0] = Some(vec![1u8; 16 * 1024]);
        assert!(piece.is_complete());
    }

    #[test]
    fn test_piece_verify_valid() {
        let hash = [1u8; 20];
        let mut piece = Piece::new(0, 16 * 1024, hash);
        piece.blocks[0] = Some(vec![1u8; 16 * 1024]);

        // This should fail since hash won't match
        assert!(!piece.verify());
        assert!(!piece.verified);
    }

    #[test]
    fn test_piece_get_missing_blocks() {
        let hash = [1u8; 20];
        let piece = Piece::new(0, 32 * 1024, hash);
        let missing = piece.get_missing_blocks();
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn test_piece_clear() {
        let hash = [1u8; 20];
        let mut piece = Piece::new(0, 16 * 1024, hash);
        piece.blocks[0] = Some(vec![1u8; 16 * 1024]);
        piece.verified = true;

        piece.clear();
        assert!(!piece.verified);
        assert!(piece.data.is_empty());
        assert!(!piece.is_complete());
    }

    #[test]
    fn test_piece_downloaded_blocks() {
        let hash = [1u8; 20];
        let mut piece = Piece::new(0, 32 * 1024, hash);
        assert_eq!(piece.downloaded_blocks(), 0);

        piece.blocks[0] = Some(vec![1u8; 16 * 1024]);
        assert_eq!(piece.downloaded_blocks(), 1);
    }

    #[test]
    fn test_piece_storage_new() {
        let hashes = vec![[1u8; 20], [2u8; 20]];
        let storage = PieceStorage::new(hashes.clone(), 1024, 2048);
        assert_eq!(storage.piece_count(), 2);
        assert_eq!(storage.piece_length(), 1024);
        assert_eq!(storage.completed_count(), 0);
    }

    #[test]
    fn test_piece_storage_get_piece() {
        let hashes = vec![[1u8; 20], [2u8; 20]];
        let storage = PieceStorage::new(hashes, 1024, 2048);

        assert!(storage.get_piece(0).is_some());
        assert!(storage.get_piece(1).is_some());
        assert!(storage.get_piece(2).is_none());
    }

    #[test]
    fn test_piece_storage_get_piece_mut() {
        let hashes = vec![[1u8; 20], [2u8; 20]];
        let mut storage = PieceStorage::new(hashes, 1024, 2048);

        if let Some(piece) = storage.get_piece_mut(0) {
            piece.blocks[0] = Some(vec![1u8; 1024]);
            assert!(piece.is_complete());
        }
    }

    #[test]
    fn test_piece_storage_bitfield() {
        let hashes = vec![[1u8; 20], [2u8; 20], [3u8; 20]];
        let mut storage = PieceStorage::new(hashes, 1024, 3072);

        let bitfield = storage.bitfield();
        // All pieces incomplete, bitfield should be all zeros
        assert_eq!(bitfield, vec![0u8; 1]);

        // Mark first piece as complete
        if let Some(piece) = storage.get_piece_mut(0) {
            piece.blocks[0] = Some(vec![1u8; 1024]);
            piece.verified = true;
        }

        let bitfield = storage.bitfield();
        // First piece uses MSB position (bit 7)
        assert_eq!(bitfield[0], 0b10000000); // First bit (MSB) set

        // Mark second piece as complete
        if let Some(piece) = storage.get_piece_mut(1) {
            piece.blocks[0] = Some(vec![1u8; 1024]);
            piece.verified = true;
        }

        let bitfield = storage.bitfield();
        // First and second bits set
        assert_eq!(bitfield[0], 0b11000000);
    }

    #[test]
    fn test_piece_storage_progress() {
        let hashes = vec![[1u8; 20], [2u8; 20], [3u8; 20], [4u8; 20]];
        let mut storage = PieceStorage::new(hashes, 1024, 4096);

        assert_eq!(storage.progress(), 0.0);

        if let Some(piece) = storage.get_piece_mut(0) {
            piece.blocks[0] = Some(vec![1u8; 1024]);
            piece.verified = true;
        }

        assert!((storage.progress() - 0.25).abs() < 0.001);

        if let Some(piece) = storage.get_piece_mut(1) {
            piece.blocks[0] = Some(vec![1u8; 1024]);
            piece.verified = true;
        }

        assert!((storage.progress() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_piece_storage_is_complete() {
        let hashes = vec![[1u8; 20], [2u8; 20]];
        let mut storage = PieceStorage::new(hashes, 1024, 2048);

        assert!(!storage.is_complete());

        for i in 0..2 {
            if let Some(piece) = storage.get_piece_mut(i) {
                piece.blocks[0] = Some(vec![1u8; 1024]);
                piece.verified = true;
            }
        }

        assert!(storage.is_complete());
    }

    #[test]
    fn test_piece_storage_empty() {
        let storage = PieceStorage::new(vec![], 1024, 0);
        assert_eq!(storage.piece_count(), 0);
        assert_eq!(storage.progress(), 0.0);
        assert!(storage.is_complete());
    }

    #[test]
    fn test_piece_storage_last_piece_smaller() {
        let hashes = vec![[1u8; 20], [2u8; 20]];
        let storage = PieceStorage::new(hashes, 1024, 1500);

        assert_eq!(storage.get_piece(0).unwrap().blocks.len(), 1);
        assert_eq!(storage.get_piece(1).unwrap().blocks.len(), 1);
    }
}
