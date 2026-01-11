//! BitTorrent handshake protocol
//!
//! Handles the initial handshake between peers.

use bytes::{BufMut, BytesMut};
use anyhow::Result;
use tracing::{debug, error, info, trace, warn};

use crate::error::TorrentError;

/// BitTorrent protocol identifier string
pub const PROTOCOL_STRING: &str = "BitTorrent protocol";

/// Length of the protocol string
pub const PROTOCOL_LENGTH: u8 = 19;

/// BitTorrent handshake message
#[derive(Debug, Clone)]
pub struct Handshake {
    /// Protocol identifier (19 bytes)
    pub protocol_id: [u8; 19],
    /// Extension bits
    pub extensions: u8,
    /// Torrent info hash
    pub info_hash: [u8; 20],
    /// Our peer ID
    pub peer_id: [u8; 20],
}

impl Handshake {
    /// Create a new handshake with info_hash and peer_id
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        debug!("Creating new handshake for info_hash: {}", hex::encode(info_hash));
        Self {
            protocol_id: PROTOCOL_STRING.as_bytes().try_into().unwrap(),
            extensions: 0,
            info_hash,
            peer_id,
        }
    }

    /// Create a new handshake with extension support
    pub fn with_extensions(info_hash: [u8; 20], peer_id: [u8; 20], extensions: u8) -> Self {
        debug!("Creating handshake with extensions: 0x{:02x}", extensions);
        Self {
            protocol_id: PROTOCOL_STRING.as_bytes().try_into().unwrap(),
            extensions,
            info_hash,
            peer_id,
        }
    }

    /// Generate a random peer ID with "-RU" prefix
    pub fn generate_peer_id() -> [u8; 20] {
        let mut peer_id = [0u8; 20];
        peer_id[0..3].copy_from_slice(b"-RU");
        peer_id[3..8].copy_from_slice(b"0000-");
        peer_id[8..].copy_from_slice(&rand::random::<[u8; 12]>());
        info!("Generated new peer ID: {}", hex::encode(peer_id));
        peer_id
    }

    /// Serialize the handshake to bytes
    pub fn serialize(&self) -> Vec<u8> {
        trace!("Serializing handshake");
        let mut buf = BytesMut::with_capacity(68);
        buf.put_u8(PROTOCOL_LENGTH);
        buf.put_slice(&self.protocol_id);
        buf.put_u8(self.extensions);
        buf.put_slice(&[0u8; 7]); // Reserved bytes
        buf.put_slice(&self.info_hash);
        buf.put_slice(&self.peer_id);
        trace!("Handshake serialized: {} bytes", buf.len());
        buf.to_vec()
    }

    /// Deserialize a handshake from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        trace!("Deserializing handshake from {} bytes", data.len());
        
        if data.len() < 68 {
            error!("Handshake data too short: expected 68 bytes, got {}", data.len());
            return Err(TorrentError::protocol_error_with_source(
                "Handshake data too short",
                format!("expected 68 bytes, got {}", data.len())
            ).into());
        }

        let protocol_length = data[0];
        if protocol_length != PROTOCOL_LENGTH {
            error!("Invalid protocol length: expected {}, got {}", PROTOCOL_LENGTH, protocol_length);
            return Err(TorrentError::protocol_error_with_source(
                "Invalid protocol length",
                format!("expected {}, got {}", PROTOCOL_LENGTH, protocol_length)
            ).into());
        }

        let protocol_id: [u8; 19] = data[1..20].try_into()
            .map_err(|e: std::array::TryFromSliceError| {
                error!("Failed to parse protocol_id: {}", e);
                TorrentError::protocol_error_with_source("Failed to parse protocol_id", e.to_string())
            })?;

        if protocol_id != PROTOCOL_STRING.as_bytes() {
            error!("Invalid protocol string");
            return Err(TorrentError::protocol_error("Invalid protocol string").into());
        }

        let extensions = data[20];
        debug!("Handshake extensions: 0x{:02x}", extensions);
        // Skip 7 reserved bytes (data[21..28])
        let mut info_hash = [0u8; 20];
        info_hash.copy_from_slice(&data[28..48]);
        debug!("Handshake info_hash: {}", hex::encode(info_hash));

        let mut peer_id = [0u8; 20];
        peer_id.copy_from_slice(&data[48..68]);
        debug!("Handshake peer_id: {}", hex::encode(peer_id));

        info!("Successfully deserialized handshake");
        Ok(Self {
            protocol_id,
            extensions,
            info_hash,
            peer_id,
        })
    }

    /// Validate the handshake protocol and info_hash
    pub fn validate(&self, expected_info_hash: &[u8; 20]) -> bool {
        debug!("Validating handshake against expected info_hash: {}", hex::encode(expected_info_hash));
        
        // Check protocol identifier
        if self.protocol_id != PROTOCOL_STRING.as_bytes() {
            warn!("Handshake validation failed: invalid protocol identifier");
            return false;
        }

        // Check info hash
        if self.info_hash != *expected_info_hash {
            warn!("Handshake validation failed: info hash mismatch");
            warn!("  Expected: {}", hex::encode(expected_info_hash));
            warn!("  Got:      {}", hex::encode(self.info_hash));
            return false;
        }

        debug!("Handshake validation successful");
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_serialize_deserialize() {
        let info_hash = [1u8; 20];
        let peer_id = [2u8; 20];
        let handshake = Handshake::new(info_hash, peer_id);

        let serialized = handshake.serialize();
        assert_eq!(serialized.len(), 68);

        let deserialized = Handshake::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.protocol_id, handshake.protocol_id);
        assert_eq!(deserialized.extensions, handshake.extensions);
        assert_eq!(deserialized.info_hash, handshake.info_hash);
        assert_eq!(deserialized.peer_id, handshake.peer_id);
    }

    #[test]
    fn test_generate_peer_id() {
        let peer_id = Handshake::generate_peer_id();
        assert_eq!(&peer_id[0..3], b"-RU");
        assert_eq!(peer_id.len(), 20);
    }

    #[test]
    fn test_handshake_validate() {
        let info_hash = [1u8; 20];
        let peer_id = [2u8; 20];
        let handshake = Handshake::new(info_hash, peer_id);

        assert!(handshake.validate(&info_hash));

        let wrong_info_hash = [3u8; 20];
        assert!(!handshake.validate(&wrong_info_hash));
    }

    #[test]
    fn test_handshake_with_extensions() {
        let info_hash = [1u8; 20];
        let peer_id = [2u8; 20];
        let handshake = Handshake::with_extensions(info_hash, peer_id, 0x01);

        assert_eq!(handshake.extensions, 0x01);
    }
}
