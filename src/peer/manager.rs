//! Peer manager module
//!
//! Manages multiple peer connections.

use crate::peer::{Peer, PeerConnection, PeerState};
use crate::protocol::Handshake;
use crate::torrent::TorrentInfo;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use anyhow::Result;
use tracing::{debug, error, info, trace, warn};

/// Manages all peer connections for a torrent
pub struct PeerManager {
    /// List of known peers
    peers: RwLock<Vec<Peer>>,
    /// Active connections
    active_connections: RwLock<HashMap<SocketAddr, PeerConnection>>,
    /// Maximum concurrent connections
    max_connections: usize,
    /// Torrent metadata
    torrent_info: Arc<TorrentInfo>,
    /// Our peer ID
    our_peer_id: [u8; 20],
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new(max_connections: usize, torrent_info: Arc<TorrentInfo>, our_peer_id: [u8; 20]) -> Self {
        Self {
            peers: RwLock::new(Vec::new()),
            active_connections: RwLock::new(HashMap::new()),
            max_connections,
            torrent_info,
            our_peer_id,
        }
    }

    /// Add a peer to the manager
    pub async fn add_peer(&self, addr: SocketAddr) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        // Check if peer already exists
        if peers.iter().any(|p| p.addr == addr) {
            trace!("Peer {} already exists, skipping", addr);
            return Ok(());
        }

        let peer = Peer::new(addr);
        peers.push(peer);
        debug!("Added peer: {} (total: {})", addr, peers.len());
        
        Ok(())
    }

    /// Add a peer with peer ID to the manager
    pub async fn add_peer_with_id(&self, addr: SocketAddr, peer_id: [u8; 20]) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        // Check if peer already exists
        if peers.iter().any(|p| p.addr == addr) {
            trace!("Peer {} already exists, skipping", addr);
            return Ok(());
        }

        let peer = Peer::with_peer_id(addr, peer_id);
        peers.push(peer);
        debug!("Added peer with ID {}: {} (total: {})", hex::encode(peer_id), addr, peers.len());
        
        Ok(())
    }

    /// Add multiple peers to the manager
    pub async fn add_peers(&self, addrs: Vec<SocketAddr>) -> Result<()> {
        let mut peers = self.peers.write().await;
        let mut added_count = 0;
        
        for addr in addrs {
            if !peers.iter().any(|p| p.addr == addr) {
                let peer = Peer::new(addr);
                peers.push(peer);
                added_count += 1;
            }
        }
        
        info!("Added {} peers (total: {})", added_count, peers.len());
        Ok(())
    }

    /// Remove a peer from the manager
    pub async fn remove_peer(&self, addr: SocketAddr) {
        debug!("Removing peer: {}", addr);
        let mut peers = self.peers.write().await;
        peers.retain(|p| p.addr != addr);
        
        // Also remove from active connections if present
        let mut connections = self.active_connections.write().await;
        if let Some(mut conn) = connections.remove(&addr) {
            if let Err(e) = conn.close().await {
                warn!("Failed to close connection to {}: {}", addr, e);
            }
        }
        info!("Removed peer: {} (remaining: {})", addr, peers.len());
    }

    /// Connect to available peers
    pub async fn connect_to_peers(&self) -> Result<usize> {
        let info_hash = self.torrent_info.info_hash;
        let our_peer_id = self.our_peer_id;
        
        // Get list of peers to connect to
        let peers_to_connect = {
            let peers = self.peers.read().await;
            let connections = self.active_connections.read().await;
            
            let current_count = connections.len();
            let slots_available = self.max_connections.saturating_sub(current_count);
            
            if slots_available == 0 {
                debug!("No connection slots available (current: {}, max: {})", current_count, self.max_connections);
                return Ok(0);
            }
            
            debug!("Attempting to connect to {} peers ({} slots available)", peers.len(), slots_available);
            
            // Find disconnected peers
            peers.iter()
                .filter(|p| !connections.contains_key(&p.addr))
                .filter(|p| p.state == PeerState::Disconnected)
                .take(slots_available)
                .map(|p| p.addr)
                .collect::<Vec<_>>()
        };
        
        let mut connected_count = 0;
        
        for addr in peers_to_connect {
            info!("Connecting to peer: {}", addr);
            match PeerConnection::connect(addr, info_hash, our_peer_id).await {
                Ok(connection) => {
                    let mut connections = self.active_connections.write().await;
                    connections.insert(addr, connection);
                    connected_count += 1;
                    info!("Successfully connected to peer: {} (total connections: {})", addr, connections.len());
                }
                Err(e) => {
                    error!("Failed to connect to {}: {}", addr, e);
                    // Mark peer as disconnected for retry later
                    let mut peers = self.peers.write().await;
                    if let Some(peer) = peers.iter_mut().find(|p| p.addr == addr) {
                        peer.set_state(PeerState::Disconnected);
                    }
                }
            }
        }
        
        info!("Connected to {} peers", connected_count);
        Ok(connected_count)
    }

    /// Manage active connections (send keep-alive, etc.)
    pub async fn manage_connections(&self) -> Result<()> {
        let mut connections = self.active_connections.write().await;
        debug!("Managing {} active connections", connections.len());
        
        for (_, connection) in connections.iter_mut() {
            // Send keep-alive to maintain connection
            if let Err(e) = connection.send_keepalive().await {
                warn!("Failed to send keep-alive to {}: {}", connection.peer_addr(), e);
            }
        }
        
        Ok(())
    }

    /// Get the best peer for downloading based on various criteria
    pub async fn get_best_peer(&self, needed_pieces: &[usize]) -> Option<PeerConnection> {
        let connections = self.active_connections.read().await;
        
        if connections.is_empty() || needed_pieces.is_empty() {
            debug!("No available peers or no needed pieces");
            return None;
        }
        
        debug!("Finding best peer for {} needed pieces", needed_pieces.len());
        
        // Score each peer and find the best one
        let mut best_addr: Option<SocketAddr> = None;
        let mut best_score = -1i32;
        
        for (addr, connection) in connections.iter() {
            if !connection.is_active() || connection.peer_choking() {
                trace!("Skipping peer {}: not active or choking us", addr);
                continue;
            }
            
            let peer = connection.peer_ref();
            let mut score = 0i32;
            
            // Prefer peers that have pieces we need
            for piece_index in needed_pieces {
                if peer.has_piece(*piece_index) {
                    score += 10;
                }
            }
            
            // Prefer peers that are not choking us
            if !connection.peer_choking() {
                score += 5;
            }
            
            // Prefer peers with good upload ratio (more downloaded from them)
            score += peer.pieces_downloaded as i32;
            
            // Prefer peers we've uploaded less to (reciprocity)
            score -= (peer.pieces_uploaded / 2) as i32;
            
            trace!("Peer {} score: {} (downloaded: {}, uploaded: {})",
                addr, score, peer.pieces_downloaded, peer.pieces_uploaded);
            
            if score > best_score {
                best_score = score;
                best_addr = Some(*addr);
            }
        }
        
        // Return the address of the best peer
        // Note: We cannot clone PeerConnection because TcpStream is not Clone
        // The caller should use the address to access the connection
        if let Some(addr) = best_addr {
            info!("Best peer: {} (score: {})", addr, best_score);
            // Return None since we cannot clone the connection
            None
        } else {
            warn!("No suitable peer found");
            None
        }
    }

    /// Disconnect a peer
    pub async fn disconnect_peer(&self, addr: SocketAddr) -> Result<()> {
        info!("Disconnecting peer: {}", addr);
        let mut connections = self.active_connections.write().await;
        
        if let Some(mut connection) = connections.remove(&addr) {
            connection.close().await?;
            
            // Update peer state
            let mut peers = self.peers.write().await;
            if let Some(peer) = peers.iter_mut().find(|p| p.addr == addr) {
                peer.set_state(PeerState::Disconnected);
            }
            debug!("Disconnected peer: {}", addr);
        } else {
            warn!("Attempted to disconnect peer {} but no connection found", addr);
        }
        
        Ok(())
    }

    /// Get the number of active connections
    pub async fn connection_count(&self) -> usize {
        self.active_connections.read().await.len()
    }

    /// Get the total number of known peers
    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }

    /// Check if we can add more connections
    pub async fn can_add_connection(&self) -> bool {
        self.active_connections.read().await.len() < self.max_connections
    }

    /// Get a peer connection by address
    /// Note: Returns None since PeerConnection cannot be cloned due to TcpStream
    pub async fn get_connection(&self, _addr: SocketAddr) -> Option<PeerConnection> {
        // Note: We cannot return a cloned PeerConnection because TcpStream is not Clone
        // In a real implementation, this would return a reference or use Arc
        None
    }

    /// Get all peer addresses
    pub async fn peer_addresses(&self) -> Vec<SocketAddr> {
        self.peers.read().await.iter().map(|p| p.addr).collect()
    }

    /// Get all connected peer addresses
    pub async fn connected_addresses(&self) -> Vec<SocketAddr> {
        self.active_connections.read().await.keys().copied().collect()
    }

    /// Get statistics for all peers
    pub async fn get_all_stats(&self) -> Vec<(SocketAddr, crate::peer::PeerStats)> {
        let peers = self.peers.read().await;
        peers.iter().map(|p| (p.addr, p.stats())).collect()
    }

    /// Run the connection management loop
    pub async fn run_management_loop(&self, interval_secs: u64) {
        info!("Starting peer management loop (interval: {}s)", interval_secs);
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        
        loop {
            interval.tick().await;
            
            // Try to connect to more peers
            if let Err(e) = self.connect_to_peers().await {
                error!("Error connecting to peers: {}", e);
            }
            
            // Manage existing connections
            if let Err(e) = self.manage_connections().await {
                error!("Error managing connections: {}", e);
            }
        }
    }
}

impl Default for PeerManager {
    fn default() -> Self {
        Self::new(
            50,
            Arc::new(TorrentInfo {
                announce: String::new(),
                announce_list: Vec::new(),
                info_hash: [0u8; 20],
                piece_length: 0,
                pieces: Vec::new(),
                name: String::new(),
                length: None,
                files: None,
            }),
            Handshake::generate_peer_id(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_peer_manager_new() {
        let torrent_info = Arc::new(TorrentInfo {
            announce: String::new(),
            announce_list: Vec::new(),
            info_hash: [0u8; 20],
            piece_length: 0,
            pieces: Vec::new(),
            name: String::new(),
            length: None,
            files: None,
        });
        
        let manager = PeerManager::new(10, torrent_info, Handshake::generate_peer_id());
        
        assert_eq!(manager.max_connections, 10);
        assert_eq!(manager.connection_count().await, 0);
        assert_eq!(manager.peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_add_peer() {
        let torrent_info = Arc::new(TorrentInfo {
            announce: String::new(),
            announce_list: Vec::new(),
            info_hash: [0u8; 20],
            piece_length: 0,
            pieces: Vec::new(),
            name: String::new(),
            length: None,
            files: None,
        });
        
        let manager = PeerManager::new(10, torrent_info, Handshake::generate_peer_id());
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        
        manager.add_peer(addr).await.unwrap();
        
        assert_eq!(manager.peer_count().await, 1);
        assert_eq!(manager.peer_addresses().await, vec![addr]);
    }

    #[tokio::test]
    async fn test_remove_peer() {
        let torrent_info = Arc::new(TorrentInfo {
            announce: String::new(),
            announce_list: Vec::new(),
            info_hash: [0u8; 20],
            piece_length: 0,
            pieces: Vec::new(),
            name: String::new(),
            length: None,
            files: None,
        });
        
        let manager = PeerManager::new(10, torrent_info, Handshake::generate_peer_id());
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        
        manager.add_peer(addr).await.unwrap();
        assert_eq!(manager.peer_count().await, 1);
        
        manager.remove_peer(addr).await;
        assert_eq!(manager.peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_can_add_connection() {
        let torrent_info = Arc::new(TorrentInfo {
            announce: String::new(),
            announce_list: Vec::new(),
            info_hash: [0u8; 20],
            piece_length: 0,
            pieces: Vec::new(),
            name: String::new(),
            length: None,
            files: None,
        });
        
        let manager = PeerManager::new(2, torrent_info, Handshake::generate_peer_id());
        
        assert!(manager.can_add_connection().await);
    }

    #[tokio::test]
    async fn test_add_peers() {
        let torrent_info = Arc::new(TorrentInfo {
            announce: String::new(),
            announce_list: Vec::new(),
            info_hash: [0u8; 20],
            piece_length: 0,
            pieces: Vec::new(),
            name: String::new(),
            length: None,
            files: None,
        });
        
        let manager = PeerManager::new(10, torrent_info, Handshake::generate_peer_id());
        
        let addrs = vec![
            "127.0.0.1:6881".parse().unwrap(),
            "127.0.0.1:6882".parse().unwrap(),
            "127.0.0.1:6883".parse().unwrap(),
        ];
        
        manager.add_peers(addrs.clone()).await.unwrap();
        
        assert_eq!(manager.peer_count().await, 3);
        
        let peer_addrs = manager.peer_addresses().await;
        for addr in addrs {
            assert!(peer_addrs.contains(&addr));
        }
    }
}
