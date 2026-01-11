//! rust-torrent-downloader - Main entry point
//!
//! A full-featured BitTorrent CLI downloader with DHT, seeding, and resume support.

use anyhow::{Context, Result};
use rust_torrent_downloader::{
    CliArgs, Config, ProgressDisplay, DownloadStats,
    TorrentParser, TorrentInfo,
    PeerManager, PeerConnection,
    DownloadManager as StorageDownloadManager,
    DHT,
    TorrentError,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};

/// Set up panic handler for unexpected errors
fn setup_panic_handler() {
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = std::backtrace::Backtrace::capture();
        let location = panic_info.location().unwrap();

        error!(
            "PANIC occurred at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
        let payload = panic_info.payload();
        if let Some(s) = payload.downcast_ref::<&str>() {
            error!("Panic message: {}", s);
        } else if let Some(s) = payload.downcast_ref::<String>() {
            error!("Panic message: {}", s);
        } else {
            error!("Panic message: unknown");
        }
        error!("Backtrace:\n{:?}", backtrace);
    }));
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic handler
    setup_panic_handler();
    
    // Parse CLI arguments
    let args = CliArgs::parse_args();
    info!("rust-torrent-downloader starting");
    debug!("CLI arguments: {:?}", args);

    // Initialize logging
    init_logging(&args);

    // Load torrent file
    let torrent_info = load_torrent_file(&args.torrent_file)
        .context("Failed to load torrent file")?;

    // Create configuration
    let config = Config::from_args(&args, torrent_info.clone());

    // Validate configuration
    config.validate()
        .context("Invalid configuration")?;

    // Display torrent information
    display_torrent_info(&torrent_info, &config)?;

    // Initialize components
    let peer_manager = Arc::new(PeerManager::new(
        config.max_connections,
        Arc::new(torrent_info.clone()),
        rust_torrent_downloader::Handshake::generate_peer_id(),
    ));

    let file_storage = Arc::new(RwLock::new(
        rust_torrent_downloader::FileStorage::new(
            config.output_dir.clone(),
            Arc::new(torrent_info.clone()),
        ).await
            .map_err(|e| {
                error!("Failed to initialize file storage: {}", e);
                anyhow::Error::from(TorrentError::storage_error_full("Failed to initialize file storage", config.output_dir.display().to_string(), e.to_string()))
            })?
    ));

    let download_manager = Arc::new(StorageDownloadManager::new(
        file_storage.clone(),
        peer_manager.clone(),
    ));

    let mut dht = None;
    if config.is_dht_enabled() {
        info!("Initializing DHT...");
        let bind_addr: std::net::SocketAddr = format!("0.0.0.0:{}", config.port).parse()
            .context("Invalid bind address for DHT")?;
        dht = Some(DHT::new(bind_addr, peer_manager.clone()).await
            .map_err(|e| {
                error!("Failed to initialize DHT: {}", e);
                anyhow::Error::from(TorrentError::dht_error_full("Failed to initialize DHT", "unknown", e.to_string()))
            })?);
        info!("DHT initialized successfully");
    }

    // Create progress display
    let mut progress = ProgressDisplay::new(config.is_quiet());

    // Start download
    progress.print_status("Starting download...")?;

    let download_result = run_download(
        &torrent_info,
        &config,
        &peer_manager,
        &download_manager,
        &mut progress,
        dht.as_ref(),
    ).await;

    match download_result {
        Ok(_) => {
            // Download completed
            info!("Download completed successfully");
            progress.print_complete(
                &DownloadStats {
                    downloaded: torrent_info.total_size(),
                    uploaded: 0,
                    download_speed: 0.0,
                    upload_speed: 0.0,
                    peers: 0,
                    progress: 1.0,
                },
                torrent_info.total_size(),
            )?;

            // Handle seeding
            if config.is_seeding_enabled() {
                info!("Seeding enabled. Starting seed phase...");
                run_seeding(&config, &mut progress).await?;
            }
        }
        Err(e) => {
            error!("Download failed: {}", e);
            progress.print_error(&format!("Download failed: {}", e))?;
            return Err(e);
        }
    }

    info!("rust-torrent-downloader finished");
    Ok(())
}

/// Initialize logging based on verbosity settings
fn init_logging(args: &CliArgs) {
    let level = args.log_level();
    debug!("Initializing logging with level: {:?}", level);
    
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    if args.is_verbose() {
        info!("Using pretty log format (verbose mode)");
        subscriber.pretty().init();
    } else {
        info!("Using compact log format");
        subscriber.compact().init();
    }
    
    debug!("Logging initialized successfully");
}

/// Load and parse the torrent file
fn load_torrent_file(path: &Path) -> Result<TorrentInfo> {
    info!("Loading torrent file: {}", path.display());
    debug!("Torrent file path: {}", path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).display());

    let torrent_data = std::fs::read(path)
        .map_err(|e| {
            error!("Failed to read torrent file '{}': {}", path.display(), e);
            anyhow::anyhow!("Failed to read torrent file: {}", e)
        })
        .context("Failed to read torrent file")?;

    debug!("Torrent file size: {} bytes", torrent_data.len());

    let info = TorrentParser::parse_bytes(&torrent_data)
        .map_err(|e| {
            error!("Failed to parse torrent file '{}': {}", path.display(), e);
            anyhow::Error::from(TorrentError::parse_error_with_source("Failed to parse torrent file", e.to_string()))
        })
        .context("Failed to parse torrent file")?;

    info!("Successfully loaded torrent file: {}", info.name);
    Ok(info)
}

/// Display torrent information
fn display_torrent_info(torrent_info: &TorrentInfo, config: &Config) -> Result<()> {
    println!("Torrent Information:");
    println!("  Name: {}", torrent_info.name);
    println!("  Size: {} ({})",
        torrent_info.total_size(),
        DownloadStats::format_bytes(torrent_info.total_size())
    );
    println!("  Pieces: {}", torrent_info.piece_count());
    println!("  Piece length: {}",
        DownloadStats::format_bytes(torrent_info.piece_length)
    );
    println!("  Info hash: {}", torrent_info.info_hash_hex());
    println!();
    println!("Configuration:");
    println!("  Output directory: {}", config.output_dir.display());
    println!("  Listen port: {}", config.port);
    println!("  Max connections: {}", config.max_connections);
    println!("  DHT: {}", if config.is_dht_enabled() { "enabled" } else { "disabled" });
    println!("  Tracker: {}", if config.is_tracker_enabled() { "enabled" } else { "disabled" });
    println!("  Seeding: {}", if config.is_seeding_enabled() { "enabled" } else { "disabled" });
    if config.is_seeding_enabled() {
        if let Some(ratio) = config.seed_ratio_limit() {
            println!("  Seed ratio: {:.1}", ratio);
        }
        if let Some(time) = config.seed_time_limit() {
            println!("  Seed time: {}", DownloadStats::format_duration(time));
        }
    }
    println!();

    Ok(())
}

/// Run the download process
async fn run_download(
torrent_info: &TorrentInfo,
config: &Config,
peer_manager: &Arc<PeerManager>,
download_manager: &Arc<StorageDownloadManager>,
progress: &mut ProgressDisplay,
dht: Option<&DHT>,
) -> Result<()> {
info!("Starting download for: {}", torrent_info.name);
debug!("Total size: {} bytes ({} pieces)", torrent_info.total_size(), torrent_info.piece_count());

// Start the download manager
download_manager.start_download().await
    .map_err(|e| {
        error!("Failed to start download manager: {}", e);
        anyhow::Error::from(TorrentError::storage_error_full("Failed to start download", torrent_info.name.clone(), e.to_string()))
    })?;

// Connect to tracker if enabled
if config.is_tracker_enabled() && !torrent_info.announce.is_empty() {
    info!("Contacting tracker: {}", torrent_info.announce);
    // TODO: Implement tracker communication
    warn!("Tracker communication not yet implemented");
}

// Bootstrap DHT if enabled
if let Some(dht) = dht {
    info!("Bootstrapping DHT...");
    // TODO: Implement DHT bootstrap
    warn!("DHT bootstrap not yet implemented");
}

// Main download loop
let mut last_stats = StorageDownloadManager::get_stats(download_manager).await;
let mut last_time = std::time::Instant::now();
let mut loop_count = 0u64;

loop {
    loop_count += 1;
    trace!("Download loop iteration {}", loop_count);

    // Check if download is complete
    if download_manager.is_complete().await {
        info!("Download complete!");
        break;
    }

    // Get current statistics
    let current_stats = StorageDownloadManager::get_stats(download_manager).await;
    let current_time = std::time::Instant::now();
    let elapsed = current_time.duration_since(last_time);

    // Calculate speeds
    if elapsed.as_secs() > 0 {
        let downloaded_delta = current_stats.downloaded_bytes.saturating_sub(last_stats.downloaded_bytes);
        let uploaded_delta = current_stats.uploaded_bytes.saturating_sub(last_stats.uploaded_bytes);

        let download_speed = downloaded_delta as f64 / elapsed.as_secs_f64();
        let upload_speed = uploaded_delta as f64 / elapsed.as_secs_f64();

        // Get peer count
        let peer_count = peer_manager.connected_addresses().await.len();
        debug!("Connected peers: {}", peer_count);

        // Get progress
        let progress_value = download_manager.get_progress().await;
        trace!("Download progress: {:.2}%", progress_value * 100.0);

        // Update progress display
        let display_stats = DownloadStats {
            downloaded: current_stats.downloaded_bytes,
            uploaded: current_stats.uploaded_bytes,
            download_speed,
            upload_speed,
            peers: peer_count,
            progress: progress_value,
        };

        progress.update(&display_stats, torrent_info.total_size())?;

        last_stats = current_stats;
        last_time = current_time;
    }

    // Request next pieces
    if let Err(e) = download_manager.request_next_pieces().await {
        warn!("Failed to request next pieces: {}", e);
    }

    // Wait a bit before next update
    tokio::time::sleep(Duration::from_millis(500)).await;
}

info!("Download loop finished after {} iterations", loop_count);
Ok(())
}

/// Run the seeding process
async fn run_seeding(config: &Config, progress: &mut ProgressDisplay) -> Result<()> {
    info!("Starting seeding phase");
    debug!("Seed ratio limit: {:?}", config.seed_ratio_limit());
    debug!("Seed time limit: {:?}", config.seed_time_limit());

    let seed_start = std::time::Instant::now();
    let seed_ratio_limit = config.seed_ratio_limit();
    let seed_time_limit = config.seed_time_limit();

    loop {
        // Check seed time limit
        if let Some(time_limit) = seed_time_limit {
            let elapsed = seed_start.elapsed();
            if elapsed >= time_limit {
                info!("Seed time limit reached: {}", DownloadStats::format_duration(time_limit));
                break;
            }
            trace!("Seeding elapsed: {:?}", elapsed);
        }

        // Check seed ratio limit
        // TODO: Implement ratio tracking
        if let Some(ratio_limit) = seed_ratio_limit {
            warn!("Seed ratio tracking not yet implemented");
        }

        // Wait a bit before next check
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    info!("Seeding phase complete");
    Ok(())
}
