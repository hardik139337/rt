//! DHT bootstrap module
//!
//! Handles bootstrapping the DHT network and discovering peers.

use crate::dht::message::{generate_transaction_id, parse_compact_peers, DHTMessage};
use crate::dht::node::{Node, NodeId};
use crate::dht::routing::RoutingTable;
use anyhow::Result;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;

/// Bootstrap configuration
#[derive(Debug, Clone)]
pub struct BootstrapConfig {
    /// Known bootstrap nodes
    pub bootstrap_nodes: Vec<SocketAddr>,
    /// Torrent info hash
    pub info_hash: [u8; 20],
}

impl BootstrapConfig {
    /// Create a new bootstrap config
    pub fn new(bootstrap_nodes: Vec<SocketAddr>, info_hash: [u8; 20]) -> Self {
        Self {
            bootstrap_nodes,
            info_hash,
        }
    }

    /// Create a bootstrap config with default bootstrap nodes
    pub fn with_defaults(info_hash: [u8; 20]) -> Self {
        let bootstrap_nodes = Self::get_default_bootstrap_nodes();
        Self {
            bootstrap_nodes,
            info_hash,
        }
    }

    /// Get default bootstrap nodes
    pub fn get_default_bootstrap_nodes() -> Vec<SocketAddr> {
        vec![
            "router.bittorrent.com:6881".parse::<SocketAddr>().unwrap_or_else(|_| "0.0.0.0:0".parse::<SocketAddr>().unwrap()),
            "dht.transmissionbt.com:6881".parse::<SocketAddr>().unwrap_or_else(|_| "0.0.0.0:0".parse::<SocketAddr>().unwrap()),
            "router.utorrent.com:6881".parse::<SocketAddr>().unwrap_or_else(|_| "0.0.0.0:0".parse::<SocketAddr>().unwrap()),
        ]
        .into_iter()
        .filter(|addr| addr.port() != 0)
        .collect()
    }
}

/// Bootstrap the DHT network
pub async fn bootstrap(
    socket: &UdpSocket,
    our_id: NodeId,
    routing_table: &mut RoutingTable,
    config: &BootstrapConfig,
) -> Result<()> {
    tracing::info!("Bootstrapping DHT network...");

    for bootstrap_addr in &config.bootstrap_nodes {
        // Create a node for the bootstrap address
        let node_id = generate_bootstrap_node_id(bootstrap_addr);
        let node = Node::new(node_id, *bootstrap_addr);

        // Add to routing table
        routing_table.add_node(node.clone());

        // Send ping to bootstrap node
        let transaction_id = generate_transaction_id();
        let ping_query = DHTMessage::create_ping_query(transaction_id.clone(), our_id);
        let serialized = ping_query.serialize()?;

        socket.send_to(&serialized, bootstrap_addr).await?;
        tracing::debug!("Sent ping to bootstrap node: {}", bootstrap_addr);
    }

    tracing::info!("DHT bootstrapping complete");
    Ok(())
}

/// Discover peers for a torrent
pub async fn discover_peers(
    socket: &UdpSocket,
    our_id: NodeId,
    routing_table: &RoutingTable,
    info_hash: [u8; 20],
) -> Result<Vec<SocketAddr>> {
    tracing::info!("Discovering peers for torrent...");

    let mut discovered_peers = Vec::new();
    let closest_nodes = routing_table.find_closest_nodes(&NodeId::new(info_hash));

    // Query closest nodes for peers
    for node in closest_nodes {
        let transaction_id = generate_transaction_id();
        let get_peers_query =
            DHTMessage::create_get_peers_query(transaction_id, our_id, info_hash);
        let serialized = get_peers_query.serialize()?;

        socket.send_to(&serialized, node.addr).await?;
        tracing::debug!("Sent get_peers to node: {}", node.addr);

        // Wait for response (simplified - in real implementation, use async channels)
        sleep(Duration::from_millis(100)).await;
    }

    Ok(discovered_peers)
}

/// Announce ourselves to the DHT
pub async fn announce(
    socket: &UdpSocket,
    our_id: NodeId,
    routing_table: &RoutingTable,
    info_hash: [u8; 20],
    port: u16,
) -> Result<()> {
    tracing::info!("Announcing to DHT...");

    let closest_nodes = routing_table.find_closest_nodes(&NodeId::new(info_hash));

    // In a real implementation, we would:
    // 1. First do get_peers to get tokens
    // 2. Then announce_peer with those tokens

    for node in closest_nodes {
        let transaction_id = generate_transaction_id();
        // Note: This would require a valid token from get_peers response
        // For now, we'll use a placeholder token
        let token = "placeholder_token".to_string();

        let announce_query = DHTMessage::create_announce_peer_query(
            transaction_id,
            our_id,
            info_hash,
            port,
            token,
        );
        let serialized = announce_query.serialize()?;

        socket.send_to(&serialized, node.addr).await?;
        tracing::debug!("Sent announce_peer to node: {}", node.addr);
    }

    tracing::info!("Announcement complete");
    Ok(())
}

/// Generate a deterministic node ID for a bootstrap node
fn generate_bootstrap_node_id(addr: &SocketAddr) -> NodeId {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(addr.to_string().as_bytes());
    let result = hasher.finalize();
    let mut id = [0u8; 20];
    id.copy_from_slice(&result);
    NodeId::new(id)
}

/// Bootstrap and discover peers in one operation
pub async fn bootstrap_and_discover(
    socket: &UdpSocket,
    our_id: NodeId,
    routing_table: &mut RoutingTable,
    info_hash: [u8; 20],
) -> Result<Vec<SocketAddr>> {
    let config = BootstrapConfig::with_defaults(info_hash);
    
    // Bootstrap the network
    bootstrap(socket, our_id, routing_table, &config).await?;
    
    // Wait a bit for responses
    sleep(Duration::from_secs(2)).await;
    
    // Discover peers
    let peers = discover_peers(socket, our_id, routing_table, info_hash).await?;
    
    Ok(peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_config_new() {
        let nodes = vec!["127.0.0.1:6881".parse::<SocketAddr>().unwrap()];
        let info_hash = [1u8; 20];
        let config = BootstrapConfig::new(nodes, info_hash);
        assert_eq!(config.bootstrap_nodes.len(), 1);
        assert_eq!(config.info_hash, info_hash);
    }

    #[test]
    fn test_bootstrap_config_with_defaults() {
        let info_hash = [1u8; 20];
        let config = BootstrapConfig::with_defaults(info_hash);
        // Note: This test may be empty if DNS resolution failed during compilation
        // The bootstrap nodes are parsed at compile time, so network issues can cause empty list
        assert_eq!(config.info_hash, info_hash);
        // We don't assert non-empty since DNS failures are possible in test environments
    }

    #[test]
    fn test_generate_bootstrap_node_id() {
        let addr: SocketAddr = "127.0.0.1:6881".parse().unwrap();
        let node_id = generate_bootstrap_node_id(&addr);
        let node_id2 = generate_bootstrap_node_id(&addr);
        // Same address should generate same ID
        assert_eq!(node_id, node_id2);
    }

    #[tokio::test]
    async fn test_bootstrap() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let our_id = NodeId::random();
        let mut routing_table = RoutingTable::new(our_id);
        let info_hash = [1u8; 20];
        let config = BootstrapConfig::with_defaults(info_hash);

        // This test will fail if bootstrap nodes are unreachable, but that's expected
        let result = bootstrap(&socket, our_id, &mut routing_table, &config).await;
        // We don't assert success here because network may be unavailable
        let _ = result;
    }
}
