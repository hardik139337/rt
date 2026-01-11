//! CLI configuration module
//!
//! Manages configuration for the CLI application.

use crate::cli::args::CliArgs;
use crate::torrent::TorrentInfo;
use std::path::PathBuf;
use std::time::Duration;
use anyhow::Result;

/// Configuration for the torrent downloader
#[derive(Debug, Clone)]
pub struct Config {
    /// Torrent information
    pub torrent_info: TorrentInfo,
    /// Download directory
    pub output_dir: PathBuf,
    /// Listening port
    pub port: u16,
    /// Maximum number of peer connections
    pub max_connections: usize,
    /// Seed after download
    pub seed: bool,
    /// Seed ratio to stop at
    pub seed_ratio: f64,
    /// Seed time in minutes
    pub seed_time: Duration,
    /// Enable DHT
    pub use_dht: bool,
    /// Enable tracker
    pub use_tracker: bool,
    /// Verbose output
    pub verbose: bool,
    /// Quiet mode
    pub quiet: bool,
}

impl Config {
    /// Create configuration from CLI arguments
    pub fn from_args(args: &CliArgs, torrent_info: TorrentInfo) -> Self {
        let output_dir = args.output_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("./downloads"));

        Self {
            torrent_info,
            output_dir,
            port: args.port,
            max_connections: args.max_connections,
            seed: args.seed,
            seed_ratio: args.seed_ratio,
            seed_time: Duration::from_secs(args.seed_time * 60),
            use_dht: args.use_dht,
            use_tracker: args.use_tracker,
            verbose: args.verbose,
            quiet: args.quiet,
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate port range
        if self.port == 0 {
            return Err(anyhow::anyhow!("Port cannot be 0"));
        }

        // Validate max connections
        if self.max_connections == 0 {
            return Err(anyhow::anyhow!("max_connections must be at least 1"));
        }

        // Validate seed ratio
        if self.seed_ratio < 0.0 {
            return Err(anyhow::anyhow!("seed_ratio must be non-negative"));
        }

        // Validate seed time
        if self.seed && self.seed_time == Duration::ZERO && self.seed_ratio == 0.0 {
            // Both seed time and ratio are 0, but seeding is enabled
            // This is valid - seed indefinitely
        }

        // Validate output directory
        if self.output_dir.as_os_str().is_empty() {
            return Err(anyhow::anyhow!("output_dir cannot be empty"));
        }

        Ok(())
    }

    /// Get the listen address for incoming connections
    pub fn get_listen_addr(&self) -> String {
        format!("0.0.0.0:{}", self.port)
    }

    /// Check if DHT should be enabled
    pub fn is_dht_enabled(&self) -> bool {
        self.use_dht
    }

    /// Check if tracker should be enabled
    pub fn is_tracker_enabled(&self) -> bool {
        self.use_tracker
    }

    /// Check if seeding should be enabled
    pub fn is_seeding_enabled(&self) -> bool {
        self.seed
    }

    /// Get the seed time limit (None for unlimited)
    pub fn seed_time_limit(&self) -> Option<Duration> {
        if self.seed_time == Duration::ZERO {
            None
        } else {
            Some(self.seed_time)
        }
    }

    /// Get the seed ratio limit (None for unlimited)
    pub fn seed_ratio_limit(&self) -> Option<f64> {
        if self.seed_ratio == 0.0 {
            None
        } else {
            Some(self.seed_ratio)
        }
    }

    /// Check if verbose mode is enabled
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Check if quiet mode is enabled
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_args() {
        let args = CliArgs {
            torrent_file: PathBuf::from("test.torrent"),
            output_dir: Some(PathBuf::from("/tmp/downloads")),
            port: 6882,
            max_connections: 100,
            seed: true,
            seed_ratio: 2.0,
            seed_time: 60,
            use_dht: false,
            use_tracker: true,
            verbose: true,
            quiet: false,
            resume: false,
        };

        let torrent_info = TorrentInfo {
            announce: "http://tracker.example.com/announce".to_string(),
            announce_list: vec!["http://tracker.example.com/announce".to_string()],
            info_hash: [0u8; 20],
            piece_length: 262144,
            pieces: vec![[0u8; 20]],
            name: "test_torrent".to_string(),
            length: Some(1048576),
            files: None,
        };

        let config = Config::from_args(&args, torrent_info);

        assert_eq!(config.output_dir, PathBuf::from("/tmp/downloads"));
        assert_eq!(config.port, 6882);
        assert_eq!(config.max_connections, 100);
        assert!(config.seed);
        assert_eq!(config.seed_ratio, 2.0);
        assert_eq!(config.seed_time, Duration::from_secs(3600));
        assert!(!config.use_dht);
        assert!(config.use_tracker);
        assert!(config.verbose);
        assert!(!config.quiet);
    }

    #[test]
    fn test_config_validate() {
        let torrent_info = TorrentInfo {
            announce: "http://tracker.example.com/announce".to_string(),
            announce_list: vec!["http://tracker.example.com/announce".to_string()],
            info_hash: [0u8; 20],
            piece_length: 262144,
            pieces: vec![[0u8; 20]],
            name: "test_torrent".to_string(),
            length: Some(1048576),
            files: None,
        };

        let config = Config {
            torrent_info,
            output_dir: PathBuf::from("./downloads"),
            port: 6881,
            max_connections: 50,
            seed: true,
            seed_ratio: 1.0,
            seed_time: Duration::ZERO,
            use_dht: true,
            use_tracker: true,
            verbose: false,
            quiet: false,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_invalid_port() {
        let torrent_info = TorrentInfo {
            announce: "http://tracker.example.com/announce".to_string(),
            announce_list: vec!["http://tracker.example.com/announce".to_string()],
            info_hash: [0u8; 20],
            piece_length: 262144,
            pieces: vec![[0u8; 20]],
            name: "test_torrent".to_string(),
            length: Some(1048576),
            files: None,
        };

        let config = Config {
            torrent_info,
            output_dir: PathBuf::from("./downloads"),
            port: 0,
            max_connections: 50,
            seed: true,
            seed_ratio: 1.0,
            seed_time: Duration::ZERO,
            use_dht: true,
            use_tracker: true,
            verbose: false,
            quiet: false,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_get_listen_addr() {
        let torrent_info = TorrentInfo {
            announce: "http://tracker.example.com/announce".to_string(),
            announce_list: vec!["http://tracker.example.com/announce".to_string()],
            info_hash: [0u8; 20],
            piece_length: 262144,
            pieces: vec![[0u8; 20]],
            name: "test_torrent".to_string(),
            length: Some(1048576),
            files: None,
        };

        let config = Config {
            torrent_info,
            output_dir: PathBuf::from("./downloads"),
            port: 6881,
            max_connections: 50,
            seed: true,
            seed_ratio: 1.0,
            seed_time: Duration::ZERO,
            use_dht: true,
            use_tracker: true,
            verbose: false,
            quiet: false,
        };

        assert_eq!(config.get_listen_addr(), "0.0.0.0:6881");
    }
}
