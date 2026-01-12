//! Torrent file parser
//!
//! Handles parsing of .torrent files and extracting metadata.

use serde_bencode::{ser};
use serde::{Serialize};
use anyhow::Result;
use tracing::{debug, error, info, trace, warn};

use crate::torrent::info::{TorrentInfo, TorrentFile};
use crate::error::TorrentError;

/// Parser for .torrent files
pub struct TorrentParser;

impl TorrentParser {
    /// Parse a .torrent file from bytes
    pub fn parse_bytes(data: &[u8]) -> Result<TorrentInfo> {
        info!("Parsing torrent file from {} bytes", data.len());
        trace!("Torrent data (first 100 bytes): {:?}", &data[..data.len().min(100)]);

        // Use custom bencode parser
        let parsed = Self::parse_bencode(data)?;

        // Extract torrent metadata
        Self::convert_to_torrent_info(parsed, data)
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

    /// Simple bencode parser
    fn parse_bencode(data: &[u8]) -> Result<BencodeValue> {
        let mut idx = 0;
        let value = Self::parse_value(data, &mut idx)?;

        if idx != data.len() {
            warn!("Parsed {}/{} bytes", idx, data.len());
        }

        Ok(value)
    }

    fn parse_value(data: &[u8], idx: &mut usize) -> Result<BencodeValue> {
        if *idx >= data.len() {
            return Err(anyhow::anyhow!("Unexpected end of data"));
        }

        let byte = data[*idx];

        match byte {
            b'i' => {
                // Integer
                *idx += 1;
                let end = data[*idx..].iter().position(|&b| b == b'e')
                    .ok_or_else(|| anyhow::anyhow!("Unterminated integer"))? + *idx;
                let num_str = std::str::from_utf8(&data[*idx..end])?;
                let value: i64 = num_str.parse()?;
                *idx = end + 1;
                Ok(BencodeValue::Int(value))
            }
            b'l' => {
                // List
                *idx += 1;
                let mut list = Vec::new();
                while *idx < data.len() && data[*idx] != b'e' {
                    list.push(Self::parse_value(data, idx)?);
                }
                *idx += 1; // skip 'e'
                Ok(BencodeValue::List(list))
            }
            b'd' => {
                // Dictionary
                *idx += 1;
                let mut dict = std::collections::BTreeMap::new();
                while *idx < data.len() && data[*idx] != b'e' {
                    let key = match Self::parse_value(data, idx)? {
                        BencodeValue::Bytes(b) => b,
                        _ => return Err(anyhow::anyhow!("Dictionary key must be bytes")),
                    };
                    let value = Self::parse_value(data, idx)?;
                    dict.insert(key, value);
                }
                *idx += 1; // skip 'e'
                Ok(BencodeValue::Dict(dict))
            }
            b'0'..=b'9' => {
                // Byte string
                let colon = data[*idx..].iter().position(|&b| b == b':')
                    .ok_or_else(|| anyhow::anyhow!("Unterminated string length"))? + *idx;
                let len_str = std::str::from_utf8(&data[*idx..colon])?;
                let length: usize = len_str.parse()?;
                *idx = colon + 1;
                let start = *idx;
                *idx += length;
                Ok(BencodeValue::Bytes(data[start..*idx].to_vec()))
            }
            _ => Err(anyhow::anyhow!("Unknown bencode type: {}", byte)),
        }
    }

    fn convert_to_torrent_info(parsed: BencodeValue, original_data: &[u8]) -> Result<TorrentInfo> {
        let root_dict = match parsed {
            BencodeValue::Dict(d) => d,
            _ => return Err(anyhow::anyhow!("Root must be a dictionary")),
        };

        // Helper to get bytes from dict
        fn get_bytes<'a>(
            dict: &'a std::collections::BTreeMap<Vec<u8>, BencodeValue>,
            key: &[u8]
        ) -> Option<&'a [u8]> {
            dict.iter().find(|(k, _)| k.as_slice() == key).and_then(|(_, v)| v.as_bytes())
        }

        // Get announce URL
        let announce_bytes = get_bytes(&root_dict, b"announce")
            .ok_or_else(|| anyhow::anyhow!("Missing announce field"))?;
        let announce = String::from_utf8_lossy(announce_bytes).to_string();

        // Get announce list
        let mut announce_list = vec![announce.clone()];
        if let Some(BencodeValue::List(tiers)) = root_dict.get(&b"announce-list".to_vec()) {
            for tier in tiers {
                if let BencodeValue::List(urls) = tier {
                    for url_bytes in urls {
                        if let Some(bytes) = url_bytes.as_bytes() {
                            let url = String::from_utf8_lossy(bytes).to_string();
                            if !announce_list.contains(&url) {
                                announce_list.push(url);
                            }
                        }
                    }
                }
            }
        }

        // Get info dict
        let info_dict = root_dict.get(&b"info".to_vec())
            .and_then(|v| v.as_dict())
            .ok_or_else(|| anyhow::anyhow!("Missing info dictionary"))?;

        // Get name
        let name_bytes = get_bytes(info_dict, b"name")
            .ok_or_else(|| anyhow::anyhow!("Missing name field"))?;
        let name = String::from_utf8_lossy(name_bytes).to_string();

        // Get piece length
        let piece_length = info_dict.get(&b"piece length".to_vec())
            .and_then(|v| v.as_int())
            .ok_or_else(|| anyhow::anyhow!("Missing piece length"))? as u64;

        // Get pieces
        let pieces_bytes = get_bytes(info_dict, b"pieces")
            .ok_or_else(|| anyhow::anyhow!("Missing pieces field"))?;
        let pieces = TorrentInfo::parse_piece_hashes(pieces_bytes)?;

        // Check if it's single or multi-file
        let (length, files) = if info_dict.contains_key(&b"length".to_vec()) {
            // Single file
            let len = info_dict.get(&b"length".to_vec())
                .and_then(|v| v.as_int())
                .ok_or_else(|| anyhow::anyhow!("Invalid length field"))? as u64;
            (Some(len), None)
        } else if let Some(BencodeValue::List(file_list)) = info_dict.get(&b"files".to_vec()) {
            // Multi-file
            let mut torrent_files = Vec::new();
            for file_entry in file_list {
                if let BencodeValue::Dict(file_dict) = file_entry {
                    let file_len = file_dict.get(&b"length".to_vec())
                        .and_then(|v| v.as_int())
                        .ok_or_else(|| anyhow::anyhow!("Missing file length"))? as u64;

                    let path_list = file_dict.get(&b"path".to_vec())
                        .and_then(|v| v.as_list())
                        .ok_or_else(|| anyhow::anyhow!("Missing file path"))?;

                    let mut path = Vec::new();
                    for path_component in path_list {
                        if let Some(bytes) = path_component.as_bytes() {
                            path.push(String::from_utf8_lossy(bytes).to_string());
                        }
                    }

                    torrent_files.push(TorrentFile {
                        path,
                        length: file_len,
                    });
                }
            }
            (None, Some(torrent_files))
        } else {
            return Err(anyhow::anyhow!("Neither length nor files found in info dict"));
        };

        // Calculate info hash
        let info_start = original_data.iter().position(|&b| b == b'd')
            .ok_or_else(|| anyhow::anyhow!("Could not find info dict start"))?;
        let info_bytes = Self::extract_info_dict(original_data, info_start)?;
        let info_hash = TorrentInfo::generate_info_hash(&info_bytes);

        info!("Successfully converted torrent info: {}", name);
        Ok(TorrentInfo {
            announce,
            announce_list,
            info_hash,
            piece_length,
            pieces,
            name,
            length,
            files,
        })
    }

    /// Extract the info dictionary bytes for info hash calculation
    fn extract_info_dict(data: &[u8], start: usize) -> Result<Vec<u8>> {
        let mut idx = start + 1; // Skip initial 'd'
        let mut depth = 1;

        while idx < data.len() && depth > 0 {
            match data[idx] {
                b'd' => depth += 1,
                b'l' => depth += 1,
                b'e' => depth -= 1,
                b'i' => {
                    // Skip integer
                    idx = data[idx..].iter().position(|&b| b == b'e').map_or(idx, |p| idx + p + 1);
                    continue;
                }
                b'0'..=b'9' => {
                    // Skip string
                    if let Some(colon) = data[idx..].iter().position(|&b| b == b':') {
                        if let Ok(len) = std::str::from_utf8(&data[idx..idx + colon]).unwrap_or("0").parse::<usize>() {
                            idx = idx + colon + 1 + len;
                            continue;
                        }
                    }
                }
                _ => {}
            }
            idx += 1;
        }

        Ok(data[start..idx].to_vec())
    }
}

/// Bencode value
#[derive(Debug, Clone)]
enum BencodeValue {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<BencodeValue>),
    Dict(std::collections::BTreeMap<Vec<u8>, BencodeValue>),
}

impl BencodeValue {
    fn as_int(&self) -> Option<i64> {
        match self {
            BencodeValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            BencodeValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&[BencodeValue]> {
        match self {
            BencodeValue::List(l) => Some(l),
            _ => None,
        }
    }

    fn as_dict(&self) -> Option<&std::collections::BTreeMap<Vec<u8>, BencodeValue>> {
        match self {
            BencodeValue::Dict(d) => Some(d),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bencode_int() {
        let data = b"i42e";
        let mut idx = 0;
        let value = TorrentParser::parse_value(data, &mut idx).unwrap();
        assert_eq!(value.as_int(), Some(42));
    }

    #[test]
    fn test_parse_bencode_string() {
        let data = b"4:test";
        let mut idx = 0;
        let value = TorrentParser::parse_value(data, &mut idx).unwrap();
        assert_eq!(value.as_bytes(), Some(b"test".as_ref()));
    }

    #[test]
    fn test_parse_bencode_list() {
        let data = b"l4:testi42ee";
        let mut idx = 0;
        let value = TorrentParser::parse_value(data, &mut idx).unwrap();
        assert!(value.as_list().is_some());
    }

    #[test]
    fn test_parse_bencode_dict() {
        let data = b"d4:testi42ee";
        let mut idx = 0;
        let value = TorrentParser::parse_value(data, &mut idx).unwrap();
        assert!(value.as_dict().is_some());
    }
}
