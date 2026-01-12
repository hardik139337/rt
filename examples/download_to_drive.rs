//! Download torrent from web seed directly to Google Drive
//!
//! This downloads files from a web seed (HTTP source) and streams
//! them directly to Google Drive without storing locally.

use std::sync::Arc;
use rust_torrent_downloader::torrent::{MagnetParser, TorrentParser, TorrentFile};

#[cfg(feature = "gdrive")]
use rust_torrent_downloader::storage::DriveClient;

const GOOGLE_ACCESS_TOKEN: &str = "YOUR_ACCESS_TOKEN";

/// Download a file from web seed and stream to Google Drive
async fn download_and_upload_file(
    drive_client: &DriveClient,
    folder_id: &str,
    file: &TorrentFile,
    base_url: &str,
) -> anyhow::Result<()> {
    let filename = file.path.join("/");
    let file_size = file.length;

    println!("Downloading: {} ({} bytes)", filename, file_size);

    // Construct the file URL from web seed base URL
    let file_url = format!("{}/{}", base_url.trim_end_matches('/'), filename);
    println!("  URL: {}", file_url);

    // Create resumable upload session
    let upload_url = drive_client
        .create_resumable_upload(&filename, "application/octet-stream", Some(folder_id))
        .await?;

    // Start HTTP download
    let response = reqwest::Client::new()
        .get(&file_url)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download file: HTTP {}", response.status());
    }

    let content_length = response.content_length().unwrap_or(file_size);
    println!("  Content-Length: {} bytes", content_length);

    // Download entire file in chunks and upload to Drive
    let mut uploaded_bytes = 0u64;
    let chunk_size = 256 * 1024; // 256KB chunks

    loop {
        let range = format!("bytes={}-{}", uploaded_bytes,
            std::cmp::min(uploaded_bytes + chunk_size as u64 - 1, content_length - 1));

        let chunk_response = reqwest::Client::new()
            .get(&file_url)
            .header("Range", range)
            .send()
            .await?;

        if !chunk_response.status().is_success() &&
            chunk_response.status().as_u16() != 206 {
            anyhow::bail!("Failed to download chunk: HTTP {}", chunk_response.status());
        }

        let chunk = chunk_response.bytes().await?;
        if chunk.is_empty() {
            break;
        }

        let chunk_len = chunk.len();

        // Upload this chunk to Drive
        drive_client
            .upload_chunk(&upload_url, chunk.clone(), uploaded_bytes, Some(content_length))
            .await?;

        uploaded_bytes += chunk_len as u64;

        let progress = (uploaded_bytes as f64 / content_length as f64) * 100.0;
        print!("  Progress: {:.1}%\r", progress);
        use std::io::Write;
        let _ = std::io::stdout().flush();

        if uploaded_bytes >= content_length {
            break;
        }
    }

    println!("  ✓ Complete: {} bytes", uploaded_bytes);
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <magnet-link-with-web-seed>", args[0]);
        eprintln!();
        eprintln!("Example magnet link with web seed:");
        eprintln!("magnet:?xt=urn:btih:...&ws=http://example.com/torrents/");
        std::process::exit(1);
    }

    let magnet_uri = &args[1];

    println!("=== Parsing Magnet Link ===");
    let magnet_info = MagnetParser::parse(magnet_uri)?;

    println!("Display Name: {}", magnet_info.display_name.as_deref().unwrap_or("(none)"));

    // Try to get torrent from exact source first
    let torrent_info = if !magnet_info.exact_sources.is_empty() {
        println!("\n=== Fetching Torrent from Exact Source ===");
        let torrent_url = &magnet_info.exact_sources[0];
        println!("Torrent URL: {}", torrent_url);

        let response = reqwest::get(torrent_url).await?;
        if !response.status().is_success() {
            anyhow::bail!("Failed to download torrent: HTTP {}", response.status());
        }

        let torrent_bytes = response.bytes().await?;
        Arc::new(TorrentParser::parse_bytes(&torrent_bytes)?)
    } else {
        anyhow::bail!("Magnet link must have exact source (xs parameter)");
    };

    println!("\n=== Torrent Info ===");
    println!("Name: {}", torrent_info.name);
    println!("Size: {:.2} MB", torrent_info.total_size() as f64 / (1024.0 * 1024.0));

    // Get web seed URL from magnet link
    let web_seed_url = magnet_info.web_seeds.first()
        .ok_or_else(|| anyhow::anyhow!("No web seed (ws) in magnet link"))?;

    println!("\n=== Web Seed URL ===");
    println!("Base URL: {}", web_seed_url);

    // Construct the torrent folder URL
    // For webtorrent.io, files are at base_url + torrent_name
    let base_url = format!("{}/{}/", web_seed_url.trim_end_matches('/'), torrent_info.name);
    println!("Files URL: {}", base_url);

    // Initialize Google Drive
    println!("\n=== Initializing Google Drive ===");

    let access_token = std::env::var("GOOGLE_ACCESS_TOKEN")
        .unwrap_or_else(|_| GOOGLE_ACCESS_TOKEN.to_string());

    let drive_client = DriveClient::new(&access_token);

    println!("Checking authentication...");
    if !drive_client.check_auth().await? {
        anyhow::bail!("Authentication failed! Check your GOOGLE_ACCESS_TOKEN");
    }
    println!("✓ Authenticated");

    println!("Creating folder: {}", torrent_info.name);
    let folder_id = drive_client.create_folder(&torrent_info.name, None).await?;
    println!("✓ Folder created: {}", folder_id);

    // Download each file from web seed and upload to Drive
    println!("\n=== Downloading Files to Drive ===");

    let files: Vec<TorrentFile> = torrent_info.files_iter().collect();

    for file in &files {
        match download_and_upload_file(&drive_client, &folder_id, file, &base_url).await {
            Ok(_) => {},
            Err(e) => {
                eprintln!("  ✗ Failed: {}", e);
                // Continue with next file
            }
        }
    }

    println!("\n=== Complete ===");
    println!("All files uploaded to Google Drive folder: {}", folder_id);
    println!("\nCheck your Google Drive!");

    Ok(())
}
