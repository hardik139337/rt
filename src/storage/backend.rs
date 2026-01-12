//! Storage backend abstraction for torrent downloads
//!
//! This module provides a trait-based abstraction for different storage backends,
//! enabling downloads to local disk, cloud storage, or custom destinations.

use async_trait::async_trait;
use bytes::Bytes;
use std::path::PathBuf;

use crate::storage::piece::PieceStorage;
use crate::torrent::info::TorrentFile;
use anyhow::Result;

/// Abstract storage backend for torrent data
///
/// This trait allows DownloadManager to work with different storage backends
/// without knowing implementation details. Implementations can store data
/// to disk, cloud storage, or any other destination.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    // ==================== Lifecycle Methods ====================
    
    /// Initialize storage for the torrent
    ///
    /// For FileStorage: Creates sparse files on disk
    /// For DriveStorage: Creates resumable upload sessions
    async fn initialize(&mut self, files: &[TorrentFile]) -> Result<()>;
    
    /// Complete all storage operations
    ///
    /// For FileStorage: No-op (files already written)
    /// For DriveStorage: Finalizes upload sessions
    async fn complete(&self) -> Result<()>;
    
    // ==================== Piece Operations ====================
    
    /// Write a verified piece to storage
    ///
    /// This method is called after piece verification succeeds.
    /// Implementations must handle the piece data without modification.
    ///
    /// # Arguments
    /// * `piece_index` - Index of the piece in the torrent
    /// * `data` - Verified piece data as Bytes (zero-copy friendly)
    ///
    /// # Requirements
    /// - Must not modify the piece data
    /// - Must handle piece data in-memory (no disk writes for DriveStorage)
    /// - Must support resumable operations (DriveStorage)
    async fn write_piece(&mut self, piece_index: u32, data: Bytes) -> Result<()>;
    
    /// Read a piece from storage (for verification/resume)
    ///
    /// For FileStorage: Reads from disk
    /// For DriveStorage: Returns cached piece data or error (not readable from Drive)
    async fn read_piece(&self, piece_index: u32) -> Result<Option<Bytes>>;
    
    // ==================== Progress Tracking ====================
    
    /// Check if download is complete
    fn is_complete(&self) -> bool;
    
    /// Get download progress (0.0 to 1.0)
    fn get_progress(&self) -> f64;
    
    /// Get the number of verified pieces
    fn verified_count(&self) -> usize;
    
    /// Get total piece count
    fn total_pieces(&self) -> usize;
    
    // ==================== Piece Storage Access ====================
    
    /// Get piece storage for verification
    fn pieces(&self) -> &PieceStorage;
    
    /// Get mutable piece storage
    fn pieces_mut(&mut self) -> &mut PieceStorage;
    
    // ==================== Metadata ====================
    
    /// Get storage type identifier
    fn storage_type(&self) -> StorageType;
    
    /// Get storage-specific metadata
    fn metadata(&self) -> StorageMetadata;
}

/// Storage type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    /// Local filesystem storage
    File,
    /// Google Drive cloud storage
    Drive,
}

/// Storage-specific metadata
#[derive(Debug, Clone)]
pub struct StorageMetadata {
    pub storage_type: StorageType,
    pub base_path: Option<PathBuf>,
    pub drive_folder_id: Option<String>,
    pub total_size: u64,
    pub piece_count: usize,
}
