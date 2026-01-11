//! Torrent file parser
//!
//! Handles parsing of .torrent files and extracting metadata.

use serde_bencode::{de, ser};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tracing::{debug, error, info, trace, warn};

use crate::torrent::info::{TorrentInfo, TorrentFile};
use crate::error::TorrentError;

/// Represents a parsed .torrent file
#[derive(Debug, Deserialize)]
struct RawTorrentFile {
    /// Primary tracker announce URL
    announce: Option<String>,
    /// List of tracker tiers, each containing tracker URLs
    #[serde(default)]
    announce_list: Option<Vec<Vec<String>>>,
    /// The info dictionary containing file metadata
    info: RawInfoDict,
}

/// The info dictionary from a .torrent file
#[derive(Debug, Deserialize, Serialize)]
struct RawInfoDict {
    /// Torrent name
    name: String,
    /// Size of each piece in bytes
    piece_length: u64,
    /// Concatenated piece hashes (20 bytes each)
    pieces: Vec<u8>,
    /// Optional: indicates if torrent is private
    #[serde(default)]
    private: Option<u8>,
    #[serde(flatten)]
    mode: TorrentMode,
}

/// Represents either a single-file or multi-file torrent
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum TorrentMode {
    /// Single file torrent
    Single {
        /// File size in bytes
        length: u64,
        /// Optional MD5 checksum
        md5sum: Option<String>,
    },
    /// Multi-file torrent
    Multi {
        /// List of files in the torrent
        files: Vec<RawFileEntry>,
    },
}

/// Represents a file in a multi-file torrent
#[derive(Debug, Deserialize, Serialize)]
struct RawFileEntry {
    /// File size in bytes
    length: u64,
    /// File path components
    path: Vec<String>,
    /// Optional MD5 checksum
    md5sum: Option<String>,
}

/// Parser for .torrent files
pub struct TorrentParser;

impl TorrentParser {
    /// Parse a .torrent file from bytes
    pub fn parse_bytes(data: &[u8]) -> Result<TorrentInfo> {
        info!("Parsing torrent file from {} bytes", data.len());
        trace!("Torrent data (first 100 bytes): {:?}", &data[..data.len().min(100)]);
        
        let raw: RawTorrentFile = de::from_bytes(data)
            .map_err(|e| {
                error!("Failed to parse bencode data: {}", e);
                TorrentError::parse_error_with_source("Failed to parse bencode data", e.to_string())
            })?;
        
        debug!("Successfully parsed raw torrent file");
        Self::convert_to_torrent_info(raw, data)
    }

    /// Parse a .torrent file from a file path
    pub fn parse_file(path: &std::path::Path) -> Result<TorrentInfo> {
        info!("Loading torrent file from: {}", path.display());
        
        let data = std::fs::read(path)
            .map_err(|e| {
                error!("Failed to read torrent file '{}': {}", path.display(), e);
                TorrentError::storage_error_full("Failed to read torrent file", path.display().to_string(), e.to_string())
            })?;
        
        debug!("Read {} bytes from torrent file", data.len());
        Self::parse_bytes(&data)
    }

    /// Convert raw parsed data to TorrentInfo
    fn convert_to_torrent_info(raw: RawTorrentFile, original_data: &[u8]) -> Result<TorrentInfo> {
        debug!("Converting raw torrent data to TorrentInfo");
        
        // Calculate info hash from the info dictionary
        let info_dict_bytes = ser::to_bytes(&raw.info)
            .map_err(|e| {
                error!("Failed to serialize info dictionary: {}", e);
                TorrentError::parse_error_with_source("Failed to serialize info dictionary", e.to_string())
            })?;
        let info_hash = TorrentInfo::generate_info_hash(&info_dict_bytes);
        debug!("Generated info hash: {}", hex::encode(info_hash));

        // Parse piece hashes
        let pieces = TorrentInfo::parse_piece_hashes(&raw.info.pieces)
            .map_err(|e| {
                error!("Failed to parse piece hashes: {}", e);
                TorrentError::parse_error_with_source("Failed to parse piece hashes", e.to_string())
            })?;
        debug!("Parsed {} piece hashes", pieces.len());

        // Build announce list
        let announce_list = Self::build_announce_list(&raw.announce, &raw.announce_list);
        debug!("Built announce list with {} trackers", announce_list.len());
        if !announce_list.is_empty() {
            trace!("Announce URLs: {:?}", announce_list);
        }

        // Get primary announce URL
        let announce = if let Some(url) = raw.announce {
            url
        } else if !announce_list.is_empty() {
            announce_list[0].clone()
        } else {
            warn!("Torrent file has no announce URLs");
            return Err(TorrentError::parse_error("Torrent file has no announce URLs").into());
        };

        // Handle single-file or multi-file mode
        let (length, files) = match raw.info.mode {
            TorrentMode::Single { length: len, .. } => {
                debug!("Single-file torrent: {} bytes", len);
                (Some(len), None)
            }
            TorrentMode::Multi { files: raw_files } => {
                debug!("Multi-file torrent with {} files", raw_files.len());
                let files: Vec<TorrentFile> = raw_files
                    .into_iter()
                    .map(|rf| TorrentFile {
                        path: rf.path,
                        length: rf.length,
                    })
                    .collect();
                (None, Some(files))
            }
        };

        info!("Successfully converted torrent info: {}", raw.info.name);
        Ok(TorrentInfo {
            announce,
            announce_list,
            info_hash,
            piece_length: raw.info.piece_length,
            pieces,
            name: raw.info.name,
            length,
            files,
        })
    }

    /// Build a complete list of announce URLs from announce and announce-list
    fn build_announce_list(
        announce: &Option<String>,
        announce_list: &Option<Vec<Vec<String>>>,
    ) -> Vec<String> {
        let mut urls = Vec::new();

        // Add primary announce URL first
        if let Some(url) = announce {
            debug!("Adding primary announce URL: {}", url);
            urls.push(url.clone());
        }

        // Add URLs from announce-list tiers, avoiding duplicates
        if let Some(tiers) = announce_list {
            debug!("Processing {} tracker tiers", tiers.len());
            for (tier_index, tier) in tiers.iter().enumerate() {
                trace!("Tier {} has {} trackers", tier_index, tier.len());
                for url in tier {
                    if !urls.contains(url) {
                        debug!("Adding tracker from tier {}: {}", tier_index, url);
                        urls.push(url.clone());
                    } else {
                        trace!("Skipping duplicate tracker: {}", url);
                    }
                }
            }
        }

        urls
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_announce_list() {
        // Test with only announce
        let list = TorrentParser::build_announce_list(
            &Some("http://tracker1.com".to_string()),
            &None,
        );
        assert_eq!(list, vec!["http://tracker1.com"]);

        // Test with announce and announce-list
        let list = TorrentParser::build_announce_list(
            &Some("http://tracker1.com".to_string()),
            &Some(vec![
                vec!["http://tracker2.com".to_string()],
                vec!["http://tracker3.com".to_string()],
            ]),
        );
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], "http://tracker1.com");

        // Test with only announce-list
        let list = TorrentParser::build_announce_list(
            &None,
            &Some(vec![
                vec!["http://tracker1.com".to_string(), "http://tracker2.com".to_string()],
            ]),
        );
        assert_eq!(list.len(), 2);

        // Test duplicate removal
        let list = TorrentParser::build_announce_list(
            &Some("http://tracker1.com".to_string()),
            &Some(vec![
                vec!["http://tracker1.com".to_string(), "http://tracker2.com".to_string()],
            ]),
        );
        assert_eq!(list.len(), 2);
    }
}
