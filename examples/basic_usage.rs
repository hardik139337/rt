//! Basic usage example for rust-torrent-downloader
//!
//! This example demonstrates how to use the library to:
//! - Parse a torrent file
//! - Parse a magnet link
//! - Get torrent/magnet information
//! - Create a download manager (for torrent files only)
//! - Initialize file storage (for torrent files only)
//!
//! Run this example with:
//! ```bash
//! # For torrent files:
//! cargo run --example basic_usage -- <path-to-torrent-file>
//!
//! # For magnet links:
//! cargo run --example basic_usage -- <magnet-link>
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use rust_torrent_downloader::torrent::{TorrentParser, MagnetParser};
use rust_torrent_downloader::storage::FileStorage;
use rust_torrent_downloader::protocol::Handshake;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Get torrent file path or magnet link from command line
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <torrent-file-or-magnet-link>", args[0]);
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  Torrent file: {} example.torrent", args[0]);
        eprintln!("  Magnet link: {} magnet:?xt=urn:btih:...", args[0]);
        eprintln!();
        eprintln!("Note: Magnet links require metadata exchange (DHT/PEX) to get file information.");
        eprintln!("This example demonstrates magnet link parsing. Full download support requires");
        eprintln!("additional DHT and peer protocol implementation.");
        std::process::exit(1);
    }

    let input = &args[1];

    // Check if input is a magnet link
    if MagnetParser::is_magnet_link(input) {
        parse_magnet_link(input).await?;
    } else {
        // Treat as a torrent file path
        let torrent_path = PathBuf::from(input);
        parse_torrent_file(&torrent_path).await?;
    }

    Ok(())
}

/// Parse and display magnet link information
async fn parse_magnet_link(magnet_uri: &str) -> anyhow::Result<()> {
    println!("Parsing magnet link...");
    println!();

    let magnet_info = MagnetParser::parse(magnet_uri)?;

    // Display magnet link information
    println!("=== Magnet Link Information ===");
    println!("Info Hash: {}", hex::encode(magnet_info.info_hash));
    println!("Display Name: {}", magnet_info.display_name.as_deref().unwrap_or("(none)"));

    if let Some(size) = magnet_info.total_size {
        println!("Total Size: {} bytes ({:.2} MB)", size, size as f64 / 1024.0 / 1024.0);
    } else {
        println!("Total Size: (unknown - metadata exchange required)");
    }

    // Display tracker information
    println!("\n=== Trackers ===");
    if magnet_info.trackers.is_empty() {
        println!("No trackers specified (DHT will be used)");
    } else {
        println!("Total Trackers: {}", magnet_info.trackers.len());
        for (i, tracker) in magnet_info.trackers.iter().enumerate() {
            println!("  {}. {}", i + 1, tracker);
        }
    }

    // Display web seed information
    if !magnet_info.web_seeds.is_empty() {
        println!("\n=== Web Seeds ===");
        println!("Total Web Seeds: {}", magnet_info.web_seeds.len());
        for (i, seed) in magnet_info.web_seeds.iter().enumerate() {
            println!("  {}. {}", i + 1, seed);
        }
    }

    // Display exact source information
    if !magnet_info.exact_sources.is_empty() {
        println!("\n=== Exact Sources ===");
        println!("These URLs can be used to download the .torrent file:");
        for (i, source) in magnet_info.exact_sources.iter().enumerate() {
            println!("  {}. {}", i + 1, source);
        }
    }

    // Generate a peer ID for our client
    let peer_id = Handshake::generate_peer_id();
    println!("\n=== Peer Information ===");
    println!("Our Peer ID: {}", hex::encode(peer_id));

    println!("\n=== Magnet Link Notes ===");
    println!("Magnet links contain only the info hash and tracker URLs.");
    println!("To download files from a magnet link, you need to:");
    println!("  1. Connect to the DHT network to find peers");
    println!("  2. Exchange metadata with peers (using extension protocol)");
    println!("  3. Download the actual torrent metadata (piece hashes, file list)");
    println!("  4. Then proceed with normal torrent download");
    println!();
    println!("For torrent files with complete metadata, see the torrent file example.");

    println!("\n=== Basic Usage Example Complete ===");

    Ok(())
}

/// Parse and display torrent file information
async fn parse_torrent_file(torrent_path: &PathBuf) -> anyhow::Result<()> {
    println!("Parsing torrent file: {}", torrent_path.display());
    println!();

    // Parse the torrent file
    let torrent_info = TorrentParser::parse_file(torrent_path)?;

    // Display torrent information
    println!("=== Torrent Information ===");
    println!("Name: {}", torrent_info.name);
    println!("Info Hash: {}", torrent_info.info_hash_hex());
    println!("Piece Length: {} bytes", torrent_info.piece_length);
    println!("Total Size: {} bytes", torrent_info.total_size());
    println!("Number of Pieces: {}", torrent_info.piece_count());
    println!("Is Multi-file: {}", torrent_info.is_multi_file());

    // Display tracker information
    println!("\n=== Trackers ===");
    println!("Primary Tracker: {}", torrent_info.announce);
    println!("Total Trackers: {}", torrent_info.announce_list.len());
    for (i, tracker) in torrent_info.announce_list.iter().enumerate() {
        println!("  {}. {}", i + 1, tracker);
    }

    // Display file information
    println!("\n=== Files ===");
    for file in torrent_info.files_iter() {
        let path = file.path.join("/");
        println!("  {} ({} bytes)", path, file.length);
    }

    // Create file storage
    let output_dir = PathBuf::from("./downloads");
    println!("\n=== Creating File Storage ===");
    println!("Output Directory: {}", output_dir.display());

    let storage = FileStorage::new(output_dir, Arc::new(torrent_info)).await?;
    storage.create_files().await?;
    println!("File structure created successfully");

    // Generate a peer ID for our client
    let peer_id = Handshake::generate_peer_id();
    println!("\n=== Peer Information ===");
    println!("Our Peer ID: {}", hex::encode(peer_id));

    // Display storage statistics
    println!("\n=== Storage Statistics ===");
    println!("Total Pieces: {}", storage.pieces().piece_count());
    println!("Completed Pieces: {}", storage.pieces().completed_count());
    println!("Progress: {:.2}%", storage.get_progress() * 100.0);

    // Display piece information for the first few pieces
    println!("\n=== Sample Piece Information ===");
    for i in 0..storage.torrent_info().piece_count().min(5) {
        if let Some(piece) = storage.pieces().get_piece(i) {
            println!("Piece {}:", i);
            println!("  Verified: {}", piece.is_verified());
            println!("  Blocks: {}/{}", piece.downloaded_blocks(), piece.block_count());
            if let Some(range) = storage.torrent_info().piece_range(i) {
                println!("  Byte Range: {}-{}", range.0, range.1);
            }
        }
    }

    println!("\n=== Basic Usage Example Complete ===");
    println!("This example demonstrates the basic API usage.");
    println!("For a complete download implementation, see advanced_usage.rs");

    Ok(())
}
