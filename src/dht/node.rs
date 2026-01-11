//! DHT node module
//!
//! Represents a node in the DHT network.

use std::net::SocketAddr;
use std::time::Instant;
use serde::{Deserialize, Serialize};

/// DHT node identifier (20 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 20]);

impl NodeId {
    /// Create a new NodeId from bytes
    pub fn new(id: [u8; 20]) -> Self {
        Self(id)
    }

    /// Generate a random NodeId
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut id = [0u8; 20];
        rng.fill(&mut id);
        Self(id)
    }

    /// Get the NodeId as bytes
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Get the NodeId as a hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse a NodeId from a hex string
    pub fn from_hex(hex_str: &str) -> Option<Self> {
        hex::decode(hex_str)
            .ok()
            .and_then(|bytes| {
                if bytes.len() == 20 {
                    let mut id = [0u8; 20];
                    id.copy_from_slice(&bytes);
                    Some(Self(id))
                } else {
                    None
                }
            })
    }
}

/// Represents a DHT node
#[derive(Debug, Clone)]
pub struct Node {
    /// Node identifier
    pub id: NodeId,
    /// Node address
    pub addr: SocketAddr,
    /// When the node was last contacted
    pub last_seen: Instant,
}

impl Node {
    /// Create a new node
    pub fn new(id: NodeId, addr: SocketAddr) -> Self {
        Self {
            id,
            addr,
            last_seen: Instant::now(),
        }
    }

    /// Create a node with a random ID
    pub fn with_random_id(addr: SocketAddr) -> Self {
        Self {
            id: NodeId::random(),
            addr,
            last_seen: Instant::now(),
        }
    }

    /// Calculate XOR distance to another node
    pub fn distance_to(&self, other: &NodeId) -> [u8; 20] {
        let mut distance = [0u8; 20];
        for i in 0..20 {
            distance[i] = self.id.0[i] ^ other.0[i];
        }
        distance
    }

    /// Check if node is responsive (seen within 15 minutes)
    pub fn is_good(&self) -> bool {
        self.last_seen.elapsed() < std::time::Duration::from_secs(900)
    }

    /// Update last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = Instant::now();
    }

    /// Get the time since last seen
    pub fn time_since_seen(&self) -> std::time::Duration {
        self.last_seen.elapsed()
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Node {}

impl std::hash::Hash for Node {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_new() {
        let id_bytes = [1u8; 20];
        let node_id = NodeId::new(id_bytes);
        assert_eq!(node_id.0, id_bytes);
    }

    #[test]
    fn test_node_id_random() {
        let node_id1 = NodeId::random();
        let node_id2 = NodeId::random();
        assert_ne!(node_id1, node_id2);
    }

    #[test]
    fn test_node_id_hex() {
        let id_bytes = [0xABu8; 20];
        let node_id = NodeId::new(id_bytes);
        let hex_str = node_id.to_hex();
        assert_eq!(hex_str.len(), 40);
        assert!(hex_str.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_node_id_from_hex() {
        let hex_str = "ab".repeat(20);
        let node_id = NodeId::from_hex(&hex_str);
        assert!(node_id.is_some());
        assert_eq!(node_id.unwrap().0, [0xABu8; 20]);
    }

    #[test]
    fn test_node_new() {
        let id = NodeId::new([1u8; 20]);
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let node = Node::new(id, addr);
        assert_eq!(node.id, id);
        assert_eq!(node.addr, addr);
        assert!(node.is_good());
    }

    #[test]
    fn test_node_distance_to() {
        let id1 = NodeId::new([0xFFu8; 20]);
        let id2 = NodeId::new([0x00u8; 20]);
        let node = Node::new(id1, "127.0.0.1:6881".parse().unwrap());
        let distance = node.distance_to(&id2);
        assert_eq!(distance, [0xFFu8; 20]);
    }

    #[test]
    fn test_node_update_last_seen() {
        let mut node = Node::new(NodeId::new([1u8; 20]), "127.0.0.1:6881".parse().unwrap());
        std::thread::sleep(std::time::Duration::from_millis(10));
        node.update_last_seen();
        assert!(node.time_since_seen() < std::time::Duration::from_millis(20));
    }
}
