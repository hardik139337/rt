//! rust-torrent-downloader
//!
//! A full-featured BitTorrent CLI downloader with DHT, seeding, and resume support.

pub mod torrent;
pub mod protocol;
pub mod peer;
pub mod dht;
pub mod storage;
pub mod cli;
pub mod error;

pub use error::TorrentError;

pub use torrent::{TorrentParser, TorrentInfo};
pub use protocol::{Handshake, Message, MessageId};
pub use peer::{PeerConnection, PeerManager, PeerInfo, PeerState};
pub use dht::{
    Node, NodeId, KBucket, RoutingTable, DHT, DHTMessage,
    QueryType, ResponseType, Transaction, BencodeDict, BencodeValue,
    BootstrapConfig, bootstrap, discover_peers, announce, bootstrap_and_discover,
    generate_transaction_id, parse_compact_nodes, parse_compact_peers,
    serialize_compact_nodes, serialize_compact_peers,
};
pub use storage::{
    PieceStorage, PieceStatus, FileStorage, ResumeData, ResumeManager,
    Piece, Block, DownloadManager, PieceDownload, DownloadStats as StorageDownloadStats
};
pub use cli::{CliArgs, Config, ProgressDisplay, DownloadStats};
