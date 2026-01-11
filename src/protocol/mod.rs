//! BitTorrent protocol module
//!
//! Implements the BitTorrent peer-to-peer protocol.

pub mod handshake;
pub mod message;
pub mod wire;

// Re-export main types
pub use handshake::{Handshake, PROTOCOL_STRING, PROTOCOL_LENGTH};
pub use message::{Message, MessageId};
pub use wire::{BitTorrentWire, WireProtocol, read_message, write_message};
