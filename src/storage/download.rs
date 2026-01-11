//! Download manager module
//!
//! Manages the download process for torrents with piece verification.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};
use crate::peer::PeerManager;
use crate::protocol::Message;
use crate::storage::file::FileStorage;
use crate::error::TorrentError;

/// Download statistics
#[derive(Debug, Clone, Default)]
pub struct DownloadStats {
    /// Total bytes downloaded
    pub downloaded_bytes: u64,
    /// Total bytes uploaded
    pub uploaded_bytes: u64,
    /// Pieces downloaded
    pub pieces_downloaded: usize,
    /// Pieces verified
    pub pieces_verified: usize,
    /// Pieces failed verification
    pub pieces_failed: usize,
    /// Download speed in bytes per second
    pub download_speed: f64,
    /// Upload speed in bytes per second
    pub upload_speed: f64,
}

/// Active piece download state
#[derive(Debug, Clone)]
pub struct PieceDownload {
    /// Piece index being downloaded
    pub piece_index: u32,
    /// Which blocks have been downloaded
    pub blocks_downloaded: Vec<bool>,
    /// Total number of blocks
    pub blocks_total: usize,
    /// When this download started
    pub started_at: Instant,
    /// Peers that are downloading this piece
    pub peers: HashSet<std::net::SocketAddr>,
}

impl PieceDownload {
    /// Create a new piece download
    pub fn new(piece_index: u32, blocks_total: usize) -> Self {
        Self {
            piece_index,
            blocks_downloaded: vec![false; blocks_total],
            blocks_total,
            started_at: Instant::now(),
            peers: HashSet::new(),
        }
    }

    /// Mark a block as downloaded
    pub fn mark_block_downloaded(&mut self, block_index: usize) {
        if block_index < self.blocks_downloaded.len() {
            self.blocks_downloaded[block_index] = true;
        }
    }

    /// Check if all blocks are downloaded
    pub fn is_complete(&self) -> bool {
        self.blocks_downloaded.iter().all(|&b| b)
    }

    /// Get number of downloaded blocks
    pub fn downloaded_blocks(&self) -> usize {
        self.blocks_downloaded.iter().filter(|&&b| b).count()
    }

    /// Get progress (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.blocks_total == 0 {
            0.0
        } else {
            self.downloaded_blocks() as f64 / self.blocks_total as f64
        }
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Add a peer to this download
    pub fn add_peer(&mut self, addr: std::net::SocketAddr) {
        self.peers.insert(addr);
    }

    /// Remove a peer from this download
    pub fn remove_peer(&mut self, addr: &std::net::SocketAddr) {
        self.peers.remove(addr);
    }

    /// Get number of peers downloading this piece
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
}

/// Download manager for torrents
pub struct DownloadManager {
    /// File storage for the torrent
    storage: Arc<RwLock<FileStorage>>,
    /// Peer manager
    peer_manager: Arc<PeerManager>,
    /// Active piece downloads
    active_downloads: Arc<RwLock<HashMap<u32, PieceDownload>>>,
    /// Track requested blocks (piece_index, block_index) -> requested_at
    requested_blocks: Arc<RwLock<HashMap<(u32, u32), Instant>>>,
    /// Download statistics
    stats: Arc<RwLock<DownloadStats>>,
    /// Maximum concurrent piece downloads
    max_concurrent_downloads: usize,
    /// Block size for requests
    block_size: u32,
}

impl DownloadManager {
    /// Create a new download manager
    pub fn new(
        storage: Arc<RwLock<FileStorage>>,
        peer_manager: Arc<PeerManager>,
    ) -> Self {
        info!("Creating download manager");
        Self {
            storage,
            peer_manager,
            active_downloads: Arc::new(RwLock::new(HashMap::new())),
            requested_blocks: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(DownloadStats::default())),
            max_concurrent_downloads: 5,
            block_size: 16 * 1024, // 16KB blocks
        }
    }

    /// Set maximum concurrent piece downloads
    pub fn set_max_concurrent_downloads(&mut self, max: usize) {
        self.max_concurrent_downloads = max;
    }

    /// Set block size for requests
    pub fn set_block_size(&mut self, size: u32) {
        self.block_size = size;
    }

    /// Start downloading the torrent
    pub async fn start_download(&self) -> Result<()> {
        info!("Starting download");
        
        // Create file structure if needed
        let storage = self.storage.read().await;
        storage.create_files().await
            .map_err(|e| {
                error!("Failed to create file structure: {}", e);
                TorrentError::storage_error_full("Failed to create file structure", "unknown".to_string(), e.to_string())
            })?;
        drop(storage);

        // Request initial pieces
        self.request_next_pieces().await?;

        info!("Download started successfully");
        Ok(())
    }

    /// Request the next pieces to download
    pub async fn request_next_pieces(&self) -> Result<()> {
        let active_downloads = self.active_downloads.read().await;
        let current_count = active_downloads.len();
        drop(active_downloads);

        if current_count >= self.max_concurrent_downloads {
            trace!("Max concurrent downloads reached ({}), skipping request", current_count);
            return Ok(());
        }

        let slots_available = self.max_concurrent_downloads - current_count;
        debug!("Requesting next pieces ({} slots available)", slots_available);
        let pieces_to_download = self.select_pieces(slots_available).await?;

        for piece_index in &pieces_to_download {
            self.start_piece_download(*piece_index).await?;
        }

        debug!("Requested {} new pieces", pieces_to_download.len());
        Ok(())
    }

    /// Select pieces to download using rarest-first strategy
    async fn select_pieces(&self, count: usize) -> Result<Vec<u32>> {
        let storage = self.storage.read().await;
        let pieces = storage.pieces();
        let active_downloads = self.active_downloads.read().await;

        // Find pieces that are not downloaded and not being downloaded
        let mut available_pieces: Vec<u32> = pieces.pieces()
            .iter()
            .filter(|p| !p.is_verified())
            .filter(|p| !active_downloads.contains_key(&p.index))
            .map(|p| p.index)
            .collect();

        debug!("Found {} available pieces to download", available_pieces.len());

        // Sort by rarity (fewest peers have piece)
        // For now, use random selection as a simple strategy
        // In a full implementation, we would query peers for their bitfields
        use rand::seq::SliceRandom;
        available_pieces.shuffle(&mut rand::thread_rng());

        let selected: Vec<_> = available_pieces.into_iter().take(count).collect();
        debug!("Selected {} pieces for download", selected.len());
        drop(storage);
        drop(active_downloads);

        Ok(selected)
    }

    /// Start downloading a specific piece
    async fn start_piece_download(&self, piece_index: u32) -> Result<()> {
        info!("Starting download of piece {}", piece_index);
        
        let storage = self.storage.read().await;
        let piece = storage.pieces().get_piece(piece_index as usize)
            .ok_or_else(|| {
                error!("Invalid piece index: {}", piece_index);
                TorrentError::validation_error_with_field("Invalid piece index", "piece_index".to_string())
            })?;

        let block_count = piece.block_count();
        debug!("Piece {} has {} blocks", piece_index, block_count);
        drop(storage);

        let mut active_downloads = self.active_downloads.write().await;
        active_downloads.insert(piece_index, PieceDownload::new(piece_index, block_count));
        drop(active_downloads);

        // Request blocks from peers
        self.request_piece_blocks(piece_index).await?;

        debug!("Piece {} download started", piece_index);
        Ok(())
    }

    /// Request blocks for a piece from peers
    async fn request_piece_blocks(&self, piece_index: u32) -> Result<()> {
        debug!("Requesting blocks for piece {}", piece_index);
        
        let storage = self.storage.read().await;
        let piece = storage.pieces().get_piece(piece_index as usize)
            .ok_or_else(|| {
                error!("Invalid piece index: {}", piece_index);
                TorrentError::validation_error_with_field("Invalid piece index", "piece_index".to_string())
            })?;

        let missing_blocks = piece.get_missing_blocks();
        debug!("Piece {} has {} missing blocks", piece_index, missing_blocks.len());
        drop(storage);

        // Select a peer for this piece
        let peer_addr = self.select_peer_for_piece(piece_index).await?;

        // Request each block
        for (offset, length) in &missing_blocks {
            trace!("Requesting block at offset {} ({} bytes)", offset, length);
            self.request_block(piece_index, *offset, *length, peer_addr).await?;
        }

        debug!("Requested {} blocks for piece {}", missing_blocks.len(), piece_index);
        Ok(())
    }

    /// Select a peer for downloading a piece
    async fn select_peer_for_piece(&self, piece_index: u32) -> Result<std::net::SocketAddr> {
        debug!("Selecting peer for piece {}", piece_index);
        
        // For now, return the first connected peer
        // In a full implementation, we would select based on:
        // - Peer has the piece
        // - Peer is not choking us
        // - Peer has good download speed
        // - We haven't downloaded too much from this peer recently

        let connected_addrs = self.peer_manager.connected_addresses().await;
        if connected_addrs.is_empty() {
            error!("No connected peers available");
            return Err(TorrentError::peer_error("No connected peers available").into());
        }

        let peer_addr = connected_addrs[0];
        debug!("Selected peer {} for piece {}", peer_addr, piece_index);
        Ok(peer_addr)
    }

    /// Request a block from a peer
    async fn request_block(
        &self,
        piece_index: u32,
        offset: u32,
        length: u32,
        peer_addr: std::net::SocketAddr,
    ) -> Result<()> {
        // Record request time
        let block_index = offset / self.block_size;
        let mut requested_blocks = self.requested_blocks.write().await;
        requested_blocks.insert((piece_index, block_index), Instant::now());
        drop(requested_blocks);

        // Send Request message to peer
        // Note: This is a placeholder - in a real implementation, we would
        // send the message through the peer connection
        let message = Message::Request {
            index: piece_index,
            begin: offset,
            length,
        };

        // TODO: Send message through peer connection
        let _ = message;

        Ok(())
    }

    /// Handle a Piece message from a peer
    pub async fn handle_piece_message(
        &self,
        piece_index: u32,
        offset: u32,
        block_data: Vec<u8>,
    ) -> Result<()> {
        trace!("Received piece {} block {} ({} bytes)", piece_index, offset, block_data.len());
        let block_index = offset / self.block_size;

        // Remove from requested blocks
        let mut requested_blocks = self.requested_blocks.write().await;
        requested_blocks.remove(&(piece_index, block_index));
        drop(requested_blocks);

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.downloaded_bytes += block_data.len() as u64;
        drop(stats);

        // Add block to piece
        let mut storage = self.storage.write().await;
        let piece = storage.pieces_mut().get_piece_mut(piece_index as usize)
            .ok_or_else(|| {
                error!("Invalid piece index: {}", piece_index);
                TorrentError::validation_error_with_field("Invalid piece index", "piece_index".to_string())
            })?;

        piece.add_block(offset, block_data)?;

        // Update active download state
        let mut active_downloads = self.active_downloads.write().await;
        if let Some(download) = active_downloads.get_mut(&piece_index) {
            download.mark_block_downloaded(block_index as usize);
            debug!("Piece {} progress: {}/{} blocks", piece_index, download.downloaded_blocks(), download.blocks_total);
        }

        // Check if piece is complete
        if piece.is_complete() {
            info!("Piece {} download complete, verifying...", piece_index);
            
            // Get piece data before dropping storage
            let piece_data = piece.data().to_vec();
            // Verify piece
            let is_valid = piece.verify();
            drop(storage);

            if is_valid {
                // Write piece to disk
                debug!("Piece {} verified, writing to disk", piece_index);
                let storage = self.storage.read().await;
                storage.write_piece(piece_index, &piece_data).await
                    .map_err(|e| {
                        error!("Failed to write piece {} to disk: {}", piece_index, e);
                        TorrentError::storage_error_full("Failed to write piece", "unknown".to_string(), e.to_string())
                    })?;
                drop(storage);

                // Update statistics
                let mut stats = self.stats.write().await;
                stats.pieces_downloaded += 1;
                stats.pieces_verified += 1;
                drop(stats);
                info!("Piece {} verified and written successfully", piece_index);

                // Remove from active downloads
                active_downloads.remove(&piece_index);

                // Request next pieces
                drop(active_downloads);
                self.request_next_pieces().await?;
            } else {
                // Piece verification failed
                warn!("Piece {} verification FAILED, retrying...", piece_index);
                let mut stats = self.stats.write().await;
                stats.pieces_failed += 1;
                drop(stats);

                // Clear piece and retry
                let mut storage = self.storage.write().await;
                if let Some(piece) = storage.pieces_mut().get_piece_mut(piece_index as usize) {
                    piece.clear();
                }
                drop(storage);

                // Restart piece download
                active_downloads.remove(&piece_index);
                drop(active_downloads);
                self.start_piece_download(piece_index).await?;
            }
        }

        Ok(())
    }

    /// Cancel slow piece requests
    pub async fn cancel_slow_peers(&self, timeout: Duration) -> Result<()> {
        debug!("Cancelling slow peer requests (timeout: {:?})", timeout);
        let now = Instant::now();
        let mut requested_blocks = self.requested_blocks.write().await;

        let mut blocks_to_cancel: Vec<(u32, u32)> = Vec::new();
        for ((piece_index, block_index), requested_at) in requested_blocks.iter() {
            if now.duration_since(*requested_at) > timeout {
                warn!("Cancelling slow request for piece {} block {} (elapsed: {:?})",
                    piece_index, block_index, now.duration_since(*requested_at));
                blocks_to_cancel.push((*piece_index, *block_index));
            }
        }

        for (piece_index, block_index) in &blocks_to_cancel {
            requested_blocks.remove(&(*piece_index, *block_index));
            // TODO: Send Cancel message to peer
        }

        drop(requested_blocks);
        debug!("Cancelled {} slow requests", blocks_to_cancel.len());
        Ok(())
    }

    /// Check if download is complete
    pub async fn is_complete(&self) -> bool {
        let storage = self.storage.read().await;
        storage.is_complete()
    }

    /// Get download statistics
    pub async fn get_stats(&self) -> DownloadStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get download progress (0.0 to 1.0)
    pub async fn get_progress(&self) -> f64 {
        let storage = self.storage.read().await;
        storage.get_progress()
    }

    /// Get the number of active downloads
    pub async fn active_download_count(&self) -> usize {
        let active_downloads = self.active_downloads.read().await;
        active_downloads.len()
    }

    /// Get the active downloads
    pub async fn get_active_downloads(&self) -> Vec<PieceDownload> {
        let active_downloads = self.active_downloads.read().await;
        active_downloads.values().cloned().collect()
    }

    /// Cancel a piece download
    pub async fn cancel_piece_download(&self, piece_index: u32) -> Result<()> {
        info!("Cancelling download of piece {}", piece_index);
        let mut active_downloads = self.active_downloads.write().await;
        if active_downloads.remove(&piece_index).is_some() {
            debug!("Removed piece {} from active downloads", piece_index);
            // Clear requested blocks for this piece
            let mut requested_blocks = self.requested_blocks.write().await;
            let before_count = requested_blocks.len();
            requested_blocks.retain(|p, _b| p.0 != piece_index);
            let after_count = requested_blocks.len();
            drop(requested_blocks);
            debug!("Cleared {} requested blocks for piece {}", before_count - after_count, piece_index);
        } else {
            warn!("Piece {} not found in active downloads", piece_index);
        }
        drop(active_downloads);

        Ok(())
    }

    /// Pause download
    pub async fn pause(&self) -> Result<()> {
        info!("Pausing download");
        // Cancel all active downloads
        let active_downloads = self.active_downloads.read().await;
        let piece_indices: Vec<u32> = active_downloads.keys().copied().collect();
        drop(active_downloads);

        debug!("Cancelling {} active piece downloads", piece_indices.len());
        for piece_index in piece_indices {
            self.cancel_piece_download(piece_index).await?;
        }

        info!("Download paused");
        Ok(())
    }

    /// Resume download
    pub async fn resume(&self) -> Result<()> {
        info!("Resuming download");
        self.request_next_pieces().await
    }
}
