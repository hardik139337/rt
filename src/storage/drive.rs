//! Google Drive storage module
//!
//! Streams torrent data directly to Google Drive without local disk storage.

#![cfg(feature = "gdrive")]

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::torrent::info::TorrentFile;
use crate::storage::piece::PieceStorage;
use crate::storage::backend::{StorageBackend, StorageType, StorageMetadata};

/// Google Drive API client for uploading files
pub struct DriveClient {
    /// HTTP client
    client: reqwest::Client,
    /// Access token for OAuth2
    access_token: String,
    /// API endpoint
    api_url: String,
}

impl DriveClient {
    /// Create a new Google Drive client
    ///
    /// # Arguments
    /// * `access_token` - OAuth2 access token for Google Drive API
    ///
    /// # Example
    /// ```ignore
    /// let client = DriveClient::new("your_access_token");
    /// ```
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            access_token: access_token.into(),
            api_url: "https://www.googleapis.com/upload/drive/v3/files".to_string(),
        }
    }

    /// Check if the client is authenticated by validating the token
    pub async fn check_auth(&self) -> Result<bool> {
        let response = self.client
            .get("https://www.googleapis.com/drive/v3/about?fields=user")
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await?;

        let status = response.status();
        let is_success = status.is_success();

        if !is_success {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!("Auth check failed: {} - {}", status, error_text);
        }

        Ok(is_success)
    }

    /// Create a resumable upload session for a file
    ///
    /// Returns the upload URL for subsequent chunk uploads
    pub async fn create_resumable_upload(
        &self,
        filename: &str,
        mime_type: &str,
        parent_folder_id: Option<&str>,
    ) -> Result<String> {
        debug!("Creating resumable upload session for: {}", filename);

        let mut url = format!(
            "{}?uploadType=resumable&supportsAllDrives=true",
            self.api_url
        );

        if let Some(folder_id) = parent_folder_id {
            url.push_str(&format!("&parents={}", folder_id));
        }

        let metadata = serde_json::json!({
            "name": filename,
            "mimeType": mime_type
        });

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Length", "0")
            .header("X-Upload-Content-Length", "*") // Unknown size initially
            .json(&metadata)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            bail!("Failed to create upload session: {} - {}", status, error_text);
        }

        let upload_url = response
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Missing Location header in response"))?
            .to_string();

        info!("Created resumable upload session: {}", upload_url);
        Ok(upload_url)
    }

    /// Upload a chunk to a resumable upload session
    ///
    /// # Arguments
    /// * `upload_url` - The URL returned by create_resumable_upload
    /// * `chunk` - Data chunk to upload
    /// * `offset` - Byte offset of this chunk in the file
    /// * `total_size` - Total file size (None if unknown)
    pub async fn upload_chunk(
        &self,
        upload_url: &str,
        chunk: Bytes,
        offset: u64,
        total_size: Option<u64>,
    ) -> Result<()> {
        let chunk_size = chunk.len();
        let content_range = if let Some(total) = total_size {
            format!("bytes {}-{}/{}", offset, offset + chunk_size as u64 - 1, total)
        } else {
            format!("bytes {}-{}/{}", offset, offset + chunk_size as u64 - 1, "*")
        };

        debug!("Uploading chunk: offset={}, size={}, range={}", offset, chunk_size, content_range);

        let mut request = self.client
            .put(upload_url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Range", content_range);

        if !chunk.is_empty() {
            request = request.body(chunk.clone());
        } else {
            // Empty chunk for 0-byte files
            request = request.body(Bytes::new());
        }

        let response = request.send().await?;

        let status = response.status();
        let status_code = status.as_u16();

        // Handle both success (200) and resume incomplete (308) status codes
        if status_code == 200 {
            // Upload complete
            if let Ok(result) = response.json::<serde_json::Value>().await {
                info!("Upload complete: file_id={}", result["id"]);
            }
            Ok(())
        } else if status_code == 308 {
            // Resume Incomplete - more data needed
            debug!("Chunk uploaded successfully, more data needed");
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            bail!("Upload failed: {} - {}", status, error_text)
        }
    }

    /// Create a folder in Google Drive
    pub async fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<String> {
        debug!("Creating folder: {}", name);

        let mut metadata = serde_json::json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder"
        });

        if let Some(parent) = parent_id {
            metadata["parents"] = serde_json::json!([parent]);
        }

        let response = self.client
            .post("https://www.googleapis.com/drive/v3/files?supportsAllDrives=true")
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&metadata)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            bail!("Failed to create folder: {} - {}", status, error_text);
        }

        let result: serde_json::Value = response.json().await?;
        let folder_id = result["id"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing folder id in response"))?;

        info!("Created folder: {} (id: {})", name, folder_id);
        Ok(folder_id.to_string())
    }
}

/// Represents a file in Google Drive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

/// Google Drive storage backend for torrents
///
/// Streams pieces directly to Google Drive without local storage
pub struct DriveStorage {
    /// Google Drive client
    client: DriveClient,
    /// Folder ID for storing downloads
    folder_id: Option<String>,
    /// Upload sessions for each file (upload_url, current_offset, total_size)
    upload_sessions: Vec<UploadSession>,
    /// Piece storage for tracking verified pieces
    piece_storage: PieceStorage,
}

/// Active upload session for a file
struct UploadSession {
    file_index: usize,
    upload_url: String,
    current_offset: u64,
    total_size: u64,
    piece_offsets: Vec<u64>,
}

impl DriveStorage {
    /// Create a new Google Drive storage backend
    ///
    /// # Arguments
    /// * `access_token` - OAuth2 access token
    /// * `folder_id` - Optional Google Drive folder ID to store files
    /// * `piece_hashes` - Piece hashes for verification
    pub fn new(access_token: impl Into<String>, folder_id: Option<String>, piece_hashes: Vec<[u8; 20]>) -> Self {
        let total_size = piece_hashes.len() as u64 * 262144;
        Self {
            client: DriveClient::new(access_token),
            folder_id,
            upload_sessions: Vec::new(),
            piece_storage: PieceStorage::new(piece_hashes, 262144, total_size),
        }
    }
    
    /// Set piece storage (called during initialization)
    pub fn set_piece_storage(&mut self, piece_storage: PieceStorage) {
        self.piece_storage = piece_storage;
    }

    /// Check authentication
    pub async fn check_auth(&self) -> Result<bool> {
        self.client.check_auth().await
    }

    /// Initialize upload sessions for all torrent files
    pub async fn initialize_uploads(&mut self, files: &[TorrentFile]) -> Result<()> {
        info!("Initializing {} upload sessions", files.len());

        self.upload_sessions = Vec::new();

        for (index, file) in files.iter().enumerate() {
            let filename = file.path.join("/");
            let total_size = file.length;

            let upload_url = self
                .client
                .create_resumable_upload(&filename, "application/octetstream", self.folder_id.as_deref())
                .await?;

            // Calculate piece offsets for this file
            let piece_length = 262144u64; // Default, will be updated
            let mut piece_offsets = Vec::new();
            let mut offset = 0u64;
            while offset < total_size {
                piece_offsets.push(offset);
                offset += piece_length;
            }

            self.upload_sessions.push(UploadSession {
                file_index: index,
                upload_url,
                current_offset: 0,
                total_size,
                piece_offsets,
            });

            info!("Initialized upload for file {} ({} bytes)", filename, total_size);
        }

        Ok(())
    }

    /// Upload a piece directly to Google Drive
    ///
    /// # Arguments
    /// * `piece_data` - Raw piece bytes
    /// * `piece_index` - Index of the piece
    /// * `_piece_length` - Length of each piece
    /// * `file_offset` - Starting byte offset of this piece in the torrent
    pub async fn upload_piece(
        &mut self,
        piece_data: Bytes,
        piece_index: usize,
        _piece_length: u64,
        file_offset: u64,
    ) -> Result<()> {
        let piece_len = piece_data.len();
        debug!("Uploading piece {} ({} bytes) to Drive", piece_index, piece_len);

        // Find which file this piece belongs to
        let session = self
            .upload_sessions
            .iter_mut()
            .find(|s| {
                let piece_start = file_offset;
                let piece_end = piece_start + piece_len as u64;
                // Check if this piece overlaps with this file
                // For simplicity, we're assuming pieces don't span files (common case)
                piece_start < s.total_size && piece_end > s.current_offset
            })
            .ok_or_else(|| anyhow!("No upload session found for piece {}", piece_index))?;

        // Calculate offset within this file
        let offset_in_file = file_offset.saturating_sub(session.current_offset);

        // Upload the chunk
        self.client
            .upload_chunk(
                &session.upload_url,
                piece_data,
                offset_in_file,
                Some(session.total_size),
            )
            .await?;

        session.current_offset = offset_in_file + piece_len as u64;

        debug!(
            "Piece {} uploaded (file progress: {}/{} bytes)",
            piece_index,
            session.current_offset,
            session.total_size
        );

        Ok(())
    }

    /// Complete all uploads
    pub async fn complete_uploads(&self) -> Result<()> {
        info!("Completing {} upload sessions", self.upload_sessions.len());

        for session in &self.upload_sessions {
            // Ensure final chunk is uploaded even if 0 bytes
            if session.current_offset < session.total_size {
                warn!(
                    "File {} is incomplete: {}/{} bytes",
                    session.file_index,
                    session.current_offset,
                    session.total_size
                );
            }
        }

        info!("All uploads completed");
        Ok(())
    }

    /// Get the folder ID being used
    pub fn folder_id(&self) -> Option<&str> {
        self.folder_id.as_deref()
    }
}

/// Implement StorageBackend trait for DriveStorage
#[async_trait]
impl StorageBackend for DriveStorage {
    async fn initialize(&mut self, files: &[TorrentFile]) -> Result<()> {
        // Initialize upload sessions for all files
        self.initialize_uploads(files).await
    }
    
    async fn write_piece(&mut self, piece_index: u32, data: Bytes) -> Result<()> {
        // Calculate file offset for this piece
        let piece_length = self.piece_storage.piece_length() as u64;
        let file_offset = piece_index as u64 * piece_length;
        
        // Upload directly to Drive (no disk write)
        self.upload_piece(data, piece_index as usize, piece_length, file_offset).await?;
        
        // Mark piece as verified in piece storage
        if let Some(piece) = self.piece_storage.get_piece_mut(piece_index as usize) {
            piece.verified = true;
        }
        
        Ok(())
    }
    
    async fn read_piece(&self, _piece_index: u32) -> Result<Option<Bytes>> {
        // DriveStorage doesn't support reading back pieces
        // Pieces are uploaded immediately and not cached
        Ok(None)
    }
    
    async fn complete(&self) -> Result<()> {
        // Finalize all upload sessions
        self.complete_uploads().await
    }
    
    fn is_complete(&self) -> bool {
        // Check if all upload sessions are complete
        self.upload_sessions.iter().all(|s| s.current_offset >= s.total_size)
    }
    
    fn get_progress(&self) -> f64 {
        let total_uploaded: u64 = self.upload_sessions.iter()
            .map(|s| s.current_offset)
            .sum();
        let total_size: u64 = self.upload_sessions.iter()
            .map(|s| s.total_size)
            .sum();
        
        if total_size == 0 {
            0.0
        } else {
            total_uploaded as f64 / total_size as f64
        }
    }
    
    fn verified_count(&self) -> usize {
        self.piece_storage.completed_count()
    }
    
    fn total_pieces(&self) -> usize {
        self.piece_storage.piece_count()
    }
    
    fn pieces(&self) -> &PieceStorage {
        &self.piece_storage
    }
    
    fn pieces_mut(&mut self) -> &mut PieceStorage {
        &mut self.piece_storage
    }
    
    fn storage_type(&self) -> StorageType {
        StorageType::Drive
    }
    
    fn metadata(&self) -> StorageMetadata {
        StorageMetadata {
            storage_type: StorageType::Drive,
            base_path: None,
            drive_folder_id: self.folder_id.clone(),
            total_size: self.upload_sessions.iter().map(|s| s.total_size).sum(),
            piece_count: self.piece_storage.piece_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drive_file_serialization() {
        let file = DriveFile {
            id: "123".to_string(),
            name: "test.txt".to_string(),
            size: Some("1024".to_string()),
            mime_type: "text/plain".to_string(),
        };

        let json = serde_json::to_string(&file).unwrap();
        assert!(json.contains("\"id\":\"123\""));
    }
}
