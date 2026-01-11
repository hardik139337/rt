//! Peer management module
//!
//! Handles peer connections and state management.

pub mod connection;
pub mod manager;
pub mod state;

// Re-export main types
pub use connection::PeerConnection;
pub use manager::PeerManager;
pub use state::{Peer, PeerState, PeerInfo, PeerSource, PeerStats};
