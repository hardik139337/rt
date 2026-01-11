//! Progress display module
//!
//! Handles displaying download progress in the CLI.

use std::io::{self, Write};
use std::time::{Duration, Instant};

/// Download statistics for progress display
#[derive(Debug, Clone, Default)]
pub struct DownloadStats {
    /// Total bytes downloaded
    pub downloaded: u64,
    /// Total bytes uploaded
    pub uploaded: u64,
    /// Download speed in bytes per second
    pub download_speed: f64,
    /// Upload speed in bytes per second
    pub upload_speed: f64,
    /// Number of connected peers
    pub peers: usize,
    /// Download progress (0.0 to 1.0)
    pub progress: f64,
}

impl DownloadStats {
    /// Create new download stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Format bytes to human readable string
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_index])
    }

    /// Format speed to human readable string
    pub fn format_speed(bytes_per_sec: f64) -> String {
        format!("{}/s", Self::format_bytes(bytes_per_sec as u64))
    }

    /// Format duration to human readable string
    pub fn format_duration(duration: Duration) -> String {
        let total_secs = duration.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Calculate ETA based on download speed and remaining bytes
    pub fn calculate_eta(downloaded: u64, total: u64, speed: f64) -> Option<Duration> {
        if speed <= 0.0 || downloaded >= total {
            return None;
        }

        let remaining = total.saturating_sub(downloaded) as f64;
        let eta_secs = remaining / speed;
        Some(Duration::from_secs_f64(eta_secs))
    }
}

/// Progress display for CLI
pub struct ProgressDisplay {
    /// Start time of the download
    start_time: Instant,
    /// Last update time
    last_update: Instant,
    /// Update interval
    update_interval: Duration,
    /// Quiet mode (no progress output)
    quiet: bool,
    /// Previous line length for clearing
    prev_line_len: usize,
}

impl ProgressDisplay {
    /// Create a new progress display
    pub fn new(quiet: bool) -> Self {
        Self {
            start_time: Instant::now(),
            last_update: Instant::now(),
            update_interval: Duration::from_millis(500),
            quiet,
            prev_line_len: 0,
        }
    }

    /// Create a progress display with default settings
    pub fn default() -> Self {
        Self::new(false)
    }

    /// Create a progress display with custom update interval
    pub fn with_interval(quiet: bool, interval: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            last_update: Instant::now(),
            update_interval: interval,
            quiet,
            prev_line_len: 0,
        }
    }

    /// Update the progress display
    pub fn update(&mut self, stats: &DownloadStats, total: u64) -> io::Result<()> {
        if self.quiet {
            return Ok(());
        }

        // Check if enough time has passed since last update
        if self.last_update.elapsed() < self.update_interval {
            return Ok(());
        }

        self.last_update = Instant::now();

        self.print_progress(stats, total)?;
        io::stdout().flush()?;

        Ok(())
    }

    /// Print progress bar
    pub fn print_progress(&mut self, stats: &DownloadStats, total: u64) -> io::Result<()> {
        if self.quiet {
            return Ok(());
        }

        // Clear the current line
        print!("\r\x1b[2K");

        // Calculate progress percentage
        let progress_percent = stats.progress * 100.0;

        // Build the progress bar
        let bar_width: usize = 40;
        let filled = (progress_percent / 100.0 * bar_width as f64) as usize;
        let empty = bar_width.saturating_sub(filled);

        let bar: String = "=".repeat(filled) + &" ".repeat(empty);

        // Calculate ETA
        let eta = DownloadStats::calculate_eta(stats.downloaded, total, stats.download_speed);
        let eta_str = eta
            .map(|e| DownloadStats::format_duration(e))
            .unwrap_or_else(|| "∞".to_string());

        // Format the progress line
        let line = format!(
            "[{}] {:.1}% | {} / {} | ↓ {} | ↑ {} | Peers: {} | ETA: {}",
            bar,
            progress_percent,
            DownloadStats::format_bytes(stats.downloaded),
            DownloadStats::format_bytes(total),
            DownloadStats::format_speed(stats.download_speed),
            DownloadStats::format_speed(stats.upload_speed),
            stats.peers,
            eta_str,
        );

        self.prev_line_len = line.len();
        print!("{}", line);

        Ok(())
    }

    /// Print download statistics
    pub fn print_stats(&self, stats: &DownloadStats, total: u64) -> io::Result<()> {
        if self.quiet {
            return Ok(());
        }

        println!();
        println!("Download Statistics:");
        println!("  Downloaded: {} / {} ({:.1}%)",
            DownloadStats::format_bytes(stats.downloaded),
            DownloadStats::format_bytes(total),
            stats.progress * 100.0
        );
        println!("  Uploaded: {}", DownloadStats::format_bytes(stats.uploaded));
        println!("  Download Speed: {}", DownloadStats::format_speed(stats.download_speed));
        println!("  Upload Speed: {}", DownloadStats::format_speed(stats.upload_speed));
        println!("  Connected Peers: {}", stats.peers);
        println!("  Elapsed Time: {}", DownloadStats::format_duration(self.start_time.elapsed()));

        Ok(())
    }

    /// Print peer information
    pub fn print_peer_info(&self, peer_count: usize, active_peers: usize) -> io::Result<()> {
        if self.quiet {
            return Ok(());
        }

        println!();
        println!("Peer Information:");
        println!("  Total Peers: {}", peer_count);
        println!("  Active Peers: {}", active_peers);

        Ok(())
    }

    /// Print completion message
    pub fn print_complete(&self, stats: &DownloadStats, total: u64) -> io::Result<()> {
        if self.quiet {
            return Ok(());
        }

        // Clear the progress line
        print!("\r\x1b[2K");
        io::stdout().flush()?;

        println!();
        println!("Download Complete!");
        println!("  Downloaded: {} / {}",
            DownloadStats::format_bytes(stats.downloaded),
            DownloadStats::format_bytes(total)
        );
        println!("  Uploaded: {}", DownloadStats::format_bytes(stats.uploaded));
        println!("  Elapsed Time: {}", DownloadStats::format_duration(self.start_time.elapsed()));

        Ok(())
    }

    /// Print a status message
    pub fn print_status(&self, message: &str) -> io::Result<()> {
        if self.quiet {
            return Ok(());
        }

        println!("\r\x1b[2K{}", message);
        Ok(())
    }

    /// Print an error message
    pub fn print_error(&self, message: &str) -> io::Result<()> {
        eprintln!("\r\x1b[2KError: {}", message);
        Ok(())
    }

    /// Print an info message
    pub fn print_info(&self, message: &str) -> io::Result<()> {
        println!("\r\x1b[2KInfo: {}", message);
        Ok(())
    }

    /// Get the elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
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
    fn test_format_bytes() {
        assert_eq!(DownloadStats::format_bytes(0), "0.00 B");
        assert_eq!(DownloadStats::format_bytes(1024), "1.00 KB");
        assert_eq!(DownloadStats::format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(DownloadStats::format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_format_speed() {
        assert_eq!(DownloadStats::format_speed(1024.0), "1.00 KB/s");
        assert_eq!(DownloadStats::format_speed(1024.0 * 1024.0), "1.00 MB/s");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(DownloadStats::format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(DownloadStats::format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(DownloadStats::format_duration(Duration::from_secs(3661)), "1h 1m 1s");
    }

    #[test]
    fn test_calculate_eta() {
        // Normal case
        let eta = DownloadStats::calculate_eta(50, 100, 10.0);
        assert_eq!(eta, Some(Duration::from_secs(5)));

        // Already complete
        let eta = DownloadStats::calculate_eta(100, 100, 10.0);
        assert_eq!(eta, None);

        // Zero speed
        let eta = DownloadStats::calculate_eta(50, 100, 0.0);
        assert_eq!(eta, None);
    }

    #[test]
    fn test_download_stats_default() {
        let stats = DownloadStats::default();
        assert_eq!(stats.downloaded, 0);
        assert_eq!(stats.uploaded, 0);
        assert_eq!(stats.download_speed, 0.0);
        assert_eq!(stats.upload_speed, 0.0);
        assert_eq!(stats.peers, 0);
        assert_eq!(stats.progress, 0.0);
    }

    #[test]
    fn test_progress_display_new() {
        let display = ProgressDisplay::new(false);
        assert!(!display.is_quiet());
        assert_eq!(display.elapsed().as_secs(), 0);
    }

    #[test]
    fn test_progress_display_quiet() {
        let display = ProgressDisplay::new(true);
        assert!(display.is_quiet());
    }

    #[test]
    fn test_progress_display_with_interval() {
        let display = ProgressDisplay::with_interval(false, Duration::from_secs(2));
        assert!(!display.is_quiet());
    }
}
