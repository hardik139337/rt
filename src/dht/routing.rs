//! DHT routing table module
//!
//! Implements the Kademlia routing table for DHT.

use crate::dht::node::{Node, NodeId};
use std::time::Instant;

const K: usize = 8; // Kademlia constant - number of nodes per bucket

/// A bucket in the routing table
#[derive(Debug, Clone)]
pub struct KBucket {
    /// Nodes in this bucket
    pub nodes: Vec<Node>,
    /// When this bucket was last modified
    pub last_changed: Instant,
    /// Bucket prefix (shared prefix of all nodes in this bucket)
    pub prefix: NodeId,
}

impl KBucket {
    /// Create a new KBucket
    pub fn new(prefix: NodeId) -> Self {
        Self {
            nodes: Vec::with_capacity(K),
            last_changed: Instant::now(),
            prefix,
        }
    }

    /// Add a node to the bucket
    pub fn add_node(&mut self, node: Node) -> bool {
        // Check if node already exists
        if let Some(pos) = self.nodes.iter().position(|n| n.id == node.id) {
            self.nodes[pos].update_last_seen();
            self.last_changed = Instant::now();
            return true;
        }

        // If bucket is full, return false
        if self.nodes.len() >= K {
            return false;
        }

        self.nodes.push(node);
        self.last_changed = Instant::now();
        true
    }

    /// Remove a node from the bucket
    pub fn remove_node(&mut self, id: &NodeId) {
        self.nodes.retain(|n| n.id != *id);
        self.last_changed = Instant::now();
    }

    /// Find a node by ID
    pub fn find_node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == *id)
    }

    /// Get the number of nodes in the bucket
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the bucket is full
    pub fn is_full(&self) -> bool {
        self.nodes.len() >= K
    }
}

/// Kademlia routing table
#[derive(Debug)]
pub struct RoutingTable {
    /// Our node ID
    pub our_id: NodeId,
    /// K buckets (160 buckets for 160-bit IDs)
    pub buckets: Vec<KBucket>,
}

impl RoutingTable {
    /// Create a new routing table
    pub fn new(our_id: NodeId) -> Self {
        let mut buckets = Vec::with_capacity(160);
        for i in 0..160 {
            let prefix = Self::calculate_prefix(&our_id, i);
            buckets.push(KBucket::new(prefix));
        }

        Self {
            our_id,
            buckets,
        }
    }

    /// Add a node to the routing table
    pub fn add_node(&mut self, node: Node) -> bool {
        let bucket_index = self.get_bucket_index(&node.id);
        self.buckets[bucket_index].add_node(node)
    }

    /// Find K closest nodes to a target ID
    pub fn find_closest_nodes(&self, target: &NodeId) -> Vec<Node> {
        let mut all_nodes: Vec<Node> = self.buckets
            .iter()
            .flat_map(|b| b.nodes.clone())
            .collect();

        // Sort by XOR distance to target
        all_nodes.sort_by_key(|n| {
            let mut distance = [0u8; 20];
            for i in 0..20 {
                distance[i] = n.id.0[i] ^ target.0[i];
            }
            distance
        });

        all_nodes.into_iter().take(K).collect()
    }

    /// Remove a node from the routing table
    pub fn remove_node(&mut self, id: &NodeId) {
        let bucket_index = self.get_bucket_index(id);
        self.buckets[bucket_index].remove_node(id);
    }

    /// Get all nodes in the routing table
    pub fn get_nodes(&self) -> Vec<Node> {
        self.buckets
            .iter()
            .flat_map(|b| b.nodes.clone())
            .collect()
    }

    /// Find a node by ID
    pub fn find_node(&self, id: &NodeId) -> Option<&Node> {
        let bucket_index = self.get_bucket_index(id);
        self.buckets[bucket_index].find_node(id)
    }

    /// Get the number of nodes in the routing table
    pub fn node_count(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// Get the bucket index for a node ID
    fn get_bucket_index(&self, id: &NodeId) -> usize {
        // Find the first bit where the IDs differ
        for i in 0..160 {
            let byte_index = i / 8;
            let bit_index = 7 - (i % 8);
            let our_bit = (self.our_id.0[byte_index] >> bit_index) & 1;
            let their_bit = (id.0[byte_index] >> bit_index) & 1;
            if our_bit != their_bit {
                return i;
            }
        }
        159 // IDs are identical
    }

    /// Calculate the prefix for a bucket index
    fn calculate_prefix(our_id: &NodeId, bucket_index: usize) -> NodeId {
        let mut prefix = our_id.0;
        if bucket_index < 160 {
            let byte_index = bucket_index / 8;
            let bit_index = 7 - (bucket_index % 8);
            // Flip the bit at this position
            prefix[byte_index] ^= 1 << bit_index;
        }
        NodeId(prefix)
    }

    /// Get all buckets that need refreshing
    pub fn get_stale_buckets(&self, timeout: std::time::Duration) -> Vec<usize> {
        self.buckets
            .iter()
            .enumerate()
            .filter(|(_, b)| b.last_changed.elapsed() > timeout)
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_table_new() {
        let our_id = NodeId::new([1u8; 20]);
        let table = RoutingTable::new(our_id);
        assert_eq!(table.our_id, our_id);
        assert_eq!(table.buckets.len(), 160);
    }

    #[test]
    fn test_add_node() {
        let our_id = NodeId::new([1u8; 20]);
        let mut table = RoutingTable::new(our_id);
        let node_id = NodeId::new([2u8; 20]);
        let node = Node::new(node_id, "127.0.0.1:6881".parse().unwrap());
        
        assert!(table.add_node(node));
        assert_eq!(table.node_count(), 1);
    }

    #[test]
    fn test_find_closest_nodes() {
        let our_id = NodeId::new([0u8; 20]);
        let mut table = RoutingTable::new(our_id);
        
        let target = NodeId::new([0xFFu8; 20]);
        
        // Add some nodes
        for i in 0..10 {
            let mut id = [0u8; 20];
            id[0] = i as u8;
            let node = Node::new(NodeId::new(id), format!("127.0.0.1:{}", 6881 + i).parse().unwrap());
            table.add_node(node);
        }
        
        let closest = table.find_closest_nodes(&target);
        assert!(!closest.is_empty());
        assert!(closest.len() <= K);
    }

    #[test]
    fn test_remove_node() {
        let our_id = NodeId::new([1u8; 20]);
        let mut table = RoutingTable::new(our_id);
        let node_id = NodeId::new([2u8; 20]);
        let node = Node::new(node_id, "127.0.0.1:6881".parse().unwrap());
        
        table.add_node(node.clone());
        assert_eq!(table.node_count(), 1);
        
        table.remove_node(&node_id);
        assert_eq!(table.node_count(), 0);
    }

    #[test]
    fn test_get_nodes() {
        let our_id = NodeId::new([1u8; 20]);
        let mut table = RoutingTable::new(our_id);
        
        for i in 0..5 {
            let mut id = [0u8; 20];
            id[0] = i as u8;
            let node = Node::new(NodeId::new(id), format!("127.0.0.1:{}", 6881 + i).parse().unwrap());
            table.add_node(node);
        }
        
        let nodes = table.get_nodes();
        assert_eq!(nodes.len(), 5);
    }

    #[test]
    fn test_bucket_index() {
        let our_id = NodeId::new([0x80u8; 20]);
        let table = RoutingTable::new(our_id);

        // Same ID should go to last bucket
        assert_eq!(table.get_bucket_index(&our_id), 159);

        // First bit different should go to bucket 0
        let different_id = NodeId::new([0x00u8; 20]);
        assert_eq!(table.get_bucket_index(&different_id), 0);
    }

    #[test]
    fn test_kbucket_new() {
        let prefix = NodeId::new([1u8; 20]);
        let bucket = KBucket::new(prefix);
        assert_eq!(bucket.prefix, prefix);
        assert_eq!(bucket.len(), 0);
        assert!(!bucket.is_full());
    }

    #[test]
    fn test_kbucket_add_node() {
        let prefix = NodeId::new([1u8; 20]);
        let mut bucket = KBucket::new(prefix);

        let node1 = Node::new(NodeId::new([2u8; 20]), "127.0.0.1:6881".parse().unwrap());
        let node2 = Node::new(NodeId::new([3u8; 20]), "127.0.0.1:6882".parse().unwrap());

        assert!(bucket.add_node(node1));
        assert_eq!(bucket.len(), 1);

        assert!(bucket.add_node(node2));
        assert_eq!(bucket.len(), 2);
    }

    #[test]
    fn test_kbucket_add_duplicate_updates() {
        let prefix = NodeId::new([1u8; 20]);
        let mut bucket = KBucket::new(prefix);

        let node_id = NodeId::new([2u8; 20]);
        let node1 = Node::new(node_id, "127.0.0.1:6881".parse().unwrap());
        let mut node2 = Node::new(node_id, "127.0.0.1:6881".parse().unwrap());

        // Add first node
        assert!(bucket.add_node(node1));
        assert_eq!(bucket.len(), 1);

        // Simulate time passing
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Add same node again (should update, not add duplicate)
        assert!(bucket.add_node(node2.clone()));
        assert_eq!(bucket.len(), 1);

        // The node should have been updated (last_seen refreshed)
        let found = bucket.find_node(&node_id).unwrap();
        assert!(found.time_since_seen() < std::time::Duration::from_millis(20));
    }

    #[test]
    fn test_kbucket_full() {
        let prefix = NodeId::new([1u8; 20]);
        let mut bucket = KBucket::new(prefix);

        // Add K nodes
        for i in 0..K {
            let mut id = [2u8; 20];
            id[19] = i as u8;
            let node = Node::new(NodeId::new(id), format!("127.0.0.1:{}", 6881 + i).parse().unwrap());
            assert!(bucket.add_node(node));
        }

        assert!(bucket.is_full());
        assert_eq!(bucket.len(), K);

        // Try to add one more - should fail
        let mut id = [2u8; 20];
        id[19] = K as u8;
        let node = Node::new(NodeId::new(id), "127.0.0.1:7000".parse().unwrap());
        assert!(!bucket.add_node(node));
    }

    #[test]
    fn test_kbucket_remove_node() {
        let prefix = NodeId::new([1u8; 20]);
        let mut bucket = KBucket::new(prefix);

        let node_id = NodeId::new([2u8; 20]);
        let node = Node::new(node_id, "127.0.0.1:6881".parse().unwrap());
        bucket.add_node(node);

        assert_eq!(bucket.len(), 1);
        bucket.remove_node(&node_id);
        assert_eq!(bucket.len(), 0);
    }

    #[test]
    fn test_kbucket_find_node() {
        let prefix = NodeId::new([1u8; 20]);
        let mut bucket = KBucket::new(prefix);

        let node_id = NodeId::new([2u8; 20]);
        let node = Node::new(node_id, "127.0.0.1:6881".parse().unwrap());
        bucket.add_node(node.clone());

        assert!(bucket.find_node(&node_id).is_some());
        assert!(bucket.find_node(&NodeId::new([3u8; 20])).is_none());
    }

    #[test]
    fn test_find_node_in_routing_table() {
        let our_id = NodeId::new([1u8; 20]);
        let mut table = RoutingTable::new(our_id);

        let node_id = NodeId::new([2u8; 20]);
        let node = Node::new(node_id, "127.0.0.1:6881".parse().unwrap());
        table.add_node(node.clone());

        assert!(table.find_node(&node_id).is_some());
        assert!(table.find_node(&NodeId::new([3u8; 20])).is_none());
    }

    #[test]
    fn test_get_stale_buckets() {
        let our_id = NodeId::new([1u8; 20]);
        let table = RoutingTable::new(our_id);

        // With a long timeout, no buckets should be stale
        let stale = table.get_stale_buckets(std::time::Duration::from_secs(1000));
        assert_eq!(stale.len(), 0);

        // With zero timeout, all buckets should be stale (they were just created)
        let stale = table.get_stale_buckets(std::time::Duration::from_secs(0));
        assert_eq!(stale.len(), 160);
    }

    #[test]
    fn test_find_closest_nodes_sorted() {
        let our_id = NodeId::new([0u8; 20]);
        let mut table = RoutingTable::new(our_id);

        let target = NodeId::new([0xFFu8; 20]);

        // Add nodes at various distances
        let node1 = Node::new(NodeId::new([0xF0u8; 20]), "127.0.0.1:6881".parse().unwrap());
        let node2 = Node::new(NodeId::new([0x0Fu8; 20]), "127.0.0.1:6882".parse().unwrap());
        let node3 = Node::new(NodeId::new([0xFFu8; 20]), "127.0.0.1:6883".parse().unwrap());

        table.add_node(node1);
        table.add_node(node2);
        table.add_node(node3);

        let closest = table.find_closest_nodes(&target);

        // Node 3 should be first (identical to target, distance 0)
        // Node 1 should be second (closer to target)
        // Node 2 should be last (furthest from target)
        assert_eq!(closest.len(), 3);
        assert_eq!(closest[0].id, NodeId::new([0xFFu8; 20]));
    }
}
