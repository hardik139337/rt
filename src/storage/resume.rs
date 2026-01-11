//! Resume data module
//!
//! Handles saving and loading resume data for torrents.

use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use anyhow::Result;
use tokio::fs;

/// Resume data for a torrent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeData {
    /// Info hash as hex string
    pub info_hash: String,
    /// Which pieces are downloaded (bitfield)
    pub downloaded_pieces: Vec<u8>,
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
        let downloaded_pieces = vec![0u8; (piece_count + 7) / 8];
        
        Self {
            info_hash,
            downloaded_pieces,
            pieces: Vec::new(),
        }
    }

    /// Set a piece as downloaded in the bitfield
    pub fn set_piece_downloaded(&mut self, piece_index: usize) {
        if piece_index < self.downloaded_pieces.len() * 8 {
            let byte_index = piece_index / 8;
            let bit_index = 7 - (piece_index % 8);
            self.downloaded_pieces[byte_index] |= 1 << bit_index;
        }
    }

    /// Check if a piece is downloaded
    pub fn is_piece_downloaded(&self, piece_index: usize) -> bool {
        if piece_index >= self.downloaded_pieces.len() * 8 {
            return false;
        }
        let byte_index = piece_index / 8;
        let bit_index = 7 - (piece_index % 8);
        (self.downloaded_pieces[byte_index] & (1 << bit_index)) != 0
    }

    /// Get the number of downloaded pieces
    pub fn downloaded_count(&self) -> usize {
        let mut count = 0;
        for (i, byte) in self.downloaded_pieces.iter().enumerate() {
            for bit in 0..8 {
                if (byte & (1 << (7 - bit))) != 0 {
                    count += 1;
                }
            }
        }
        count
    }

    /// Add or update a piece state
    pub fn update_piece_state(&mut self, piece_state: PieceState) {
        if let Some(existing) = self.pieces.iter_mut().find(|p| p.index == piece_state.index) {
            *existing = piece_state;
        } else {
            self.pieces.push(piece_state);
        }
    }

    /// Get piece state for a specific index
    pub fn get_piece_state(&self, index: u32) -> Option<&PieceState> {
        self.pieces.iter().find(|p| p.index == index)
    }

    /// Remove piece state for a specific index
    pub fn remove_piece_state(&mut self, index: u32) {
        self.pieces.retain(|p| p.index != index);
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, data).await?;
        Ok(())
    }

    /// Load from file
    pub async fn load(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read(path).await?;
        Ok(Some(Self::deserialize(&data)?))
    }
}

impl PieceState {
    /// Create a new piece state
    pub fn new(index: u32, block_count: usize) -> Self {
        Self {
            index,
            blocks: vec![false; block_count],
        }
    }

    /// Mark a block as downloaded
    pub fn set_block_downloaded(&mut self, block_index: usize) {
        if block_index < self.blocks.len() {
            self.blocks[block_index] = true;
        }
    }

    /// Check if a block is downloaded
    pub fn is_block_downloaded(&self, block_index: usize) -> bool {
        self.blocks.get(block_index).copied().unwrap_or(false)
    }

    /// Get the number of downloaded blocks
    pub fn downloaded_blocks(&self) -> usize {
        self.blocks.iter().filter(|&&b| b).count()
    }

    /// Check if all blocks are downloaded
    pub fn is_complete(&self) -> bool {
        self.blocks.iter().all(|&b| b)
    }

    /// Clear all blocks
    pub fn clear(&mut self) {
        self.blocks = vec![false; self.blocks.len()];
    }
}

/// Resume data manager
pub struct ResumeManager {
    resume_dir: PathBuf,
}

impl ResumeManager {
    /// Create a new resume manager
    pub fn new(resume_dir: PathBuf) -> Self {
        Self { resume_dir }
    }

    /// Get the resume file path for a torrent
    fn resume_file_path(&self, info_hash: &str) -> PathBuf {
        self.resume_dir.join(format!("{}.resume", info_hash))
    }

    /// Save resume data for a torrent
    pub async fn save_resume_data(&self, resume_data: &ResumeData) -> Result<()> {
        let resume_path = self.resume_file_path(&resume_data.info_hash);
        resume_data.save(&resume_path).await?;
        Ok(())
    }

    /// Load resume data for a torrent
    pub async fn load_resume_data(&self, info_hash: &str) -> Result<Option<ResumeData>> {
        let resume_path = self.resume_file_path(info_hash);
        ResumeData::load(&resume_path).await
    }

    /// Delete resume data for a torrent
    pub async fn delete_resume_data(&self, info_hash: &str) -> Result<()> {
        let resume_path = self.resume_file_path(info_hash);
        if resume_path.exists() {
            fs::remove_file(&resume_path).await?;
        }
        Ok(())
    }

    /// Check if resume data exists for a torrent
    pub async fn has_resume_data(&self, info_hash: &str) -> bool {
        self.resume_file_path(info_hash).exists()
    }

    /// Get all resume data files
    pub async fn list_resume_files(&self) -> Result<Vec<String>> {
        let mut info_hashes = Vec::new();

        if !self.resume_dir.exists() {
            return Ok(info_hashes);
        }

        let mut entries = fs::read_dir(&self.resume_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "resume") {
                if let Some(stem) = path.file_stem() {
                    if let Some(info_hash) = stem.to_str() {
                        info_hashes.push(info_hash.to_string());
                    }
                }
            }
        }

        Ok(info_hashes)
    }
}

impl Default for ResumeManager {
    fn default() -> Self {
        Self::new(PathBuf::from(".resume"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resume_data_new() {
        let resume = ResumeData::new("test_hash".to_string(), 10);
        assert_eq!(resume.info_hash, "test_hash");
        // 10 pieces need 2 bytes (10 + 7) / 8 = 2
        assert_eq!(resume.downloaded_pieces.len(), 2);
        assert!(resume.pieces.is_empty());
    }

    #[test]
    fn test_resume_data_set_piece_downloaded() {
        let mut resume = ResumeData::new("test_hash".to_string(), 16);

        assert!(!resume.is_piece_downloaded(0));
        assert!(!resume.is_piece_downloaded(5));

        resume.set_piece_downloaded(5);
        assert!(!resume.is_piece_downloaded(0));
        assert!(resume.is_piece_downloaded(5));

        resume.set_piece_downloaded(0);
        assert!(resume.is_piece_downloaded(0));
    }

    #[test]
    fn test_resume_data_is_piece_downloaded_out_of_range() {
        let resume = ResumeData::new("test_hash".to_string(), 8);
        // Piece index 8 is out of range (only 0-7 valid for 8 pieces)
        assert!(!resume.is_piece_downloaded(8));
        assert!(!resume.is_piece_downloaded(100));
    }

    #[test]
    fn test_resume_data_downloaded_count() {
        let mut resume = ResumeData::new("test_hash".to_string(), 20);
        assert_eq!(resume.downloaded_count(), 0);

        resume.set_piece_downloaded(0);
        resume.set_piece_downloaded(5);
        resume.set_piece_downloaded(19);

        assert_eq!(resume.downloaded_count(), 3);
    }

    #[test]
    fn test_resume_data_update_piece_state() {
        let mut resume = ResumeData::new("test_hash".to_string(), 10);
        let piece_state = PieceState::new(5, 4);

        resume.update_piece_state(piece_state.clone());
        assert_eq!(resume.pieces.len(), 1);

        // Update existing piece
        let mut updated = piece_state.clone();
        updated.set_block_downloaded(0);
        resume.update_piece_state(updated);
        assert_eq!(resume.pieces.len(), 1);
        assert!(resume.pieces[0].is_block_downloaded(0));
    }

    #[test]
    fn test_resume_data_get_piece_state() {
        let mut resume = ResumeData::new("test_hash".to_string(), 10);
        let piece_state = PieceState::new(3, 4);
        resume.update_piece_state(piece_state);

        assert!(resume.get_piece_state(3).is_some());
        assert!(resume.get_piece_state(5).is_none());
    }

    #[test]
    fn test_resume_data_remove_piece_state() {
        let mut resume = ResumeData::new("test_hash".to_string(), 10);
        let piece_state = PieceState::new(3, 4);
        resume.update_piece_state(piece_state);

        assert!(resume.get_piece_state(3).is_some());
        resume.remove_piece_state(3);
        assert!(resume.get_piece_state(3).is_none());
    }

    #[test]
    fn test_resume_data_serialize_deserialize() {
        let mut resume = ResumeData::new("test_hash".to_string(), 10);
        resume.set_piece_downloaded(2);
        resume.set_piece_downloaded(5);

        let piece_state = PieceState::new(3, 4);
        resume.update_piece_state(piece_state);

        let serialized = resume.serialize().unwrap();
        let deserialized = ResumeData::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.info_hash, "test_hash");
        assert!(deserialized.is_piece_downloaded(2));
        assert!(deserialized.is_piece_downloaded(5));
        assert!(!deserialized.is_piece_downloaded(0));
        assert_eq!(deserialized.pieces.len(), 1);
    }

    #[test]
    fn test_piece_state_new() {
        let state = PieceState::new(5, 10);
        assert_eq!(state.index, 5);
        assert_eq!(state.blocks.len(), 10);
        assert!(!state.is_complete());
    }

    #[test]
    fn test_piece_state_set_block_downloaded() {
        let mut state = PieceState::new(0, 5);
        assert!(!state.is_block_downloaded(0));
        assert!(!state.is_block_downloaded(4));

        state.set_block_downloaded(2);
        assert!(state.is_block_downloaded(2));
        assert!(!state.is_block_downloaded(0));
    }

    #[test]
    fn test_piece_state_set_block_out_of_range() {
        let mut state = PieceState::new(0, 5);
        // Should not panic, just ignore
        state.set_block_downloaded(10);
        assert_eq!(state.downloaded_blocks(), 0);
    }

    #[test]
    fn test_piece_state_is_block_downloaded_out_of_range() {
        let state = PieceState::new(0, 5);
        assert!(!state.is_block_downloaded(10));
        assert!(!state.is_block_downloaded(100));
    }

    #[test]
    fn test_piece_state_downloaded_blocks() {
        let mut state = PieceState::new(0, 5);
        assert_eq!(state.downloaded_blocks(), 0);

        state.set_block_downloaded(0);
        state.set_block_downloaded(2);
        state.set_block_downloaded(4);

        assert_eq!(state.downloaded_blocks(), 3);
    }

    #[test]
    fn test_piece_state_is_complete() {
        let mut state = PieceState::new(0, 3);
        assert!(!state.is_complete());

        state.set_block_downloaded(0);
        assert!(!state.is_complete());

        state.set_block_downloaded(1);
        state.set_block_downloaded(2);
        assert!(state.is_complete());
    }

    #[test]
    fn test_piece_state_clear() {
        let mut state = PieceState::new(0, 3);
        state.set_block_downloaded(0);
        state.set_block_downloaded(1);

        assert_eq!(state.downloaded_blocks(), 2);
        state.clear();
        assert_eq!(state.downloaded_blocks(), 0);
        assert!(!state.is_block_downloaded(0));
    }

    #[test]
    fn test_resume_manager_new() {
        let manager = ResumeManager::new(PathBuf::from("/tmp/resume"));
        assert_eq!(manager.resume_dir, PathBuf::from("/tmp/resume"));
    }

    #[test]
    fn test_resume_manager_default() {
        let manager = ResumeManager::default();
        assert_eq!(manager.resume_dir, PathBuf::from(".resume"));
    }

    #[test]
    fn test_resume_manager_resume_file_path() {
        let manager = ResumeManager::new(PathBuf::from("/tmp/resume"));
        let path = manager.resume_file_path("abc123");
        assert_eq!(path, PathBuf::from("/tmp/resume/abc123.resume"));
    }

    #[tokio::test]
    async fn test_resume_manager_save_and_delete() {
        let temp_dir = std::env::temp_dir().join("test_resume");
        let manager = ResumeManager::new(temp_dir.clone());

        let resume_data = ResumeData::new("test_hash".to_string(), 10);
        manager.save_resume_data(&resume_data).await.unwrap();

        assert!(manager.has_resume_data("test_hash").await);

        manager.delete_resume_data("test_hash").await.unwrap();
        assert!(!manager.has_resume_data("test_hash").await);

        // Cleanup
        let _ = tokio::fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn test_resume_manager_load_nonexistent() {
        let temp_dir = std::env::temp_dir().join("test_resume_load");
        let manager = ResumeManager::new(temp_dir.clone());

        let result = manager.load_resume_data("nonexistent").await.unwrap();
        assert!(result.is_none());

        // Cleanup
        let _ = tokio::fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn test_resume_manager_save_and_load() {
        let temp_dir = std::env::temp_dir().join("test_resume_save_load");
        let manager = ResumeManager::new(temp_dir.clone());

        let mut resume_data = ResumeData::new("test_hash".to_string(), 10);
        resume_data.set_piece_downloaded(2);
        resume_data.set_piece_downloaded(5);

        manager.save_resume_data(&resume_data).await.unwrap();

        let loaded = manager.load_resume_data("test_hash").await.unwrap().unwrap();
        assert_eq!(loaded.info_hash, "test_hash");
        assert!(loaded.is_piece_downloaded(2));
        assert!(loaded.is_piece_downloaded(5));
        assert!(!loaded.is_piece_downloaded(0));

        // Cleanup
        let _ = tokio::fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn test_resume_manager_list_empty() {
        let temp_dir = std::env::temp_dir().join("test_resume_list_empty");
        let manager = ResumeManager::new(temp_dir.clone());

        let list = manager.list_resume_files().await.unwrap();
        assert!(list.is_empty());

        // Cleanup
        let _ = tokio::fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn test_resume_manager_list_files() {
        let temp_dir = std::env::temp_dir().join("test_resume_list_files");
        let manager = ResumeManager::new(temp_dir.clone());

        manager.save_resume_data(&ResumeData::new("hash1".to_string(), 5)).await.unwrap();
        manager.save_resume_data(&ResumeData::new("hash2".to_string(), 5)).await.unwrap();

        let list = manager.list_resume_files().await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"hash1".to_string()));
        assert!(list.contains(&"hash2".to_string()));

        // Cleanup
        let _ = tokio::fs::remove_dir_all(temp_dir).await;
    }
}
