//! Basic usage example for rust-torrent-downloader
//!
//! This example demonstrates how to use the library to:
//! - Parse a torrent file
//! - Get torrent information
//! - Create a download manager
//! - Initialize file storage
//!
//! Run this example with:
//! ```bash
//! cargo run --example basic_usage -- <path-to-torrent-file>
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use rust_torrent_downloader::torrent::TorrentParser;
use rust_torrent_downloader::storage::FileStorage;
use rust_torrent_downloader::protocol::Handshake;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Get torrent file path from command line
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <torrent-file>", args[0]);
        eprintln!("Example: {} example.torrent", args[0]);
        std::process::exit(1);
    }

    let torrent_path = PathBuf::from(&args[1]);

    // Parse the torrent file
    println!("Parsing torrent file: {}", torrent_path.display());
    let torrent_info = TorrentParser::parse_file(&torrent_path)?;

    // Display torrent information
    println!("\n=== Torrent Information ===");
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
    for i in 0..torrent_info.piece_count().min(5) {
        if let Some(piece) = storage.pieces().get_piece(i) {
            println!("Piece {}:", i);
            println!("  Verified: {}", piece.is_verified());
            println!("  Blocks: {}/{}", piece.downloaded_blocks(), piece.block_count());
            if let Some(range) = torrent_info.piece_range(i) {
                println!("  Byte Range: {}-{}", range.0, range.1);
            }
        }
    }

    println!("\n=== Basic Usage Example Complete ===");
    println!("This example demonstrates the basic API usage.");
    println!("For a complete download implementation, see advanced_usage.rs");

    Ok(())
}
