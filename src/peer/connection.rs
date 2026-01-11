//! Peer connection module
//!
//! Manages individual peer connections.

use crate::protocol::{Handshake, Message, BitTorrentWire, WireProtocol};
use crate::peer::{Peer, PeerState};
use crate::error::TorrentError;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use anyhow::Result;
use std::net::SocketAddr;
use tracing::{debug, error, info, trace, warn};

/// Represents a connected peer
pub struct PeerConnection {
    /// Peer information
    pub peer: Peer,
    /// TCP connection stream
    stream: TcpStream,
    /// Whether handshake has been completed
    pub handshake_completed: bool,
    /// Wire protocol handler
    wire: BitTorrentWire,
}

impl PeerConnection {
    /// Create a new peer connection from an existing socket
    pub fn from_socket(socket: TcpStream) -> Result<Self> {
        let peer_addr = socket.peer_addr()
            .map_err(|e| {
                error!("Failed to get peer address from socket: {}", e);
                TorrentError::peer_error_full("Failed to get peer address", "unknown".to_string(), e.to_string())
            })?;
        info!("Creating peer connection from socket: {}", peer_addr);
        Ok(Self {
            peer: Peer::new(peer_addr),
            stream: socket,
            handshake_completed: false,
            wire: BitTorrentWire,
        })
    }

    /// Create a new peer connection with a peer
    pub fn with_peer(socket: TcpStream, peer: Peer) -> Result<Self> {
        info!("Creating peer connection with peer: {}", peer.addr);
        Ok(Self {
            peer,
            stream: socket,
            handshake_completed: false,
            wire: BitTorrentWire,
        })
    }

    /// Connect to a peer at the given address and perform handshake
    pub async fn connect(addr: SocketAddr, info_hash: [u8; 20], our_peer_id: [u8; 20]) -> Result<Self> {
        info!("Connecting to peer: {}", addr);
        
        // Set connection timeout
        let socket = timeout(Duration::from_secs(10), TcpStream::connect(addr))
            .await
            .map_err(|e| {
                warn!("Connection timeout to {}", addr);
                TorrentError::network_error_full("Connection timeout", addr.to_string(), e.to_string())
            })?
            .map_err(|e| {
                error!("Failed to connect to {}: {}", addr, e);
                TorrentError::network_error_full("Failed to connect", addr.to_string(), e.to_string())
            })?;

        debug!("Connected to peer: {}", addr);
        let mut connection = Self::from_socket(socket)?;
        connection.peer.set_state(PeerState::Connecting);

        // Perform handshake
        connection.perform_handshake(info_hash, our_peer_id).await?;

        info!("Successfully connected and handshaked with peer: {}", addr);
        Ok(connection)
    }

    /// Perform the BitTorrent handshake
    async fn perform_handshake(&mut self, info_hash: [u8; 20], our_peer_id: [u8; 20]) -> Result<()> {
        info!("Performing handshake with peer: {}", self.peer.addr);
        
        // Create our handshake
        let our_handshake = Handshake::new(info_hash, our_peer_id);
        
        // Send our handshake
        debug!("Sending handshake to peer: {}", self.peer.addr);
        self.wire.write_handshake(&mut self.stream, &our_handshake).await
            .map_err(|e| {
                error!("Failed to send handshake to {}: {}", self.peer.addr, e);
                TorrentError::peer_error_full("Failed to send handshake", self.peer.addr.to_string(), e.to_string())
            })?;

        // Read peer's handshake
        debug!("Reading handshake from peer: {}", self.peer.addr);
        let peer_handshake = self.wire.read_handshake(&mut self.stream).await
            .map_err(|e| {
                error!("Failed to read handshake from {}: {}", self.peer.addr, e);
                TorrentError::peer_error_full("Failed to read handshake", self.peer.addr.to_string(), e.to_string())
            })?;

        // Validate peer's handshake
        if !peer_handshake.validate(&info_hash) {
            error!("Handshake validation failed with peer {}: info hash mismatch", self.peer.addr);
            return Err(TorrentError::peer_error_full(
                "Handshake validation failed: info hash mismatch",
                self.peer.addr.to_string(),
                "info hash mismatch".to_string()
            ).into());
        }

        // Update peer information
        self.peer.set_peer_id(peer_handshake.peer_id);
        self.peer.set_state(PeerState::Connected);
        self.handshake_completed = true;

        debug!("Handshake completed successfully with peer: {}", self.peer.addr);
        Ok(())
    }

    /// Send a message to the peer
    pub async fn send_message(&mut self, message: &Message) -> Result<()> {
        if !self.handshake_completed {
            error!("Attempted to send message before handshake completion to peer: {}", self.peer.addr);
            return Err(TorrentError::peer_error_full(
                "Cannot send message: handshake not completed",
                self.peer.addr.to_string(),
                "handshake not completed".to_string()
            ).into());
        }

        debug!("Sending {:?} message to peer: {}", message.message_id(), self.peer.addr);
        self.wire.write_message(&mut self.stream, message).await
            .map_err(|e| {
                error!("Failed to send message to {}: {}", self.peer.addr, e);
                TorrentError::peer_error_full("Failed to send message", self.peer.addr.to_string(), e.to_string())
            })?;
        Ok(())
    }

    /// Receive a message from the peer
    pub async fn receive_message(&mut self) -> Result<Message> {
        if !self.handshake_completed {
            error!("Attempted to receive message before handshake completion from peer: {}", self.peer.addr);
            return Err(TorrentError::peer_error_full(
                "Cannot receive message: handshake not completed",
                self.peer.addr.to_string(),
                "handshake not completed".to_string()
            ).into());
        }

        // Set read timeout
        let message = timeout(Duration::from_secs(30), self.wire.read_message(&mut self.stream))
            .await
            .map_err(|e| {
                warn!("Receive message timeout from peer: {}", self.peer.addr);
                TorrentError::peer_error_full("Receive message timeout", self.peer.addr.to_string(), e.to_string())
            })?
            .map_err(|e| {
                error!("Failed to read message from {}: {}", self.peer.addr, e);
                TorrentError::peer_error_full("Failed to read message", self.peer.addr.to_string(), e.to_string())
            })?;

        debug!("Received {:?} message from peer: {}", message.message_id(), self.peer.addr);
        Ok(message)
    }

    /// Request a piece block from the peer
    pub async fn request_piece(&mut self, piece_index: u32, begin: u32, length: u32) -> Result<()> {
        if !self.peer.can_request() {
            warn!("Cannot request piece from peer {}: peer is not ready", self.peer.addr);
            return Err(TorrentError::peer_error_full(
                "Cannot request piece: peer is not ready",
                self.peer.addr.to_string(),
                "peer is not ready".to_string()
            ).into());
        }

        debug!("Requesting piece {} block {} ({} bytes) from peer: {}", piece_index, begin, length, self.peer.addr);
        let request = Message::Request {
            index: piece_index,
            begin,
            length,
        };

        self.send_message(&request).await
    }

    /// Send our bitfield to the peer
    pub async fn send_bitfield(&mut self, bitfield: Vec<u8>) -> Result<()> {
        if !self.handshake_completed {
            error!("Attempted to send bitfield before handshake completion to peer: {}", self.peer.addr);
            return Err(TorrentError::peer_error_full(
                "Cannot send bitfield: handshake not completed",
                self.peer.addr.to_string(),
                "handshake not completed".to_string()
            ).into());
        }

        debug!("Sending bitfield ({} bytes) to peer: {}", bitfield.len(), self.peer.addr);
        let message = Message::Bitfield { bitfield };
        self.send_message(&message).await
    }

    /// Send interested message to the peer
    pub async fn send_interested(&mut self) -> Result<()> {
        debug!("Sending Interested to peer: {}", self.peer.addr);
        self.send_message(&Message::Interested).await?;
        self.peer.am_interested = true;
        Ok(())
    }

    /// Send not interested message to the peer
    pub async fn send_not_interested(&mut self) -> Result<()> {
        debug!("Sending NotInterested to peer: {}", self.peer.addr);
        self.send_message(&Message::NotInterested).await?;
        self.peer.am_interested = false;
        Ok(())
    }

    /// Send choke message to the peer
    pub async fn send_choke(&mut self) -> Result<()> {
        debug!("Sending Choke to peer: {}", self.peer.addr);
        self.send_message(&Message::Choke).await?;
        self.peer.am_choking = true;
        Ok(())
    }

    /// Send unchoke message to the peer
    pub async fn send_unchoke(&mut self) -> Result<()> {
        debug!("Sending Unchoke to peer: {}", self.peer.addr);
        self.send_message(&Message::Unchoke).await?;
        self.peer.am_choking = false;
        Ok(())
    }

    /// Send keep-alive message to the peer
    pub async fn send_keepalive(&mut self) -> Result<()> {
        trace!("Sending KeepAlive to peer: {}", self.peer.addr);
        self.send_message(&Message::KeepAlive).await
    }

    /// Close the connection
    pub async fn close(&mut self) -> Result<()> {
        info!("Closing connection to peer: {}", self.peer.addr);
        self.peer.set_state(PeerState::Disconnected);
        
        debug!("Connection closed to peer: {}", self.peer.addr);
        Ok(())
    }

    /// Get the peer's address
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer.addr
    }

    /// Get the peer's ID
    pub fn peer_id(&self) -> Option<[u8; 20]> {
        self.peer.peer_id
    }

    /// Get mutable reference to the peer
    pub fn peer_mut(&mut self) -> &mut Peer {
        &mut self.peer
    }

    /// Get reference to the peer
    pub fn peer_ref(&self) -> &Peer {
        &self.peer
    }

    /// Check if we are choking the peer
    pub fn am_choking(&self) -> bool {
        self.peer.am_choking
    }

    /// Check if the peer is choking us
    pub fn peer_choking(&self) -> bool {
        self.peer.peer_choking
    }

    /// Check if we are interested in the peer
    pub fn am_interested(&self) -> bool {
        self.peer.am_interested
    }

    /// Check if the peer is interested in us
    pub fn peer_interested(&self) -> bool {
        self.peer.peer_interested
    }

    /// Check if the connection is active
    pub fn is_active(&self) -> bool {
        self.handshake_completed && self.peer.state.is_connected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_connection_from_socket() {
        // This test would require a real socket, so we just verify the struct compiles
        // In a real test, we'd create a mock socket
    }
}
