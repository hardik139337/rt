//! Torrent information structures
//!
//! Provides high-level information about torrents.

use sha1::{Digest, Sha1};
use anyhow::Result;

/// Represents a file in a multi-file torrent
#[derive(Debug, Clone)]
pub struct TorrentFile {
    /// File path components (e.g., ["folder", "subfolder", "file.txt"])
    pub path: Vec<String>,
    /// File size in bytes
    pub length: u64,
}

/// High-level torrent information
#[derive(Debug, Clone)]
pub struct TorrentInfo {
    /// Primary tracker announce URL
    pub announce: String,
    /// List of all tracker announce URLs
    pub announce_list: Vec<String>,
    /// SHA1 hash of info dictionary
    pub info_hash: [u8; 20],
    /// Size of each piece in bytes
    pub piece_length: u64,
    /// List of piece hashes (each is a 20-byte SHA1 hash)
    pub pieces: Vec<[u8; 20]>,
    /// Torrent name
    pub name: String,
    /// Single file size (None for multi-file torrents)
    pub length: Option<u64>,
    /// Files in multi-file torrents (None for single-file torrents)
    pub files: Option<Vec<TorrentFile>>,
}

impl TorrentInfo {
    /// Calculate total size of all files in torrent
    pub fn total_size(&self) -> u64 {
        if let Some(length) = self.length {
            length
        } else if let Some(files) = &self.files {
            files.iter().map(|f| f.length).sum()
        } else {
            0
        }
    }

    /// Get number of pieces in torrent
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    /// Get an iterator over all files in torrent
    pub fn files_iter(&self) -> impl Iterator<Item = TorrentFile> + '_ {
        let name = self.name.clone();
        let length = self.length.unwrap_or(0);
        
        if let Some(files) = &self.files {
            Box::new(files.iter().cloned()) as Box<dyn Iterator<Item = _> + '_>
        } else if self.length.is_some() {
            // Single file torrent - create a virtual TorrentFile
            Box::new(std::iter::once(TorrentFile {
                path: vec![name],
                length,
            })) as Box<dyn Iterator<Item = _> + '_>
        } else {
            Box::new(std::iter::empty()) as Box<dyn Iterator<Item = _> + '_>
        }
    }

    /// Check if this is a multi-file torrent
    pub fn is_multi_file(&self) -> bool {
        self.files.is_some()
    }

    /// Get info hash as a hex string
    pub fn info_hash_hex(&self) -> String {
        hex::encode(self.info_hash)
    }

    /// Generate info hash from info dictionary bytes
    pub fn generate_info_hash(info_dict_bytes: &[u8]) -> [u8; 20] {
        let mut hasher = Sha1::new();
        hasher.update(info_dict_bytes);
        let result = hasher.finalize();
        result.into()
    }

    /// Parse piece hashes from concatenated bytes in torrent file
    pub fn parse_piece_hashes(pieces_bytes: &[u8]) -> Result<Vec<[u8; 20]>> {
        if pieces_bytes.len() % 20 != 0 {
            return Err(anyhow::anyhow!(
                "Pieces field length must be a multiple of 20, got {}",
                pieces_bytes.len()
            ));
        }

        let mut pieces = Vec::new();
        for chunk in pieces_bytes.chunks_exact(20) {
            let mut hash = [0u8; 20];
            hash.copy_from_slice(chunk);
            pieces.push(hash);
        }

        Ok(pieces)
    }

    /// Get piece hash for a specific piece index
    pub fn piece_hash(&self, index: usize) -> Option<[u8; 20]> {
        self.pieces.get(index).copied()
    }

    /// Get byte range for a specific piece
    pub fn piece_range(&self, index: usize) -> Option<(u64, u64)> {
        if index >= self.pieces.len() {
            return None;
        }

        let start = (index as u64) * self.piece_length;
        let total = self.total_size();
        let end = std::cmp::min(start + self.piece_length, total);

        Some((start, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_file() {
        let file = TorrentFile {
            path: vec!["folder".to_string(), "file.txt".to_string()],
            length: 1024,
        };
        assert_eq!(file.path.len(), 2);
        assert_eq!(file.length, 1024);
    }

    #[test]
    fn test_torrent_info_single_file() {
        let info = TorrentInfo {
            announce: "http://tracker.example.com".to_string(),
            announce_list: vec!["http://tracker.example.com".to_string()],
            info_hash: [1u8; 20],
            piece_length: 1024,
            pieces: vec![[2u8; 20], [3u8; 20]],
            name: "test.torrent".to_string(),
            length: Some(2048),
            files: None,
        };

        assert_eq!(info.total_size(), 2048);
        assert_eq!(info.piece_count(), 2);
        assert!(!info.is_multi_file());
        assert_eq!(info.info_hash_hex(), hex::encode([1u8; 20]));
    }

    #[test]
    fn test_torrent_info_multi_file() {
        let info = TorrentInfo {
            announce: "http://tracker.example.com".to_string(),
            announce_list: vec!["http://tracker.example.com".to_string()],
            info_hash: [1u8; 20],
            piece_length: 1024,
            pieces: vec![[2u8; 20]],
            name: "test.torrent".to_string(),
            length: None,
            files: Some(vec![
                TorrentFile { path: vec!["file1.txt".to_string()], length: 500 },
                TorrentFile { path: vec!["file2.txt".to_string()], length: 524 },
            ]),
        };

        assert_eq!(info.total_size(), 1024);
        assert!(info.is_multi_file());
    }

    #[test]
    fn test_files_iter_single_file() {
        let info = TorrentInfo {
            announce: "http://tracker.example.com".to_string(),
            announce_list: vec![],
            info_hash: [1u8; 20],
            piece_length: 1024,
            pieces: vec![],
            name: "single.txt".to_string(),
            length: Some(2048),
            files: None,
        };

        let files: Vec<_> = info.files_iter().collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, vec!["single.txt"]);
        assert_eq!(files[0].length, 2048);
    }

    #[test]
    fn test_files_iter_multi_file() {
        let info = TorrentInfo {
            announce: "http://tracker.example.com".to_string(),
            announce_list: vec![],
            info_hash: [1u8; 20],
            piece_length: 1024,
            pieces: vec![],
            name: "multi".to_string(),
            length: None,
            files: Some(vec![
                TorrentFile { path: vec!["file1.txt".to_string()], length: 100 },
                TorrentFile { path: vec!["file2.txt".to_string()], length: 200 },
            ]),
        };

        let files: Vec<_> = info.files_iter().collect();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_generate_info_hash() {
        let data = b"test data";
        let hash = TorrentInfo::generate_info_hash(data);
        assert_eq!(hash.len(), 20);
    }

    #[test]
    fn test_parse_piece_hashes_valid() {
        let hashes: Vec<u8> = (0..40).map(|i| i as u8).collect();
        let result = TorrentInfo::parse_piece_hashes(&hashes).unwrap();
        assert_eq!(result.len(), 2);
        let expected: [u8; 20] = (0..20).map(|i| i as u8).collect::<Vec<u8>>().try_into().unwrap();
        assert_eq!(result[0], expected);
    }

    #[test]
    fn test_parse_piece_hashes_invalid() {
        let hashes = vec![1u8; 21]; // Not a multiple of 20
        assert!(TorrentInfo::parse_piece_hashes(&hashes).is_err());
    }

    #[test]
    fn test_piece_hash() {
        let info = TorrentInfo {
            announce: "http://tracker.example.com".to_string(),
            announce_list: vec![],
            info_hash: [1u8; 20],
            piece_length: 1024,
            pieces: vec![[2u8; 20], [3u8; 20]],
            name: "test".to_string(),
            length: Some(2048),
            files: None,
        };

        assert_eq!(info.piece_hash(0), Some([2u8; 20]));
        assert_eq!(info.piece_hash(1), Some([3u8; 20]));
        assert_eq!(info.piece_hash(2), None);
    }

    #[test]
    fn test_piece_range() {
        let info = TorrentInfo {
            announce: "http://tracker.example.com".to_string(),
            announce_list: vec![],
            info_hash: [1u8; 20],
            piece_length: 1024,
            pieces: vec![[2u8; 20], [3u8; 20]],
            name: "test".to_string(),
            length: Some(1500),
            files: None,
        };

        assert_eq!(info.piece_range(0), Some((0, 1024)));
        assert_eq!(info.piece_range(1), Some((1024, 1500))); // Last piece is shorter
        assert_eq!(info.piece_range(2), None);
    }
}
