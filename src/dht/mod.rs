//! DHT (Distributed Hash Table) module
//!
//! Implements the BitTorrent DHT for peer discovery.

pub mod node;
pub mod routing;
pub mod message;
pub mod bootstrap;
pub mod dht;

// Re-exports for convenience
pub use node::{Node, NodeId};
pub use routing::{KBucket, RoutingTable};
pub use message::{
    DHTMessage, QueryType, ResponseType, Transaction, BencodeDict, BencodeValue,
    generate_transaction_id, parse_compact_nodes, parse_compact_peers,
    serialize_compact_nodes, serialize_compact_peers,
};
pub use bootstrap::{BootstrapConfig, bootstrap, discover_peers, announce, bootstrap_and_discover};
pub use dht::DHT;
