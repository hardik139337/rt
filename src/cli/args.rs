//! CLI arguments module
//!
//! Defines command-line argument parsing using clap.

use clap::Parser;
use std::path::PathBuf;

/// CLI arguments for the torrent downloader
#[derive(Debug, Parser)]
#[command(name = "rust-torrent-downloader")]
#[command(about = "A full-featured BitTorrent CLI downloader", long_about = None)]
pub struct CliArgs {
    /// Path to the .torrent file
    #[arg(value_name = "TORRENT_FILE")]
    pub torrent_file: PathBuf,

    /// Download directory
    #[arg(short, long, value_name = "DIR")]
    pub output_dir: Option<PathBuf>,

    /// Listening port for incoming connections
    #[arg(short, long, default_value_t = 6881)]
    pub port: u16,

    /// Maximum number of peer connections
    #[arg(short, long, default_value_t = 50)]
    pub max_connections: usize,

    /// Seed after download completes
    #[arg(long, default_value_t = true)]
    pub seed: bool,

    /// Seed ratio to stop at (e.g., 1.0 = 100%)
    #[arg(long, default_value_t = 1.0)]
    pub seed_ratio: f64,

    /// Seed time in minutes (0 = no time limit)
    #[arg(long, default_value_t = 0)]
    pub seed_time: u64,

    /// Enable DHT peer discovery
    #[arg(long, default_value_t = true)]
    pub use_dht: bool,

    /// Enable tracker communication
    #[arg(long, default_value_t = true)]
    pub use_tracker: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Quiet mode (no output except errors)
    #[arg(short, long)]
    pub quiet: bool,

    /// Resume from checkpoint
    #[arg(long)]
    pub resume: bool,
}

impl CliArgs {
    /// Parse CLI arguments from command line
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Check if verbose mode is enabled
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Check if quiet mode is enabled
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Get the log level based on verbosity settings
    pub fn log_level(&self) -> tracing::Level {
        if self.verbose {
            tracing::Level::DEBUG
        } else if self.quiet {
            tracing::Level::ERROR
        } else {
            tracing::Level::INFO
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let args = CliArgs {
            torrent_file: PathBuf::from("test.torrent"),
            output_dir: None,
            port: 6881,
            max_connections: 50,
            seed: true,
            seed_ratio: 1.0,
            seed_time: 0,
            use_dht: true,
            use_tracker: true,
            verbose: false,
            quiet: false,
            resume: false,
        };

        assert_eq!(args.port, 6881);
        assert_eq!(args.max_connections, 50);
        assert!(args.seed);
        assert_eq!(args.seed_ratio, 1.0);
        assert_eq!(args.seed_time, 0);
        assert!(args.use_dht);
        assert!(args.use_tracker);
    }
}
