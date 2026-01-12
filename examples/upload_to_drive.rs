//! Simple example to upload existing files to Google Drive
//!
//! This tests the Google Drive upload functionality by uploading
//! files that already exist locally.

use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[cfg(feature = "gdrive")]
use rust_torrent_downloader::storage::DriveClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Get access token from environment
    let access_token = std::env::var("GOOGLE_ACCESS_TOKEN")
        .expect("GOOGLE_ACCESS_TOKEN environment variable not set");

    println!("=== Google Drive Upload Test ===\n");

    // Create Drive client
    let drive_client = DriveClient::new(&access_token);

    // Check authentication
    println!("Checking Google Drive authentication...");
    if !drive_client.check_auth().await? {
        anyhow::bail!("Authentication failed");
    }
    println!("✓ Authentication successful\n");

    // Create a test folder
    println!("Creating test folder...");
    let folder_name = "Test Upload";
    let folder_id = drive_client.create_folder(folder_name, None).await?;
    println!("✓ Folder created: {} (id: {})\n", folder_name, folder_id);

    // Files to upload from downloads folder
    let files_to_upload = vec![
        ("downloads/Big Buck Bunny.en.srt", "Big Buck Bunny.en.srt", "text/plain"),
        ("downloads/poster.jpg", "poster.jpg", "image/jpeg"),
    ];

    for (local_path, remote_name, mime_type) in files_to_upload {
        println!("Uploading: {}", remote_name);

        // Check if file exists
        if !Path::new(local_path).exists() {
            println!("  ✗ File not found: {}\n", local_path);
            continue;
        }

        // Read file
        let mut file = File::open(local_path).await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len();

        println!("  Size: {} bytes", file_size);

        // Create resumable upload session
        let upload_url = drive_client
            .create_resumable_upload(remote_name, mime_type, Some(&folder_id))
            .await?;

        // Read and upload file in chunks
        let mut buffer = vec![0u8; 256 * 1024]; // 256KB chunks
        let mut offset = 0u64;
        let mut uploaded = 0u64;

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            let chunk = bytes::Bytes::copy_from_slice(&buffer[..n]);
            drive_client
                .upload_chunk(&upload_url, chunk, offset, Some(file_size))
                .await?;

            offset += n as u64;
            uploaded += n as u64;

            let progress = (uploaded as f64 / file_size as f64) * 100.0;
            print!("  Progress: {:.1}%\r", progress);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }

        println!("  ✓ Upload complete: {} bytes\n", uploaded);
    }

    println!("=== Upload Test Complete ===");
    println!("\nCheck your Google Drive for folder: {}", folder_name);

    Ok(())
}
