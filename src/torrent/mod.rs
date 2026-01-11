//! Torrent file handling module
//!
//! This module provides functionality for parsing and working with .torrent files.

pub mod parser;
pub mod info;

pub use parser::TorrentParser;
pub use info::{TorrentInfo, TorrentFile};
