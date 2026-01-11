//! CLI module
//!
//! Command-line interface for the torrent downloader.

pub mod args;
pub mod config;
pub mod progress;

pub use args::CliArgs;
pub use config::Config;
pub use progress::{ProgressDisplay, DownloadStats};
