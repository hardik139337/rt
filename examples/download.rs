//! Download example for rust-torrent-downloader
//!
//! This example demonstrates how to download torrents from:
//! - .torrent files
//! - Magnet links (with exact source support)
//!
//! Run this example with:
//! ```bash
//! # For torrent files:
//! cargo run --example download -- <path-to-torrent-file>
//!
//! # For magnet links:
//! cargo run --example download -- <magnet-link>
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use rust_torrent_downloader::torrent::{TorrentParser, MagnetParser};
use rust_torrent_downloader::storage::FileStorage;
use rust_torrent_downloader::peer::PeerManager;
use rust_torrent_downloader::protocol::Handshake;
use rust_torrent_downloader::dht::{NodeId, RoutingTable, bootstrap_and_discover};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Get torrent file path or magnet link from command line
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <torrent-file-or-magnet-link>", args[0]);
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  Torrent file: {} example.torrent", args[0]);
        eprintln!("  Magnet link: {} magnet:?xt=urn:btih:...", args[0]);
        std::process::exit(1);
    }

    let input = &args[1];

    // Check if input is a magnet link
    let torrent_info = if MagnetParser::is_magnet_link(input) {
        handle_magnet_link(input).await?
    } else {
        // Treat as a torrent file path
        let torrent_path = PathBuf::from(input);
        println!("=== Parsing Torrent File ===");
        Arc::new(TorrentParser::parse_file(&torrent_path)?)
    };

    println!("\n=== Torrent Information ===");
    println!("Name: {}", torrent_info.name);
    println!("Size: {} bytes ({:.2} MB)",
        torrent_info.total_size(),
        torrent_info.total_size() as f64 / (1024.0 * 1024.0)
    );
    println!("Pieces: {}", torrent_info.piece_count());
    println!("Info Hash: {}", torrent_info.info_hash_hex());

    // Display trackers
    println!("\n=== Trackers ===");
    for (i, tracker) in torrent_info.announce_list.iter().enumerate() {
        println!("  {}. {}", i + 1, tracker);
    }

    // Initialize file storage
    println!("\n=== Initializing Storage ===");
    let output_dir = PathBuf::from("./downloads");
    let storage = Arc::new(RwLock::new(
        FileStorage::new(output_dir.clone(), torrent_info.clone()).await?
    ));
    storage.read().await.create_files().await?;
    println!("Storage initialized at: {}", output_dir.display());

    // Initialize peer manager
    println!("\n=== Initializing Peer Manager ===");
    let our_peer_id = Handshake::generate_peer_id();
    let max_peers = 50;

    let peer_manager = Arc::new(
        PeerManager::new(max_peers, torrent_info.clone(), our_peer_id)
    );
    println!("Peer ID: {}", hex::encode(our_peer_id));
    println!("Max connections: {}", max_peers);

    // Initialize DHT
    println!("\n=== Initializing DHT ===");
    let our_node_id = NodeId::random();
    let mut routing_table = RoutingTable::new(our_node_id);

    // Create UDP socket for DHT
    let dht_socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
    println!("DHT socket bound to: {}", dht_socket.local_addr()?);
    println!("Our Node ID: {}", our_node_id.to_hex());

    // Bootstrap DHT and discover peers
    let discovered_peers = bootstrap_and_discover(
        &dht_socket,
        our_node_id,
        &mut routing_table,
        torrent_info.info_hash,
    ).await?;

    println!("DHT bootstrap complete");
    println!("Discovered {} peers via DHT", discovered_peers.len());

    // Add discovered peers to peer manager
    for peer_addr in discovered_peers {
        peer_manager.add_peer(peer_addr).await?;
    }

    println!("\nTotal known peers: {}", peer_manager.peer_count().await);

    // Display current progress
    println!("\n=== Current Progress ===");
    let progress = storage.read().await.get_progress();
    let downloaded_pieces = storage.read().await.downloaded_count();
    let total_pieces = torrent_info.piece_count();
    println!("Downloaded: {}/{} pieces ({:.2}%)",
        downloaded_pieces,
        total_pieces,
        progress * 100.0
    );

    // Display file list
    println!("\n=== Files ===");
    for file in torrent_info.files_iter() {
        let path = file.path.join("/");
        let size_mb = file.length as f64 / (1024.0 * 1024.0);
        println!("  {} ({:.2} MB)", path, size_mb);
    }

    // Note: Actual peer connection and piece download would happen here
    // This example demonstrates the setup but doesn't implement full
    // peer protocol communication
    println!("\n=== Setup Complete ===");
    println!("Note: This example sets up the download infrastructure.");
    println!("For actual peer communication and downloading, you would:");
    println!("  1. Connect to discovered peers");
    println!("  2. Perform BitTorrent handshake");
    println!("  3. Exchange messages and download pieces");
    println!("  4. Verify pieces against torrent hashes");
    println!("  5. Save pieces to disk");
    println!();
    println!("The advanced_usage.rs example shows the full download manager setup.");

    Ok(())
}

/// Handle magnet link by fetching torrent metadata from exact source if available
async fn handle_magnet_link(magnet_uri: &str) -> anyhow::Result<Arc<rust_torrent_downloader::torrent::TorrentInfo>> {
    println!("=== Parsing Magnet Link ===");
    let magnet_info = MagnetParser::parse(magnet_uri)?;

    println!("Info Hash: {}", hex::encode(magnet_info.info_hash));
    println!("Display Name: {}", magnet_info.display_name.as_deref().unwrap_or("(none)"));

    // Display trackers from magnet link
    if !magnet_info.trackers.is_empty() {
        println!("\n=== Magnet Link Trackers ===");
        for (i, tracker) in magnet_info.trackers.iter().enumerate() {
            println!("  {}. {}", i + 1, tracker);
        }
    }

    // Check if we have an exact source (xs parameter)
    if !magnet_info.exact_sources.is_empty() {
        println!("\n=== Fetching Torrent from Exact Source ===");
        let torrent_url = &magnet_info.exact_sources[0];
        println!("Torrent URL: {}", torrent_url);

        // Download the torrent file
        let client = reqwest::Client::builder()
            .no_gzip()
            .no_brotli()
            .build()?;
        let response = client.get(torrent_url).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("Failed to download torrent file: HTTP {}", response.status());
        }

        let torrent_bytes = response.bytes().await?;
        println!("Downloaded {} bytes", torrent_bytes.len());

        // Parse the torrent file
        println!("Parsing torrent file...");
        let torrent_info = TorrentParser::parse_bytes(&torrent_bytes)?;
        println!("Successfully parsed torrent: {}", torrent_info.name);

        return Ok(Arc::new(torrent_info));
    }

    // No exact source available - need DHT metadata exchange
    println!("\n=== No Exact Source Available ===");
    println!("This magnet link does not include an exact source URL.");
    println!("To download from this magnet link, you would need to:");
    println!("  1. Connect to DHT network");
    println!("  2. Find peers with this info hash");
    println!("  3. Exchange metadata using extension protocol (BEP-9)");
    println!("  4. Download torrent metadata from peers");
    println!();
    println!("For now, please use a magnet link with an exact source (xs parameter)");
    println!("or provide a .torrent file directly.");

    anyhow::bail!("Magnet link without exact source is not yet supported");
}
