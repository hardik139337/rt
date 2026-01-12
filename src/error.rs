//! Error types for the torrent downloader
//!
//! This module defines comprehensive error types for all components
//! of the BitTorrent downloader.

use std::fmt;
use std::fmt::Write;

/// Comprehensive error type for torrent operations
#[derive(Debug, Clone)]
pub enum TorrentError {
    /// Torrent file parsing errors
    ParseError {
        message: String,
        source: Option<String>,
    },

    /// BitTorrent protocol errors
    ProtocolError {
        message: String,
        source: Option<String>,
    },

    /// Peer connection errors
    PeerError {
        message: String,
        peer: Option<String>,
        source: Option<String>,
    },

    /// File I/O and storage errors
    StorageError {
        message: String,
        path: Option<String>,
        source: Option<String>,
    },
    
    /// Cloud storage errors (Google Drive, etc.)
    CloudStorageError {
        message: String,
        provider: String,
        source: Option<String>,
        is_retryable: bool,
    },

    /// DHT (Distributed Hash Table) errors
    DHTError {
        message: String,
        node: Option<String>,
        source: Option<String>,
    },

    /// Configuration errors
    ConfigError {
        message: String,
        field: Option<String>,
    },

    /// Network errors
    NetworkError {
        message: String,
        address: Option<String>,
        source: Option<String>,
    },

    /// Validation errors
    ValidationError {
        message: String,
        field: Option<String>,
    },
}

impl TorrentError {
    /// Create a new ParseError
    pub fn parse_error(message: impl Into<String>) -> Self {
        TorrentError::ParseError {
            message: message.into(),
            source: None,
        }
    }

    /// Create a new ParseError with source
    pub fn parse_error_with_source(message: impl Into<String>, source: impl Into<String>) -> Self {
        TorrentError::ParseError {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Create a new ProtocolError
    pub fn protocol_error(message: impl Into<String>) -> Self {
        TorrentError::ProtocolError {
            message: message.into(),
            source: None,
        }
    }

    /// Create a new ProtocolError with source
    pub fn protocol_error_with_source(message: impl Into<String>, source: impl Into<String>) -> Self {
        TorrentError::ProtocolError {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Create a new PeerError
    pub fn peer_error(message: impl Into<String>) -> Self {
        TorrentError::PeerError {
            message: message.into(),
            peer: None,
            source: None,
        }
    }

    /// Create a new PeerError with peer address
    pub fn peer_error_with_peer(message: impl Into<String>, peer: impl Into<String>) -> Self {
        TorrentError::PeerError {
            message: message.into(),
            peer: Some(peer.into()),
            source: None,
        }
    }

    /// Create a new PeerError with peer and source
    pub fn peer_error_full(message: impl Into<String>, peer: impl Into<String>, source: impl Into<String>) -> Self {
        TorrentError::PeerError {
            message: message.into(),
            peer: Some(peer.into()),
            source: Some(source.into()),
        }
    }

    /// Create a new StorageError
    pub fn storage_error(message: impl Into<String>) -> Self {
        TorrentError::StorageError {
            message: message.into(),
            path: None,
            source: None,
        }
    }

    /// Create a new StorageError with path
    pub fn storage_error_with_path(message: impl Into<String>, path: impl Into<String>) -> Self {
        TorrentError::StorageError {
            message: message.into(),
            path: Some(path.into()),
            source: None,
        }
    }

    /// Create a new StorageError with path and source
    pub fn storage_error_full(message: impl Into<String>, path: impl Into<String>, source: impl Into<String>) -> Self {
        TorrentError::StorageError {
            message: message.into(),
            path: Some(path.into()),
            source: Some(source.into()),
        }
    }
    
    /// Create a new CloudStorageError
    pub fn cloud_storage_error(message: impl Into<String>, provider: impl Into<String>) -> Self {
        TorrentError::CloudStorageError {
            message: message.into(),
            provider: provider.into(),
            source: None,
            is_retryable: false,
        }
    }
    
    /// Create a new CloudStorageError with source
    pub fn cloud_storage_error_with_source(
        message: impl Into<String>,
        provider: impl Into<String>,
        source: impl Into<String>
    ) -> Self {
        TorrentError::CloudStorageError {
            message: message.into(),
            provider: provider.into(),
            source: Some(source.into()),
            is_retryable: false,
        }
    }
    
    /// Create a new retryable CloudStorageError
    pub fn cloud_storage_error_retryable(
        message: impl Into<String>,
        provider: impl Into<String>,
        source: impl Into<String>
    ) -> Self {
        TorrentError::CloudStorageError {
            message: message.into(),
            provider: provider.into(),
            source: Some(source.into()),
            is_retryable: true,
        }
    }

    /// Create a new DHTError
    pub fn dht_error(message: impl Into<String>) -> Self {
        TorrentError::DHTError {
            message: message.into(),
            node: None,
            source: None,
        }
    }

    /// Create a new DHTError with node
    pub fn dht_error_with_node(message: impl Into<String>, node: impl Into<String>) -> Self {
        TorrentError::DHTError {
            message: message.into(),
            node: Some(node.into()),
            source: None,
        }
    }

    /// Create a new DHTError with node and source
    pub fn dht_error_full(message: impl Into<String>, node: impl Into<String>, source: impl Into<String>) -> Self {
        TorrentError::DHTError {
            message: message.into(),
            node: Some(node.into()),
            source: Some(source.into()),
        }
    }

    /// Create a new ConfigError
    pub fn config_error(message: impl Into<String>) -> Self {
        TorrentError::ConfigError {
            message: message.into(),
            field: None,
        }
    }

    /// Create a new ConfigError with field
    pub fn config_error_with_field(message: impl Into<String>, field: impl Into<String>) -> Self {
        TorrentError::ConfigError {
            message: message.into(),
            field: Some(field.into()),
        }
    }

    /// Create a new NetworkError
    pub fn network_error(message: impl Into<String>) -> Self {
        TorrentError::NetworkError {
            message: message.into(),
            address: None,
            source: None,
        }
    }

    /// Create a new NetworkError with address
    pub fn network_error_with_address(message: impl Into<String>, address: impl Into<String>) -> Self {
        TorrentError::NetworkError {
            message: message.into(),
            address: Some(address.into()),
            source: None,
        }
    }

    /// Create a new NetworkError with address and source
    pub fn network_error_full(message: impl Into<String>, address: impl Into<String>, source: impl Into<String>) -> Self {
        TorrentError::NetworkError {
            message: message.into(),
            address: Some(address.into()),
            source: Some(source.into()),
        }
    }

    /// Create a new ValidationError
    pub fn validation_error(message: impl Into<String>) -> Self {
        TorrentError::ValidationError {
            message: message.into(),
            field: None,
        }
    }

    /// Create a new ValidationError with field
    pub fn validation_error_with_field(message: impl Into<String>, field: impl Into<String>) -> Self {
        TorrentError::ValidationError {
            message: message.into(),
            field: Some(field.into()),
        }
    }

    /// Add context to the error
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        let ctx = context.into();
        match &mut self {
            TorrentError::ParseError { source, .. } => {
                *source = Some(source.as_ref().map_or_else(|| ctx.clone(), |s| format!("{}: {}", s, ctx)));
            }
            TorrentError::ProtocolError { source, .. } => {
                *source = Some(source.as_ref().map_or_else(|| ctx.clone(), |s| format!("{}: {}", s, ctx)));
            }
            TorrentError::PeerError { source, .. } => {
                *source = Some(source.as_ref().map_or_else(|| ctx.clone(), |s| format!("{}: {}", s, ctx)));
            }
            TorrentError::StorageError { source, .. } => {
                *source = Some(source.as_ref().map_or_else(|| ctx.clone(), |s| format!("{}: {}", s, ctx)));
            }
            TorrentError::DHTError { source, .. } => {
                *source = Some(source.as_ref().map_or_else(|| ctx.clone(), |s| format!("{}: {}", s, ctx)));
            }
            TorrentError::NetworkError { source, .. } => {
                *source = Some(source.as_ref().map_or_else(|| ctx.clone(), |s| format!("{}: {}", s, ctx)));
            }
            TorrentError::CloudStorageError { source, .. } => {
                *source = Some(source.as_ref().map_or_else(|| ctx.clone(), |s| format!("{}: {}", s, ctx)));
            }
            _ => {}
        }
        self
    }
}

impl fmt::Display for TorrentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TorrentError::ParseError { message, source } => {
                if let Some(src) = source {
                    write!(f, "Parse error: {} (source: {})", message, src)
                } else {
                    write!(f, "Parse error: {}", message)
                }
            }
            TorrentError::ProtocolError { message, source } => {
                if let Some(src) = source {
                    write!(f, "Protocol error: {} (source: {})", message, src)
                } else {
                    write!(f, "Protocol error: {}", message)
                }
            }
            TorrentError::PeerError { message, peer, source } => {
                match (peer, source) {
                    (Some(p), Some(s)) => write!(f, "Peer error: {} (peer: {}, source: {})", message, p, s),
                    (Some(p), None) => write!(f, "Peer error: {} (peer: {})", message, p),
                    (None, Some(s)) => write!(f, "Peer error: {} (source: {})", message, s),
                    (None, None) => write!(f, "Peer error: {}", message),
                }
            }
            TorrentError::StorageError { message, path, source } => {
                match (path, source) {
                    (Some(p), Some(s)) => write!(f, "Storage error: {} (path: {}, source: {})", message, p, s),
                    (Some(p), None) => write!(f, "Storage error: {} (path: {})", message, p),
                    (None, Some(s)) => write!(f, "Storage error: {} (source: {})", message, s),
                    (None, None) => write!(f, "Storage error: {}", message),
                }
            }
            TorrentError::CloudStorageError { message, provider, source, is_retryable } => {
                match (source, is_retryable) {
                    (Some(s), true) => write!(f, "Cloud storage error (retryable): {} [{}] (source: {})", message, provider, s),
                    (Some(s), false) => write!(f, "Cloud storage error: {} [{}] (source: {})", message, provider, s),
                    (None, true) => write!(f, "Cloud storage error (retryable): {} [{}]", message, provider),
                    (None, false) => write!(f, "Cloud storage error: {} [{}]", message, provider),
                }
            }
            TorrentError::DHTError { message, node, source } => {
                match (node, source) {
                    (Some(n), Some(s)) => write!(f, "DHT error: {} (node: {}, source: {})", message, n, s),
                    (Some(n), None) => write!(f, "DHT error: {} (node: {})", message, n),
                    (None, Some(s)) => write!(f, "DHT error: {} (source: {})", message, s),
                    (None, None) => write!(f, "DHT error: {}", message),
                }
            }
            TorrentError::ConfigError { message, field } => {
                if let Some(field_val) = field {
                    write!(f, "Config error: {} (field: {})", message, field_val)
                } else {
                    write!(f, "Config error: {}", message)
                }
            }
            TorrentError::NetworkError { message, address, source } => {
                match (address, source) {
                    (Some(a), Some(s)) => write!(f, "Network error: {} (address: {}, source: {})", message, a, s),
                    (Some(a), None) => write!(f, "Network error: {} (address: {})", message, a),
                    (None, Some(s)) => write!(f, "Network error: {} (source: {})", message, s),
                    (None, None) => write!(f, "Network error: {}", message),
                }
            }
            TorrentError::ValidationError { message, field } => {
                if let Some(field_val) = field {
                    write!(f, "Validation error: {} (field: {})", message, field_val)
                } else {
                    write!(f, "Validation error: {}", message)
                }
            }
        }
    }
}

impl std::error::Error for TorrentError {}

// Implement From traits for common error types

impl From<std::io::Error> for TorrentError {
    fn from(err: std::io::Error) -> Self {
        TorrentError::storage_error_full(err.to_string(), "unknown".to_string(), err.kind().to_string())
    }
}

// Note: serde_bencode::Error is the public type, not de::Error or ser::Error
impl From<serde_bencode::Error> for TorrentError {
    fn from(err: serde_bencode::Error) -> Self {
        TorrentError::parse_error_with_source("Failed to parse bencode data", err.to_string())
    }
}

impl From<serde_json::Error> for TorrentError {
    fn from(err: serde_json::Error) -> Self {
        TorrentError::storage_error_full("Failed to parse JSON data", "unknown".to_string(), err.to_string())
    }
}

impl From<std::net::AddrParseError> for TorrentError {
    fn from(err: std::net::AddrParseError) -> Self {
        TorrentError::network_error_full("Failed to parse address", "unknown".to_string(), err.to_string())
    }
}

impl From<tokio::time::error::Elapsed> for TorrentError {
    fn from(err: tokio::time::error::Elapsed) -> Self {
        TorrentError::network_error("Operation timed out")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        let err = TorrentError::parse_error("Invalid torrent file");
        assert_eq!(err.to_string(), "Parse error: Invalid torrent file");
    }

    #[test]
    fn test_parse_error_with_source() {
        let err = TorrentError::parse_error_with_source("Invalid torrent file", "bencode error");
        assert!(err.to_string().contains("Parse error"));
        assert!(err.to_string().contains("Invalid torrent file"));
        assert!(err.to_string().contains("bencode error"));
    }

    #[test]
    fn test_peer_error_with_peer() {
        let err = TorrentError::peer_error_with_peer("Connection failed", "127.0.0.1:6881");
        assert!(err.to_string().contains("Peer error"));
        assert!(err.to_string().contains("Connection failed"));
        assert!(err.to_string().contains("127.0.0.1:6881"));
    }

    #[test]
    fn test_storage_error_with_path() {
        let err = TorrentError::storage_error_with_path("File not found", "/path/to/file");
        assert!(err.to_string().contains("Storage error"));
        assert!(err.to_string().contains("File not found"));
        assert!(err.to_string().contains("/path/to/file"));
    }

    #[test]
    fn test_with_context() {
        let err = TorrentError::parse_error("Invalid data").with_context("while parsing torrent");
        assert!(err.to_string().contains("while parsing torrent"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let err: TorrentError = io_err.into();
        assert!(matches!(err, TorrentError::StorageError { .. }));
    }

    #[test]
    fn test_from_addr_parse_error() {
        let addr_err = "invalid:address".parse::<std::net::SocketAddr>().unwrap_err();
        let err: TorrentError = addr_err.into();
        assert!(matches!(err, TorrentError::NetworkError { .. }));
    }

    #[test]
    fn test_config_error_with_field() {
        let err = TorrentError::config_error_with_field("Invalid value", "max_connections");
        assert!(err.to_string().contains("Config error"));
        assert!(err.to_string().contains("max_connections"));
    }

    #[test]
    fn test_validation_error_with_field() {
        let err = TorrentError::validation_error_with_field("Value out of range", "port");
        assert!(err.to_string().contains("Validation error"));
        assert!(err.to_string().contains("port"));
    }

    #[test]
    fn test_cloud_storage_error() {
        let err = TorrentError::cloud_storage_error("Upload failed", "Google Drive");
        assert!(err.to_string().contains("Cloud storage error"));
        assert!(err.to_string().contains("Upload failed"));
        assert!(err.to_string().contains("Google Drive"));
    }

    #[test]
    fn test_cloud_storage_error_retryable() {
        let err = TorrentError::cloud_storage_error_retryable("Network timeout", "Google Drive", "timeout");
        assert!(err.to_string().contains("Cloud storage error (retryable)"));
        assert!(err.to_string().contains("Network timeout"));
    }
}
