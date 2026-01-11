//! Peer state module
//!
//! Defines peer information and state tracking.

use std::net::SocketAddr;
use serde::{Serialize, Deserialize};

/// Represents the state of a peer connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// Initial state, no connection
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Connected and ready
    Connected,
    /// We are choking the peer
    Choked,
    /// We are not choking the peer
    Unchoked,
    /// We are interested in the peer
    Interested,
    /// We are not interested in the peer
    NotInterested,
}

impl PeerState {
    /// Check if the peer is connected
    pub fn is_connected(&self) -> bool {
        matches!(self, PeerState::Connected)
    }

    /// Check if the peer can send data
    pub fn can_send(&self) -> bool {
        matches!(self, PeerState::Connected)
    }

    /// Check if the peer can receive data
    pub fn can_receive(&self) -> bool {
        matches!(self, PeerState::Connected)
    }
}

impl Default for PeerState {
    fn default() -> Self {
        PeerState::Disconnected
    }
}

/// Information about a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub peer_id: Option<[u8; 20]>,
    pub source: PeerSource,
}

/// Where the peer was discovered from
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerSource {
    Tracker,
    DHT,
    PEX,
    Manual,
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(addr: SocketAddr, source: PeerSource) -> Self {
        Self {
            addr,
            peer_id: None,
            source,
        }
    }

    /// Create a new peer info with peer ID
    pub fn with_peer_id(addr: SocketAddr, peer_id: [u8; 20], source: PeerSource) -> Self {
        Self {
            addr,
            peer_id: Some(peer_id),
            source,
        }
    }

    /// Get the peer ID as a hex string
    pub fn peer_id_hex(&self) -> Option<String> {
        self.peer_id.map(hex::encode)
    }
}

/// Represents a peer with its state and statistics
#[derive(Debug, Clone)]
pub struct Peer {
    /// Peer address
    pub addr: SocketAddr,
    /// Peer identifier
    pub peer_id: Option<[u8; 20]>,
    /// Current state
    pub state: PeerState,
    /// We're choking them
    pub am_choking: bool,
    /// We're interested
    pub am_interested: bool,
    /// They're choking us
    pub peer_choking: bool,
    /// They're interested
    pub peer_interested: bool,
    /// Pieces they have (bitfield)
    pub bitfield: Option<Vec<u8>>,
    /// Pieces downloaded from this peer
    pub pieces_downloaded: u32,
    /// Pieces uploaded to this peer
    pub pieces_uploaded: u32,
}

impl Peer {
    /// Create a new peer
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            peer_id: None,
            state: PeerState::Disconnected,
            am_choking: true,
            am_interested: false,
            peer_choking: true,
            peer_interested: false,
            bitfield: None,
            pieces_downloaded: 0,
            pieces_uploaded: 0,
        }
    }

    /// Create a new peer with peer ID
    pub fn with_peer_id(addr: SocketAddr, peer_id: [u8; 20]) -> Self {
        let mut peer = Self::new(addr);
        peer.peer_id = Some(peer_id);
        peer
    }

    /// Update peer's bitfield
    pub fn update_bitfield(&mut self, bitfield: Vec<u8>) {
        self.bitfield = Some(bitfield);
    }

    /// Check if peer has specific piece
    pub fn has_piece(&self, piece_index: usize) -> bool {
        if let Some(ref bitfield) = self.bitfield {
            let byte_index = piece_index / 8;
            let bit_index = 7 - (piece_index % 8);
            
            if byte_index < bitfield.len() {
                let byte = bitfield[byte_index];
                return (byte >> bit_index) & 1 == 1;
            }
        }
        false
    }

    /// Check if we can request from peer
    pub fn can_request(&self) -> bool {
        !self.peer_choking && self.am_interested && self.state.is_connected()
    }

    /// Get peer statistics
    pub fn stats(&self) -> PeerStats {
        PeerStats {
            addr: self.addr,
            peer_id: self.peer_id,
            state: self.state,
            am_choking: self.am_choking,
            am_interested: self.am_interested,
            peer_choking: self.peer_choking,
            peer_interested: self.peer_interested,
            pieces_downloaded: self.pieces_downloaded,
            pieces_uploaded: self.pieces_uploaded,
            has_bitfield: self.bitfield.is_some(),
        }
    }

    /// Set peer state
    pub fn set_state(&mut self, state: PeerState) {
        self.state = state;
    }

    /// Set peer ID
    pub fn set_peer_id(&mut self, peer_id: [u8; 20]) {
        self.peer_id = Some(peer_id);
    }

    /// Increment pieces downloaded
    pub fn increment_downloaded(&mut self) {
        self.pieces_downloaded = self.pieces_downloaded.saturating_add(1);
    }

    /// Increment pieces uploaded
    pub fn increment_uploaded(&mut self) {
        self.pieces_uploaded = self.pieces_uploaded.saturating_add(1);
    }

    /// Get the number of pieces the peer has
    pub fn piece_count(&self) -> usize {
        if let Some(ref bitfield) = self.bitfield {
            bitfield.iter().map(|byte| byte.count_ones() as usize).sum()
        } else {
            0
        }
    }
}

/// Peer statistics
#[derive(Debug, Clone)]
pub struct PeerStats {
    /// Peer address
    pub addr: SocketAddr,
    /// Peer identifier
    pub peer_id: Option<[u8; 20]>,
    /// Current state
    pub state: PeerState,
    /// We're choking them
    pub am_choking: bool,
    /// We're interested
    pub am_interested: bool,
    /// They're choking us
    pub peer_choking: bool,
    /// They're interested
    pub peer_interested: bool,
    /// Pieces downloaded from this peer
    pub pieces_downloaded: u32,
    /// Pieces uploaded to this peer
    pub pieces_uploaded: u32,
    /// Whether peer has sent bitfield
    pub has_bitfield: bool,
}

impl PeerStats {
    /// Get the peer ID as a hex string
    pub fn peer_id_hex(&self) -> Option<String> {
        self.peer_id.map(hex::encode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_new() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let peer = Peer::new(addr);
        
        assert_eq!(peer.addr, addr);
        assert!(peer.peer_id.is_none());
        assert_eq!(peer.state, PeerState::Disconnected);
        assert!(peer.am_choking);
        assert!(!peer.am_interested);
        assert!(peer.peer_choking);
        assert!(!peer.peer_interested);
        assert_eq!(peer.pieces_downloaded, 0);
        assert_eq!(peer.pieces_uploaded, 0);
    }

    #[test]
    fn test_peer_with_peer_id() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let peer_id = [1u8; 20];
        let peer = Peer::with_peer_id(addr, peer_id);
        
        assert_eq!(peer.addr, addr);
        assert_eq!(peer.peer_id, Some(peer_id));
    }

    #[test]
    fn test_update_bitfield() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let mut peer = Peer::new(addr);
        
        let bitfield = vec![0b11000000, 0b00000011];
        peer.update_bitfield(bitfield);
        
        assert!(peer.bitfield.is_some());
    }

    #[test]
    fn test_has_piece() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let mut peer = Peer::new(addr);
        
        let bitfield = vec![0b11000000]; // bits 0 and 1 are set
        peer.update_bitfield(bitfield);
        
        assert!(peer.has_piece(0));
        assert!(peer.has_piece(1));
        assert!(!peer.has_piece(2));
        assert!(!peer.has_piece(7));
    }

    #[test]
    fn test_can_request() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let mut peer = Peer::new(addr);
        
        // Default state - can't request
        assert!(!peer.can_request());
        
        // Set connected, but still choked
        peer.set_state(PeerState::Connected);
        assert!(!peer.can_request());
        
        // Unchoke peer, but not interested
        peer.peer_choking = false;
        assert!(!peer.can_request());
        
        // Set interested
        peer.am_interested = true;
        assert!(peer.can_request());
    }

    #[test]
    fn test_stats() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let peer = Peer::new(addr);
        
        let stats = peer.stats();
        assert_eq!(stats.addr, addr);
        assert_eq!(stats.state, PeerState::Disconnected);
    }

    #[test]
    fn test_increment_downloaded() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let mut peer = Peer::new(addr);
        
        peer.increment_downloaded();
        assert_eq!(peer.pieces_downloaded, 1);
        
        peer.increment_downloaded();
        assert_eq!(peer.pieces_downloaded, 2);
    }

    #[test]
    fn test_increment_uploaded() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let mut peer = Peer::new(addr);
        
        peer.increment_uploaded();
        assert_eq!(peer.pieces_uploaded, 1);
        
        peer.increment_uploaded();
        assert_eq!(peer.pieces_uploaded, 2);
    }

    #[test]
    fn test_piece_count() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let mut peer = Peer::new(addr);
        
        // No bitfield
        assert_eq!(peer.piece_count(), 0);
        
        // Bitfield with 3 bits set
        let bitfield = vec![0b11100000];
        peer.update_bitfield(bitfield);
        assert_eq!(peer.piece_count(), 3);
        
        // Multiple bytes
        let bitfield = vec![0b11111111, 0b00001111];
        peer.update_bitfield(bitfield);
        assert_eq!(peer.piece_count(), 12);
    }

    #[test]
    fn test_peer_state_defaults() {
        assert_eq!(PeerState::default(), PeerState::Disconnected);
    }
}
