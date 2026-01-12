//! Google Drive torrent download example
//!
//! This example demonstrates how to download torrents directly to Google Drive
//! without storing any data on local disk.
//!
//! # Setup
//!
//! 1. Create a Google Cloud Project
//! 2. Enable the Google Drive API
//! 3. Create OAuth 2.0 credentials
//! 4. Run: `gcloud auth application-default login`
//!
//! Or get tokens from: https://developers.google.com/oauthplayground/
//!
//! # Run
//!
//! ```bash
//! export GOOGLE_ACCESS_TOKEN="your_access_token"
//! export GOOGLE_REFRESH_TOKEN="your_refresh_token"
//! export GOOGLE_CLIENT_ID="your_client_id"
//! export GOOGLE_CLIENT_SECRET="your_client_secret"
//! cargo run --example gdrive_download --features gdrive -- <torrent-file-or-magnet-link>
//!
//! # Or set the tokens directly in this file (see constants below)
//! ```

use std::env;
use std::sync::Arc;
use rust_torrent_downloader::torrent::{TorrentParser, MagnetParser, TorrentFile};
use rust_torrent_downloader::storage::{DriveStorage, DriveClient};

/// Google Drive OAuth2 credentials
///
/// Get your credentials from: https://developers.google.com/oauthplayground/
/// Steps:
/// 1. Open the URL above
/// 2. Click "OAuth 2.0 Configuration"
/// 3. Select "Drive API v3"
/// 4. Click "Authorize APIs"
/// 5. Copy the access token and refresh token
const GOOGLE_ACCESS_TOKEN: &str = "YOUR_ACCESS_TOKEN";
const GOOGLE_REFRESH_TOKEN: &str = "YOUR_REFRESH_TOKEN";
const GOOGLE_CLIENT_ID: &str = "";
const GOOGLE_CLIENT_SECRET: &str = "";

/// Refresh an expired access token using refresh token
async fn refresh_access_token(
    refresh_token: &str,
    client_id: &str,
    client_secret: &str,
) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let params = [
        ("refresh_token", refresh_token),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "refresh_token"),
    ];

    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to refresh token: HTTP {}", response.status());
    }

    let json: serde_json::Value = response.json().await?;
    let new_token = json["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No access_token in response"))?;

    Ok(new_token.to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Get credentials from environment or use constants
    let access_token = env::var("GOOGLE_ACCESS_TOKEN")
        .unwrap_or_else(|_| GOOGLE_ACCESS_TOKEN.to_string());
    let refresh_token = env::var("GOOGLE_REFRESH_TOKEN")
        .unwrap_or_else(|_| {
            if GOOGLE_REFRESH_TOKEN.is_empty() {
                String::new()
            } else {
                GOOGLE_REFRESH_TOKEN.to_string()
            }
        });
    let client_id = env::var("GOOGLE_CLIENT_ID")
        .unwrap_or_else(|_| {
            if GOOGLE_CLIENT_ID.is_empty() {
                String::new()
            } else {
                GOOGLE_CLIENT_ID.to_string()
            }
        });
    let client_secret = env::var("GOOGLE_CLIENT_SECRET")
        .unwrap_or_else(|_| {
            if GOOGLE_CLIENT_SECRET.is_empty() {
                String::new()
            } else {
                GOOGLE_CLIENT_SECRET.to_string()
            }
        });

    // Get torrent file or magnet link
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <torrent-file-or-magnet-link>", args[0]);
        eprintln!();
        eprintln!("Google Drive credentials:");
        eprintln!("  - Access token (GOOGLE_ACCESS_TOKEN)");
        eprintln!("  - Refresh token (GOOGLE_REFRESH_TOKEN) - for automatic refresh");
        eprintln!("  - Client ID (GOOGLE_CLIENT_ID) - required for refresh");
        eprintln!("  - Client Secret (GOOGLE_CLIENT_SECRET) - required for refresh");
        eprintln!();
        eprintln!("Or set them in the code constants.");
        eprintln!();
        eprintln!("To get credentials:");
        eprintln!("1. Open https://developers.google.com/oauthplayground/");
        eprintln!("2. Click 'OAuth 2.0 Configuration'");
        eprintln!("3. Select 'Drive API v3'");
        eprintln!("4. Click 'Authorize APIs'");
        eprintln!("5. Copy the access token and refresh token");
        std::process::exit(1);
    }

    let input = &args[1];

    // Parse torrent info
    let torrent_info = if MagnetParser::is_magnet_link(input) {
        handle_magnet_link(input).await?
    } else {
        Arc::new(TorrentParser::parse_file(std::path::Path::new(input))?)
    };

    println!("\n=== Torrent Information ===");
    println!("Name: {}", torrent_info.name);
    println!("Size: {} bytes ({:.2} MB)",
        torrent_info.total_size(),
        torrent_info.total_size() as f64 / (1024.0 * 1024.0)
    );
    println!("Pieces: {}", torrent_info.piece_count());

    println!("\n=== Files ===");
    for file in torrent_info.files_iter() {
        let path = file.path.join("/");
        let size_mb = file.length as f64 / (1024.0 * 1024.0);
        println!("  {} ({:.2} MB)", path, size_mb);
    }

    // Initialize Google Drive storage
    println!("\n=== Initializing Google Drive Storage ===");

    // Create a DriveClient first
    let mut drive_client = DriveClient::new(&access_token);

    // Check authentication
    println!("Checking Google Drive authentication...");
    let mut current_token = access_token;

    // Try auth, if fails and we have refresh token, try refreshing
    if !drive_client.check_auth().await? {
        if !refresh_token.is_empty() && !client_id.is_empty() && !client_secret.is_empty() {
            println!("Access token expired, refreshing...");
            current_token = refresh_access_token(&refresh_token, &client_id, &client_secret).await?;
            println!("✓ Token refreshed");
            drive_client = DriveClient::new(&current_token);

            if !drive_client.check_auth().await? {
                anyhow::bail!("Authentication failed! Check your credentials");
            }
        } else {
            anyhow::bail!("Authentication failed! Check your GOOGLE_ACCESS_TOKEN");
        }
    }
    println!("✓ Authentication successful");

    // Create a folder for this torrent
    println!("Creating folder: {}", torrent_info.name);
    let folder_id = drive_client.create_folder(&torrent_info.name, None).await?;
    println!("✓ Folder created: {}", folder_id);

    // Re-initialize with folder
    let mut drive_storage = DriveStorage::new(&current_token, Some(folder_id.clone()));

    // Get files from torrent info
    let files: Vec<TorrentFile> = torrent_info.files_iter().map(|f| f.clone()).collect();

    // Initialize upload sessions for all files
    println!("\n=== Initializing Upload Sessions ===");
    drive_storage.initialize_uploads(&files).await?;
    println!("✓ Initialized {} upload sessions", files.len());

    // Simulate downloading and uploading pieces
    // In a real implementation, you would:
    // 1. Connect to peers via BitTorrent protocol
    // 2. Download piece by piece
    // 3. Upload each piece to Google Drive as it arrives

    println!("\n=== Piece Upload Demo ===");
    println!("This example demonstrates the Google Drive upload capability.");
    println!("In a full implementation, pieces would be:");
    println!("  1. Downloaded from BitTorrent peers");
    println!("  2. Immediately uploaded to Google Drive");
    println!("  3. Never written to local disk");
    println!();
    println!("Upload sessions created for:");
    for (i, file) in files.iter().enumerate() {
        let path = file.path.join("/");
        println!("  {}. {} ({} bytes)", i + 1, path, file.length);
    }

    println!("\n=== Setup Complete ===");
    println!("Your torrent '{}' is ready for download to Google Drive!", torrent_info.name);
    println!("Files will be stored in folder: {}", folder_id);
    println!();
    println!("Note: Full peer-to-peer download would require:");
    println!("  - Connecting to DHT/trackers to find peers");
    println!("  - Performing BitTorrent handshake");
    println!("  - Downloading pieces from peers");
    println!("  - Uploading each piece to Drive as it arrives");

    Ok(())
}

/// Handle magnet link by fetching torrent from exact source
async fn handle_magnet_link(
    magnet_uri: &str
) -> anyhow::Result<Arc<rust_torrent_downloader::torrent::TorrentInfo>> {
    println!("=== Parsing Magnet Link ===");
    let magnet_info = MagnetParser::parse(magnet_uri)?;

    println!("Info Hash: {}", hex::encode(magnet_info.info_hash));
    println!("Display Name: {}", magnet_info.display_name.as_deref().unwrap_or("(none)"));

    if !magnet_info.exact_sources.is_empty() {
        println!("\n=== Fetching Torrent from Exact Source ===");
        let torrent_url = &magnet_info.exact_sources[0];
        println!("Torrent URL: {}", torrent_url);

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

        let torrent_info = TorrentParser::parse_bytes(&torrent_bytes)?;
        println!("Successfully parsed torrent: {}", torrent_info.name);

        return Ok(Arc::new(torrent_info));
    }

    println!("\n=== No Exact Source Available ===");
    println!("This magnet link requires DHT metadata exchange.");
    println!("Please use a magnet link with an exact source (xs parameter)");
    println!("or provide a .torrent file directly.");

    anyhow::bail!("Magnet link without exact source is not yet supported");
}
