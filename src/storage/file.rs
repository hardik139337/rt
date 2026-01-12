//! File storage module
//!
//! Handles file I/O operations for torrent data.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use tokio::fs;
use tokio::io::{AsyncWriteExt, AsyncReadExt, AsyncSeekExt};
use tracing::{debug, error, info, trace, warn};
use bytes::Bytes;
use crate::torrent::TorrentInfo;
use crate::storage::piece::{Piece, PieceStorage, PieceStatus};
use crate::storage::backend::{StorageBackend, StorageType, StorageMetadata};
use crate::torrent::info::TorrentFile;
use crate::error::TorrentError;

/// File storage for torrent data
#[derive(Debug)]
pub struct FileStorage {
    /// Base download directory
    base_path: PathBuf,
    /// Torrent information
    torrent_info: Arc<TorrentInfo>,
    /// All pieces
    pieces: PieceStorage,
    /// Which pieces are downloaded (bitfield)
    downloaded_pieces: Vec<bool>,
}

/// Represents a file entry in the storage
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub offset: u64,
    pub length: u64,
}

impl FileStorage {
    /// Create a new file storage
    pub async fn new(base_path: PathBuf, torrent_info: Arc<TorrentInfo>) -> Result<Self> {
        info!("Creating file storage for torrent: {}", torrent_info.name);
        info!("Base path: {}", base_path.display());
        
        let pieces = PieceStorage::new(
            torrent_info.pieces.clone(),
            torrent_info.piece_length as u32,
            torrent_info.total_size(),
        );

        let downloaded_pieces = vec![false; pieces.piece_count()];
        debug!("Initialized {} pieces", pieces.piece_count());

        Ok(Self {
            base_path,
            torrent_info,
            pieces,
            downloaded_pieces,
        })
    }

    /// Create file structure for torrent
    pub async fn create_files(&self) -> Result<()> {
        info!("Creating file structure for torrent: {}", self.torrent_info.name);
        
        // Create base directory if needed
        if !self.base_path.exists() {
            debug!("Creating base directory: {}", self.base_path.display());
            fs::create_dir_all(&self.base_path).await
                .map_err(|e| {
                    error!("Failed to create base directory '{}': {}", self.base_path.display(), e);
                    TorrentError::storage_error_full("Failed to create base directory", self.base_path.display().to_string(), e.to_string())
                })?;
        }

        // Handle multi-file torrent
        if let Some(files) = &self.torrent_info.files {
            info!("Creating {} files for multi-file torrent", files.len());
            for file in files {
                let file_path = self.base_path.join(file.path.join("/"));
                debug!("Creating file: {}", file_path.display());
                if let Some(parent) = file_path.parent() {
                    if !parent.exists() {
                        debug!("Creating directory: {}", parent.display());
                        fs::create_dir_all(parent).await
                            .map_err(|e| {
                                error!("Failed to create directory '{}': {}", parent.display(), e);
                                TorrentError::storage_error_full("Failed to create directory", parent.display().to_string(), e.to_string())
                            })?;
                    }
                }
                // Create sparse file
                let mut f = fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&file_path)
                    .await
                    .map_err(|e| {
                        error!("Failed to create file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to create file", file_path.display().to_string(), e.to_string())
                    })?;
                f.set_len(file.length).await
                    .map_err(|e| {
                        error!("Failed to set file length for '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to set file length", file_path.display().to_string(), e.to_string())
                    })?;
                f.flush().await
                    .map_err(|e| {
                        error!("Failed to flush file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to flush file", file_path.display().to_string(), e.to_string())
                    })?;
            }
        } else {
            // Single file torrent
            info!("Creating single file torrent");
            let file_path = self.base_path.join(&self.torrent_info.name);
            let length = self.torrent_info.length.unwrap_or(0);
            debug!("Creating file: {} ({} bytes)", file_path.display(), length);
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&file_path)
                .await
                .map_err(|e| {
                    error!("Failed to create file '{}': {}", file_path.display(), e);
                    TorrentError::storage_error_full("Failed to create file", file_path.display().to_string(), e.to_string())
                })?;
            f.set_len(length).await
                .map_err(|e| {
                    error!("Failed to set file length for '{}': {}", file_path.display(), e);
                    TorrentError::storage_error_full("Failed to set file length", file_path.display().to_string(), e.to_string())
                })?;
            f.flush().await
                .map_err(|e| {
                    error!("Failed to flush file '{}': {}", file_path.display(), e);
                    TorrentError::storage_error_full("Failed to flush file", file_path.display().to_string(), e.to_string())
                })?;
        }

        info!("File structure created successfully");
        Ok(())
    }

    /// Write a piece to disk (internal method)
    async fn write_piece_internal(&self, piece_index: u32, data: &[u8]) -> Result<()> {
        debug!("Writing piece {} to disk ({} bytes)", piece_index, data.len());
        let offset = piece_index as u64 * self.torrent_info.piece_length;
        self.write_data(offset, data).await?;
        trace!("Piece {} written successfully", piece_index);
        Ok(())
    }

    /// Write data at a specific offset
    async fn write_data(&self, offset: u64, data: &[u8]) -> Result<()> {
        trace!("Writing data at offset {} ({} bytes)", offset, data.len());
        let mut remaining_data = data;
        let mut current_offset = offset;
  
        // Collect files into a Vec to avoid Send issues with iterator
        let files: Vec<_> = self.torrent_info.as_ref().files_iter().collect();
        for file in files {
            let file_path = self.base_path.join(file.path.join("/"));
            let file_start = self.get_file_offset(&file);
            let file_end = file_start + file.length;
 
            // Skip files that come before this offset
            if file_end <= current_offset {
                trace!("Skipping file: {} (before offset)", file_path.display());
                continue;
            }
 
            // Calculate how much to write to this file
            let write_offset = current_offset.saturating_sub(file_start);
            let write_length = std::cmp::min(
                remaining_data.len() as u64,
                file.length - write_offset,
            ) as usize;
 
            if write_length > 0 {
                trace!("Writing {} bytes to file {} at offset {}", write_length, file_path.display(), write_offset);
                let mut file_handle = fs::OpenOptions::new()
                    .write(true)
                    .open(&file_path)
                    .await
                    .map_err(|e| {
                        error!("Failed to open file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to open file for writing", file_path.display().to_string(), e.to_string())
                    })?;
 
                file_handle.seek(std::io::SeekFrom::Start(write_offset)).await
                    .map_err(|e| {
                        error!("Failed to seek in file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to seek in file", file_path.display().to_string(), e.to_string())
                    })?;
                file_handle.write_all(&remaining_data[..write_length]).await
                    .map_err(|e| {
                        error!("Failed to write to file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to write to file", file_path.display().to_string(), e.to_string())
                    })?;
                file_handle.flush().await
                    .map_err(|e| {
                        error!("Failed to flush file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to flush file", file_path.display().to_string(), e.to_string())
                    })?;
 
                remaining_data = &remaining_data[write_length..];
                current_offset += write_length as u64;
            }
 
            if remaining_data.is_empty() {
                break;
            }
        }
 
        debug!("Successfully wrote {} bytes at offset {}", data.len(), offset);
        Ok(())
    }

    /// Read a piece from disk (internal method)
    async fn read_piece_internal(&self, piece_index: u32) -> Result<Vec<u8>> {
        debug!("Reading piece {} from disk", piece_index);
        let offset = piece_index as u64 * self.torrent_info.piece_length;
        let piece_length = self.pieces.get_piece(piece_index as usize)
            .map(|p| p.data.len())
            .unwrap_or(self.torrent_info.piece_length as usize);
        trace!("Piece {} at offset {} ({} bytes)", piece_index, offset, piece_length);
        let data = self.read_data(offset, piece_length).await?;
        debug!("Successfully read piece {} ({} bytes)", piece_index, data.len());
        Ok(data)
    }

    /// Read data from a specific offset
    async fn read_data(&self, offset: u64, length: usize) -> Result<Vec<u8>> {
        trace!("Reading data at offset {} ({} bytes)", offset, length);
        let mut buffer = Vec::with_capacity(length);
        let mut remaining_length = length as u64;
        let mut current_offset = offset;
  
        // Collect files into a Vec to avoid Send issues with iterator
        let files: Vec<_> = self.torrent_info.as_ref().files_iter().collect();
        for file in files {
            let file_path = self.base_path.join(file.path.join("/"));
            let file_start = self.get_file_offset(&file);
            let file_end = file_start + file.length;
 
            // Skip files that come before this offset
            if file_end <= current_offset {
                trace!("Skipping file: {} (before offset)", file_path.display());
                continue;
            }
 
            // Calculate how much to read from this file
            let read_offset = current_offset.saturating_sub(file_start);
            let read_length = std::cmp::min(
                remaining_length,
                file.length - read_offset,
            ) as usize;
 
            if read_length > 0 {
                trace!("Reading {} bytes from file {} at offset {}", read_length, file_path.display(), read_offset);
                let mut file_handle = fs::File::open(&file_path).await
                    .map_err(|e| {
                        error!("Failed to open file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to open file for reading", file_path.display().to_string(), e.to_string())
                    })?;
 
                file_handle.seek(std::io::SeekFrom::Start(read_offset)).await
                    .map_err(|e| {
                        error!("Failed to seek in file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to seek in file", file_path.display().to_string(), e.to_string())
                    })?;
                let mut chunk = vec![0u8; read_length];
                file_handle.read_exact(&mut chunk).await
                    .map_err(|e| {
                        error!("Failed to read from file '{}': {}", file_path.display(), e);
                        TorrentError::storage_error_full("Failed to read from file", file_path.display().to_string(), e.to_string())
                    })?;
                buffer.extend_from_slice(&chunk);
 
                remaining_length -= read_length as u64;
                current_offset += read_length as u64;
            }
 
            if remaining_length == 0 {
                break;
            }
        }
 
        debug!("Successfully read {} bytes at offset {}", buffer.len(), offset);
        Ok(buffer)
    }

    /// Verify a piece against its hash
    pub async fn verify_piece(&self, piece_index: u32) -> Result<bool> {
        debug!("Verifying piece {}", piece_index);
        let data = self.read_piece_internal(piece_index).await?;
        let expected_hash = self.torrent_info.as_ref().piece_hash(piece_index as usize)
            .ok_or_else(|| {
                error!("Invalid piece index: {}", piece_index);
                TorrentError::validation_error_with_field("Invalid piece index", "piece_index".to_string())
            })?;
 
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(&data);
        let hash = hasher.finalize();
 
        let is_valid = hash.as_slice() == expected_hash;
        if is_valid {
            debug!("Piece {} verification: PASSED", piece_index);
        } else {
            warn!("Piece {} verification: FAILED (hash mismatch)", piece_index);
        }
        Ok(is_valid)
    }

    /// Check if download is complete
    pub fn is_complete(&self) -> bool {
        self.pieces.is_complete()
    }

    /// Get download progress (0.0 to 1.0)
    pub fn get_progress(&self) -> f64 {
        self.pieces.progress()
    }

    /// Get resume data
    pub fn resume_data(&self) -> ResumeData {
        let info_hash = hex::encode(self.torrent_info.info_hash);
        let downloaded_pieces = self.downloaded_pieces.clone();
 
        let pieces: Vec<PieceState> = self.pieces.pieces().iter()
            .map(|p| PieceState {
                index: p.index,
                blocks: p.blocks.iter().map(|b| b.is_some()).collect(),
            })
            .collect();
 
        ResumeData {
            info_hash,
            downloaded_pieces,
            pieces,
        }
    }

    /// Load resume data
    pub async fn load_resume(&mut self, resume_data: &ResumeData) -> Result<()> {
        info!("Loading resume data for torrent: {}", self.torrent_info.name);
        
        // Mark downloaded pieces
        let mut restored_count = 0;
        for (i, downloaded) in resume_data.downloaded_pieces.iter().enumerate() {
            if *downloaded {
                // Read piece data from disk
                if let Ok(data) = self.read_piece_internal(i as u32).await {
                    if let Some(piece) = self.pieces.get_piece_mut(i) {
                        piece.verified = true;
                        piece.data = data;
                        restored_count += 1;
                    }
                } else {
                    warn!("Failed to read piece {} from disk for resume", i);
                }
            }
        }
        debug!("Restored {} downloaded pieces from resume data", restored_count);
 
        // Restore partial piece states
        let mut partial_count = 0;
        for piece_state in &resume_data.pieces {
            if let Some(piece) = self.pieces.get_piece_mut(piece_state.index as usize) {
                for (i, downloaded) in piece_state.blocks.iter().enumerate() {
                    if *downloaded && i < piece.blocks.len() {
                        // Mark block as downloaded (data will be read from disk)
                        piece.blocks[i] = Some(Vec::new());
                        partial_count += 1;
                    }
                }
            }
        }
        debug!("Restored {} partial pieces from resume data", partial_count);
        
        info!("Resume data loaded successfully");
        Ok(())
    }

    /// Get the base path
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Get the torrent info
    pub fn torrent_info(&self) -> &Arc<TorrentInfo> {
        &self.torrent_info
    }

    /// Get the piece storage
    pub fn pieces(&self) -> &PieceStorage {
        &self.pieces
    }

    /// Get mutable piece storage
    pub fn pieces_mut(&mut self) -> &mut PieceStorage {
        &mut self.pieces
    }

    /// Mark a piece as downloaded
    pub fn mark_piece_downloaded(&mut self, piece_index: usize) {
        if piece_index < self.downloaded_pieces.len() {
            self.downloaded_pieces[piece_index] = true;
        }
    }

    /// Get the number of downloaded pieces
    pub fn downloaded_count(&self) -> usize {
        self.downloaded_pieces.iter().filter(|&&d| d).count()
    }

    /// Get file offset for a file
    fn get_file_offset(&self, file: &crate::torrent::TorrentFile) -> u64 {
        let mut offset = 0u64;
        for f in self.torrent_info.as_ref().files_iter() {
            if f.path == file.path {
                break;
            }
            offset += f.length;
        }
        offset
    }
}

/// Resume data for a torrent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeData {
    /// Info hash as hex string
    pub info_hash: String,
    /// Which pieces are downloaded
    pub downloaded_pieces: Vec<bool>,
    /// Partial piece data
    pub pieces: Vec<PieceState>,
}

/// State of a single piece for resume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieceState {
    /// Piece index
    pub index: u32,
    /// Which blocks are downloaded
    pub blocks: Vec<bool>,
}

impl ResumeData {
    /// Create new resume data
    pub fn new(info_hash: String, piece_count: usize) -> Self {
        Self {
            info_hash,
            downloaded_pieces: vec![false; piece_count],
            pieces: Vec::new(),
        }
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }

    /// Save to file
    pub async fn save(&self, path: &Path) -> Result<()> {
        let data = self.serialize()?;
        fs::write(path, data).await?;
        Ok(())
    }

    /// Load from file
    pub async fn load(path: &Path) -> Result<Option<Self>> {
        debug!("Loading resume data from: {}", path.display());
        if !path.exists() {
            info!("Resume file not found: {}", path.display());
            return Ok(None);
        }
        let data = fs::read(path).await
            .map_err(|e| {
                error!("Failed to read resume file '{}': {}", path.display(), e);
                TorrentError::storage_error_full("Failed to read resume file", path.display().to_string(), e.to_string())
            })?;
        let resume_data = Self::deserialize(&data)?;
        info!("Resume data loaded successfully");
        Ok(Some(resume_data))
    }
}

/// Implement StorageBackend trait for FileStorage
#[async_trait]
impl StorageBackend for FileStorage {
    async fn initialize(&mut self, files: &[TorrentFile]) -> Result<()> {
        // For FileStorage, initialize creates the file structure
        // We ignore the files parameter since we already have torrent_info
        self.create_files().await
    }
    
    async fn write_piece(&mut self, piece_index: u32, data: Bytes) -> Result<()> {
        // Convert Bytes to &[u8] for compatibility with existing write_piece
        // Call the internal write_piece method that takes &[u8]
        FileStorage::write_piece_internal(self, piece_index, data.as_ref()).await
    }
    
    async fn read_piece(&self, piece_index: u32) -> Result<Option<Bytes>> {
        // Call the internal read_piece method
        let data = FileStorage::read_piece_internal(self, piece_index).await?;
        Ok(Some(Bytes::from(data)))
    }
    
    async fn complete(&self) -> Result<()> {
        // No-op for file storage - files are already written
        Ok(())
    }
    
    fn is_complete(&self) -> bool {
        self.pieces.is_complete()
    }
    
    fn get_progress(&self) -> f64 {
        self.pieces.progress()
    }
    
    fn verified_count(&self) -> usize {
        self.pieces.completed_count()
    }
    
    fn total_pieces(&self) -> usize {
        self.pieces.piece_count()
    }
    
    fn pieces(&self) -> &PieceStorage {
        &self.pieces
    }
    
    fn pieces_mut(&mut self) -> &mut PieceStorage {
        &mut self.pieces
    }
    
    fn storage_type(&self) -> StorageType {
        StorageType::File
    }
    
    fn metadata(&self) -> StorageMetadata {
        StorageMetadata {
            storage_type: StorageType::File,
            base_path: Some(self.base_path.clone()),
            drive_folder_id: None,
            total_size: self.torrent_info.total_size(),
            piece_count: self.pieces.piece_count(),
        }
    }
}
