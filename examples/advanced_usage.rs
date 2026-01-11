//! Advanced usage example for rust-torrent-downloader
//!
//! This example demonstrates advanced features including:
//! - Peer management and connection handling
//! - Download management with piece verification
//! - DHT peer discovery
//! - Resume data handling
//! - Progress tracking
//!
//! Run this example with:
//! ```bash
//! cargo run --example advanced_usage -- <path-to-torrent-file>
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use rust_torrent_downloader::torrent::TorrentParser;
use rust_torrent_downloader::storage::{FileStorage, DownloadManager, ResumeManager};
use rust_torrent_downloader::peer::{PeerManager, PeerInfo, PeerSource};
use rust_torrent_downloader::protocol::Handshake;
use rust_torrent_downloader::dht::{Node, NodeId, RoutingTable, bootstrap_and_discover};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Get torrent file path from command line
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <torrent-file>", args[0]);
        eprintln!("Example: {} example.torrent", args[0]);
        std::process::exit(1);
    }

    let torrent_path = PathBuf::from(&args[1]);

    // ========================================
    // Step 1: Parse torrent file
    // ========================================
    println!("=== Step 1: Parsing Torrent File ===");
    let torrent_info = TorrentParser::parse_file(&torrent_path)?;
    println!("Torrent: {}", torrent_info.name);
    println!("Size: {} bytes ({:.2} MB)", 
        torrent_info.total_size(), 
        torrent_info.total_size() as f64 / (1024.0 * 1024.0)
    );
    println!("Pieces: {}", torrent_info.piece_count());

    // ========================================
    // Step 2: Initialize file storage
    // ========================================
    println!("\n=== Step 2: Initializing Storage ===");
    let output_dir = PathBuf::from("./downloads");
    let storage = Arc::new(RwLock::new(
        FileStorage::new(output_dir.clone(), Arc::new(torrent_info.clone())).await?
    ));
    storage.read().await.create_files().await?;
    println!("Storage initialized at: {}", output_dir.display());

    // ========================================
    // Step 3: Check for resume data
    // ========================================
    println!("\n=== Step 3: Checking Resume Data ===");
    let resume_manager = ResumeManager::new(PathBuf::from(".resume"));
    let info_hash_hex = torrent_info.info_hash_hex();
    
    if resume_manager.has_resume_data(&info_hash_hex).await {
        println!("Resume data found for: {}", torrent_info.name);
        if let Some(resume_data) = resume_manager.load_resume_data(&info_hash_hex).await? {
            let downloaded = resume_data.downloaded_count();
            let total = torrent_info.piece_count();
            println!("  Progress: {}/{} pieces ({:.2}%)", 
                downloaded, total, 
                (downloaded as f64 / total as f64) * 100.0
            );
            
            // Load resume data into storage
            storage.write().await.load_resume(&resume_data).await?;
            println!("Resume data loaded successfully");
        }
    } else {
        println!("No resume data found - starting fresh download");
    }

    // ========================================
    // Step 4: Initialize peer manager
    // ========================================
    println!("\n=== Step 4: Initializing Peer Manager ===");
    let our_peer_id = Handshake::generate_peer_id();
    let max_peers = 50;
    
    let peer_manager = Arc::new(
        PeerManager::new(max_peers, Arc::new(torrent_info.clone()), our_peer_id)
    );
    println!("Peer ID: {}", hex::encode(our_peer_id));
    println!("Max connections: {}", max_peers);

    // ========================================
    // Step 5: Add some example peers
    // ========================================
    println!("\n=== Step 5: Adding Example Peers ===");
    let example_peers = vec![
        "127.0.0.1:6881".parse().unwrap(),
        "127.0.0.1:6882".parse().unwrap(),
        "127.0.0.1:6883".parse().unwrap(),
    ];
    
    for peer_addr in &example_peers {
        let peer_info = PeerInfo::new(*peer_addr, PeerSource::Manual);
        peer_manager.add_peer(*peer_addr).await?;
        println!("  Added peer: {}", peer_addr);
    }

    // ========================================
    // Step 6: Initialize DHT
    // ========================================
    println!("\n=== Step 6: Initializing DHT ===");
    let our_node_id = NodeId::random();
    let mut routing_table = RoutingTable::new(our_node_id);
    
    // Create UDP socket for DHT
    let dht_socket = tokio::net::UdpSocket::bind("0.0.0.0:6881").await?;
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
        let peer_info = PeerInfo::new(peer_addr, PeerSource::DHT);
        peer_manager.add_peer(peer_addr).await?;
    }

    println!("Total known peers: {}", peer_manager.peer_count().await);

    // ========================================
    // Step 7: Initialize download manager
    // ========================================
    println!("\n=== Step 7: Initializing Download Manager ===");
    let download_manager = Arc::new(
        DownloadManager::new(storage.clone(), peer_manager.clone())
    );
    
    // Configure download manager
    download_manager.set_max_concurrent_downloads(5);
    download_manager.set_block_size(16 * 1024); // 16KB blocks
    
    println!("Max concurrent downloads: 5");
    println!("Block size: 16 KB");

    // ========================================
    // Step 8: Display download statistics
    // ========================================
    println!("\n=== Step 8: Download Statistics ===");
    
    // Start download (this is a simulation - actual download would require network)
    let progress_task = tokio::spawn({
        let download_manager = download_manager.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            
            loop {
                interval.tick().await;
                
                let progress = download_manager.get_progress().await;
                let stats = download_manager.get_stats().await;
                
                // Clear terminal and display progress
                print!("\x1B[2J\x1B[1;1H");
                println!("=== Download Progress ===");
                println!("Progress: {:.2}%", progress * 100.0);
                println!("Downloaded: {} bytes ({:.2} MB)", 
                    stats.downloaded_bytes,
                    stats.downloaded_bytes as f64 / (1024.0 * 1024.0)
                );
                println!("Uploaded: {} bytes ({:.2} MB)", 
                    stats.uploaded_bytes,
                    stats.uploaded_bytes as f64 / (1024.0 * 1024.0)
                );
                println!("Pieces Downloaded: {}", stats.pieces_downloaded);
                println!("Pieces Verified: {}", stats.pieces_verified);
                println!("Pieces Failed: {}", stats.pieces_failed);
                println!("Active Downloads: {}", download_manager.active_download_count().await);
                
                // Display progress bar
                let bar_width = 50;
                let filled = (progress * bar_width as f64) as usize;
                let empty = bar_width - filled;
                print!("[");
                print!("{}", "=".repeat(filled));
                print!("{}", " ".repeat(empty));
                print!("] {:.2}%\n", progress * 100.0);
                
                // Exit if complete
                if progress >= 1.0 {
                    println!("\nDownload complete!");
                    break;
                }
            }
        }
    });

    // ========================================
    // Step 9: Save resume data periodically
    // ========================================
    println!("\n=== Step 9: Resume Data Management ===");
    
    let save_task = tokio::spawn({
        let storage = storage.clone();
        let resume_manager = resume_manager.clone();
        let torrent_info = torrent_info.clone();
        
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Get current resume data
                let resume_data = storage.read().await.resume_data();
                
                // Save to file
                if let Err(e) = resume_manager.save_resume_data(&resume_data).await {
                    eprintln!("Failed to save resume data: {}", e);
                } else {
                    println!("Resume data saved: {}/{} pieces complete", 
                        resume_data.downloaded_count(),
                        torrent_info.piece_count()
                    );
                }
            }
        }
    });

    // ========================================
    // Step 10: Display peer information
    // ========================================
    println!("\n=== Step 10: Peer Information ===");
    
    let all_stats = peer_manager.get_all_stats().await;
    println!("Total Peers: {}", all_stats.len());
    
    for (addr, stats) in all_stats.iter().take(5) {
        println!("\nPeer: {}", addr);
        println!("  State: {:?}", stats.state);
        println!("  Am Choking: {}", stats.am_choking);
        println!("  Am Interested: {}", stats.am_interested);
        println!("  Peer Choking: {}", stats.peer_choking);
        println!("  Peer Interested: {}", stats.peer_interested);
        println!("  Downloaded: {} pieces", stats.pieces_downloaded);
        println!("  Uploaded: {} pieces", stats.pieces_uploaded);
        if let Some(peer_id) = stats.peer_id_hex() {
            println!("  Peer ID: {}", peer_id);
        }
    }

    // ========================================
    // Step 11: Display routing table information
    // ========================================
    println!("\n=== Step 11: DHT Routing Table ===");
    let nodes = routing_table.get_nodes();
    println!("Total DHT Nodes: {}", nodes.len());
    
    for node in nodes.iter().take(5) {
        println!("  Node: {} ({})", 
            node.addr, 
            node.id.to_hex()
        );
        println!("    Last seen: {:?}", node.time_since_seen());
        println!("    Is good: {}", node.is_good());
    }

    // ========================================
    // Summary and cleanup
    // ========================================
    println!("\n=== Advanced Usage Example Summary ===");
    println!("This example demonstrated:");
    println!("  ✓ Torrent file parsing");
    println!("  ✓ File storage initialization");
    println!("  ✓ Resume data handling");
    println!("  ✓ Peer management");
    println!("  ✓ DHT bootstrapping and peer discovery");
    println!("  ✓ Download manager configuration");
    println!("  ✓ Progress tracking");
    println!("  ✓ Periodic resume data saving");
    println!("  ✓ Peer statistics");
    println!("  ✓ DHT routing table management");
    
    println!("\nNote: This is a demonstration of the API.");
    println!("For actual downloads, you would:");
    println!("  1. Connect to peers via tracker or DHT");
    println!("  2. Perform handshake with each peer");
    println!("  3. Request and download pieces");
    println!("  4. Verify pieces against torrent hashes");
    println!("  5. Write verified pieces to disk");
    println!("  6. Continue until download is complete");

    // Cancel tasks after a short delay (for demo purposes)
    tokio::time::sleep(Duration::from_secs(5)).await;
    progress_task.abort();
    save_task.abort();

    println!("\nExample complete!");
    
    Ok(())
}

/// Helper function to display a formatted progress bar
fn display_progress_bar(progress: f64, width: usize) -> String {
    let filled = (progress * width as f64) as usize;
    let empty = width - filled;
    format!("[{}{}] {:.2}%", "=".repeat(filled), " ".repeat(empty), progress * 100.0)
}
