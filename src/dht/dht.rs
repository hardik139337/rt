//! DHT main module
//!
//! Main DHT implementation for peer discovery.

use crate::dht::bootstrap::{bootstrap, discover_peers, announce, BootstrapConfig};
use crate::dht::message::{
    parse_compact_nodes, parse_compact_peers, DHTMessage, QueryType,
    Transaction,
};
use crate::dht::node::{Node, NodeId};
use crate::dht::routing::RoutingTable;
use crate::peer::PeerManager;
use crate::error::TorrentError;
use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::{interval};
use tracing::{debug, error, info, trace, warn};

/// Main DHT struct
pub struct DHT {
    /// Routing table
    pub routing_table: Arc<RwLock<RoutingTable>>,
    /// UDP socket for DHT communication
    pub socket: UdpSocket,
    /// Our node ID
    pub our_id: NodeId,
    /// Transaction tracking
    pub transactions: Arc<RwLock<HashMap<String, Transaction>>>,
    /// Peer manager for discovered peers
    pub peer_manager: Arc<PeerManager>,
    /// Local address
    pub local_addr: SocketAddr,
    /// Running state
    pub running: Arc<RwLock<bool>>,
}

impl DHT {
    /// Create a new DHT instance
    pub async fn new(
        bind_addr: SocketAddr,
        peer_manager: Arc<PeerManager>,
    ) -> Result<Self> {
        info!("Creating DHT instance on {}", bind_addr);
        
        let socket = UdpSocket::bind(bind_addr).await
            .map_err(|e| {
                error!("Failed to bind UDP socket to {}: {}", bind_addr, e);
                TorrentError::network_error_full("Failed to bind UDP socket", bind_addr.to_string(), e.to_string())
            })?;
        let local_addr = socket.local_addr()
            .map_err(|e| {
                error!("Failed to get local address: {}", e);
                TorrentError::network_error_full("Failed to get local address", "unknown".to_string(), e.to_string())
            })?;
        let our_id = NodeId::random();
        let routing_table = Arc::new(RwLock::new(RoutingTable::new(our_id)));
        let transactions = Arc::new(RwLock::new(HashMap::new()));
        let running = Arc::new(RwLock::new(false));

        info!("DHT initialized with ID: {}", our_id.to_hex());
        info!("DHT listening on: {}", local_addr);

        Ok(Self {
            routing_table,
            socket,
            our_id,
            transactions,
            peer_manager,
            local_addr,
            running,
        })
    }

    /// Start DHT service
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            warn!("DHT is already running");
            return Ok(());
        }
        *running = true;
        drop(running);

        info!("Starting DHT service...");

        // Bootstrap DHT
        let info_hash = [0u8; 20]; // Placeholder - will be set when downloading
        let config = BootstrapConfig::with_defaults(info_hash);
        let mut routing_table = self.routing_table.write().await;
        bootstrap(&self.socket, self.our_id, &mut routing_table, &config).await
            .map_err(|e| {
                error!("Failed to bootstrap DHT: {}", e);
                TorrentError::dht_error_full("Failed to bootstrap DHT", "unknown".to_string(), e.to_string())
            })?;
        drop(routing_table);

        info!("DHT service started");
        Ok(())
    }

    /// Send a query to a node
    pub async fn send_query(&self, node: &Node, message: DHTMessage) -> Result<()> {
        let transaction_id = message.get_transaction_id().unwrap_or_default();
        let serialized = message.serialize()
            .map_err(|e| {
                error!("Failed to serialize DHT message: {}", e);
                TorrentError::dht_error_full("Failed to serialize DHT message", "unknown".to_string(), e.to_string())
            })?;

        // Track transaction
        if let DHTMessage::Query { id, query_type, .. } = &message {
            let transaction = Transaction::new(transaction_id.clone(), *id, query_type.clone());
            self.transactions.write().await.insert(transaction_id, transaction);
        }

        self.socket.send_to(&serialized, node.addr).await
            .map_err(|e| {
                error!("Failed to send query to {}: {}", node.addr, e);
                TorrentError::network_error_full("Failed to send query", node.addr.to_string(), e.to_string())
            })?;
        debug!("Sent query to {}: {:?}", node.addr, message.message_type());

        Ok(())
    }

    /// Handle incoming DHT message
    pub async fn handle_message(&self, data: &[u8], from: SocketAddr) -> Result<()> {
        trace!("Handling message from {} ({} bytes)", from, data.len());
        let message = DHTMessage::deserialize(data)
            .map_err(|e| {
                error!("Failed to deserialize DHT message from {}: {}", from, e);
                TorrentError::dht_error_full("Failed to deserialize DHT message", from.to_string(), e.to_string())
            })?;

        match message {
            DHTMessage::Query { id, query_type, args } => {
                self.handle_query(id, query_type, args, from).await?;
            }
            DHTMessage::Response { id, response_type, args } => {
                self.handle_response(id, response_type, args, from).await?;
            }
            DHTMessage::Error { id, code, message: msg } => {
                self.handle_error(id, code, msg, from).await?;
            }
        }

        Ok(())
    }

    /// Handle incoming query
    async fn handle_query(
        &self,
        id: NodeId,
        query_type: QueryType,
        args: HashMap<String, crate::dht::message::BencodeValue>,
        from: SocketAddr,
    ) -> Result<()> {
        debug!("Received {} query from {}", query_type, from);

        // Add querying node to our routing table
        let node = Node::new(id, from);
        self.routing_table.write().await.add_node(node);

        // Handle different query types
        match query_type {
            QueryType::Ping => {
                // Send ping response
                let response = DHTMessage::Response {
                    id: self.our_id,
                    response_type: crate::dht::message::ResponseType::Ping,
                    args: args,
                };
                let serialized = response.serialize()
                    .map_err(|e| {
                        error!("Failed to serialize ping response: {}", e);
                        TorrentError::dht_error_full("Failed to serialize ping response", "unknown".to_string(), e.to_string())
                    })?;
                self.socket.send_to(&serialized, from).await
                    .map_err(|e| {
                        error!("Failed to send ping response to {}: {}", from, e);
                        TorrentError::network_error_full("Failed to send ping response", from.to_string(), e.to_string())
                    })?;
            }
            QueryType::FindNode => {
                // Find closest nodes to target
                if let Some(target_hex) = args.get("target") {
                    if let crate::dht::message::BencodeValue::String(target_str) = target_hex {
                        if let Some(target_id) = NodeId::from_hex(target_str) {
                            let closest = self.routing_table.read().await.find_closest_nodes(&target_id);
                            let mut response_args = HashMap::new();
                            response_args.insert(
                                "id".to_string(),
                                crate::dht::message::BencodeValue::String(self.our_id.to_hex()),
                            );
                            // In a real implementation, serialize nodes to compact format
                            response_args.insert(
                                "nodes".to_string(),
                                crate::dht::message::BencodeValue::String("".to_string()),
                            );

                            let response = DHTMessage::Response {
                                id: self.our_id,
                                response_type: crate::dht::message::ResponseType::FindNode,
                                args: response_args,
                            };
                            let serialized = response.serialize()
                                .map_err(|e| {
                                    error!("Failed to serialize find_node response: {}", e);
                                    TorrentError::dht_error_full("Failed to serialize find_node response", "unknown".to_string(), e.to_string())
                                })?;
                            self.socket.send_to(&serialized, from).await
                                .map_err(|e| {
                                    error!("Failed to send find_node response to {}: {}", from, e);
                                    TorrentError::network_error_full("Failed to send find_node response", from.to_string(), e.to_string())
                                })?;
                        }
                    }
                }
            }
            QueryType::GetPeers => {
                // Return peers if we have them, otherwise return closest nodes
                let mut response_args = HashMap::new();
                response_args.insert(
                    "id".to_string(),
                    crate::dht::message::BencodeValue::String(self.our_id.to_hex()),
                );
                response_args.insert(
                    "token".to_string(),
                    crate::dht::message::BencodeValue::String("token".to_string()),
                );

                let response = DHTMessage::Response {
                    id: self.our_id,
                    response_type: crate::dht::message::ResponseType::GetPeers,
                    args: response_args,
                };
                let serialized = response.serialize()
                    .map_err(|e| {
                        error!("Failed to serialize get_peers response: {}", e);
                        TorrentError::dht_error_full("Failed to serialize get_peers response", "unknown".to_string(), e.to_string())
                    })?;
                self.socket.send_to(&serialized, from).await
                    .map_err(|e| {
                        error!("Failed to send get_peers response to {}: {}", from, e);
                        TorrentError::network_error_full("Failed to send get_peers response", from.to_string(), e.to_string())
                    })?;
            }
            QueryType::AnnouncePeer => {
                // Acknowledge announce
                let response = DHTMessage::Response {
                    id: self.our_id,
                    response_type: crate::dht::message::ResponseType::AnnouncePeer,
                    args: args,
                };
                let serialized = response.serialize()
                    .map_err(|e| {
                        error!("Failed to serialize announce_peer response: {}", e);
                        TorrentError::dht_error_full("Failed to serialize announce_peer response", "unknown".to_string(), e.to_string())
                    })?;
                self.socket.send_to(&serialized, from).await
                    .map_err(|e| {
                        error!("Failed to send announce_peer response to {}: {}", from, e);
                        TorrentError::network_error_full("Failed to send announce_peer response", from.to_string(), e.to_string())
                    })?;
            }
        }

        Ok(())
    }

    /// Handle incoming response
    async fn handle_response(
        &self,
        id: NodeId,
        response_type: crate::dht::message::ResponseType,
        args: HashMap<String, crate::dht::message::BencodeValue>,
        from: SocketAddr,
    ) -> Result<()> {
        debug!("Received {:?} response from {}", response_type, from);

        // Update node in routing table
        let node = Node::new(id, from);
        self.routing_table.write().await.add_node(node);

        // Process response based on type
        match response_type {
            crate::dht::message::ResponseType::FindNode => {
                // Parse and add nodes from response
                if let Some(nodes_str) = args.get("nodes") {
                    if let crate::dht::message::BencodeValue::String(nodes_data) = nodes_str {
                        if let Ok(nodes_bytes) = hex::decode(nodes_data) {
                            if let Ok(parsed_nodes) = parse_compact_nodes(&nodes_bytes) {
                                let mut table = self.routing_table.write().await;
                                for (node_id, addr) in &parsed_nodes {
                                    table.add_node(Node::new(*node_id, *addr));
                                }
                                debug!("Added {} nodes from find_node response", parsed_nodes.len());
                            }
                        }
                    }
                }
            }
            crate::dht::message::ResponseType::GetPeers => {
                // Parse peers from response
                if let Some(values) = args.get("values") {
                    if let crate::dht::message::BencodeValue::String(peers_data) = values {
                        if let Ok(peers_bytes) = hex::decode(peers_data) {
                            if let Ok(parsed_peers) = parse_compact_peers(&peers_bytes) {
                                for peer_addr in &parsed_peers {
                                    if let Err(e) = self.peer_manager.add_peer(*peer_addr).await {
                                        warn!("Failed to add peer {}: {}", peer_addr, e);
                                    }
                                }
                                debug!("Added {} peers from get_peers response", parsed_peers.len());
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle incoming error
    async fn handle_error(
        &self,
        id: NodeId,
        code: u32,
        message: String,
        from: SocketAddr,
    ) -> Result<()> {
        warn!("Received error from {}: code={}, message={}", from, code, message);

        // Remove transaction
        let transaction_id = id.to_hex();
        self.transactions.write().await.remove(&transaction_id);

        Ok(())
    }

    /// Find peers for a given info hash
    pub async fn find_peers(&self, info_hash: [u8; 20]) -> Result<Vec<SocketAddr>> {
        info!("Finding peers for info_hash: {}", hex::encode(info_hash));
        
        let routing_table = self.routing_table.read().await;
        let peers = discover_peers(
            &self.socket,
            self.our_id,
            &routing_table,
            info_hash,
        ).await
            .map_err(|e| {
                error!("Failed to discover peers: {}", e);
                TorrentError::dht_error_full("Failed to discover peers", "unknown".to_string(), e.to_string())
            })?;

        info!("Found {} peers", peers.len());
        Ok(peers)
    }

    /// Announce ourselves to DHT
    pub async fn announce_peer(&self, info_hash: [u8; 20], port: u16) -> Result<()> {
        info!("Announcing to DHT for info_hash: {}", hex::encode(info_hash));
        
        let routing_table = self.routing_table.read().await;
        announce(
            &self.socket,
            self.our_id,
            &routing_table,
            info_hash,
            port,
        ).await
            .map_err(|e| {
                error!("Failed to announce to DHT: {}", e);
                TorrentError::dht_error_full("Failed to announce to DHT", "unknown".to_string(), e.to_string())
            })?;

        Ok(())
    }

    /// Main DHT event loop
    pub async fn run_loop(&self) -> Result<()> {
        info!("Starting DHT event loop");

        let mut buffer = [0u8; 4096];
        let mut cleanup_interval = interval(Duration::from_secs(60));
        let mut refresh_interval = interval(Duration::from_secs(300));

        loop {
            let running = *self.running.read().await;
            if !running {
                info!("DHT event loop stopped");
                break;
            }

            tokio::select! {
                // Handle incoming messages
                result = self.socket.recv_from(&mut buffer) => {
                    match result {
                        Ok((len, from)) => {
                            if let Err(e) = self.handle_message(&buffer[..len], from).await {
                                error!("Error handling message from {}: {}", from, e);
                            }
                        }
                        Err(e) => {
                            error!("Error receiving message: {}", e);
                        }
                    }
                }
                // Cleanup expired transactions
                _ = cleanup_interval.tick() => {
                    self.cleanup_transactions().await;
                }
                // Refresh routing table buckets
                _ = refresh_interval.tick() => {
                    self.refresh_buckets().await;
                }
            }
        }

        Ok(())
    }

    /// Clean up expired transactions
    pub async fn cleanup_transactions(&self) {
        let timeout = Duration::from_secs(60);
        let mut transactions = self.transactions.write().await;
        let initial_count = transactions.len();

        transactions.retain(|_t, transaction| !transaction.is_expired(timeout));

        let removed = initial_count - transactions.len();
        if removed > 0 {
            debug!("Cleaned up {} expired transactions", removed);
        }
    }

    /// Refresh routing table buckets
    pub async fn refresh_buckets(&self) {
        debug!("Refreshing routing table buckets");

        let stale_buckets = self.routing_table.read().await.get_stale_buckets(Duration::from_secs(900));

        if stale_buckets.is_empty() {
            return;
        }

        debug!("Found {} stale buckets to refresh", stale_buckets.len());

        // In a real implementation, we would refresh each bucket by querying nodes
        // For now, we just log stale buckets
        for bucket_index in stale_buckets {
            debug!("Bucket {} is stale", bucket_index);
        }
    }

    /// Stop DHT service
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("DHT service stopped");
    }

    /// Get the number of nodes in routing table
    pub async fn node_count(&self) -> usize {
        self.routing_table.read().await.node_count()
    }

    /// Get all nodes in routing table
    pub async fn get_all_nodes(&self) -> Vec<Node> {
        self.routing_table.read().await.get_nodes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dht_new() {
        let peer_manager = Arc::new(PeerManager::default());
        let bind_addr: SocketAddr = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
        
        let dht = DHT::new(bind_addr, peer_manager).await;
        assert!(dht.is_ok());
        
        let dht = dht.unwrap();
        assert_eq!(dht.our_id.0.len(), 20);
    }

    #[tokio::test]
    async fn test_dht_start_stop() {
        let peer_manager = Arc::new(PeerManager::default());
        let bind_addr: SocketAddr = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
        
        let dht = DHT::new(bind_addr, peer_manager).await.unwrap();
        dht.start().await.unwrap();
        dht.stop().await;
    }

    #[tokio::test]
    async fn test_cleanup_transactions() {
        let peer_manager = Arc::new(PeerManager::default());
        let bind_addr: SocketAddr = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
        
        let dht = DHT::new(bind_addr, peer_manager).await.unwrap();
        
        // Add a transaction
        let transaction = Transaction::new(
            "test".to_string(),
            NodeId::random(),
            QueryType::Ping,
        );
        dht.transactions.write().await.insert("test".to_string(), transaction);
        
        dht.cleanup_transactions().await;
        
        // Transaction should still be there (not expired)
        assert_eq!(dht.transactions.read().await.len(), 1);
    }

    #[tokio::test]
    async fn test_node_count() {
        let peer_manager = Arc::new(PeerManager::default());
        let bind_addr: SocketAddr = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
        
        let dht = DHT::new(bind_addr, peer_manager).await.unwrap();
        let count = dht.node_count().await;
        assert_eq!(count, 0);
    }
}
