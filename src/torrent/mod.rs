//! Torrent file handling module
//!
//! This module provides functionality for parsing and working with .torrent files and magnet links.

pub mod parser;
pub mod info;
pub mod magnet;

pub use parser::TorrentParser;
pub use info::{TorrentInfo, TorrentFile};
pub use magnet::{MagnetParser, MagnetInfo};
