//! Storage module
//!
//! Handles file storage, resume capability, and download management.

pub mod backend;
pub mod piece;
pub mod file;
pub mod resume;
pub mod download;

#[cfg(feature = "gdrive")]
pub mod drive;

// Re-export backend types
pub use backend::{StorageBackend, StorageType, StorageMetadata};

// Re-export piece types
pub use piece::{Piece, Block, PieceStorage, PieceStatus};

// Re-export file storage types
pub use file::{FileStorage, FileEntry, ResumeData as FileResumeData, PieceState as FilePieceState};

// Re-export resume types
pub use resume::{ResumeData, PieceState, ResumeManager};

// Re-export download types
pub use download::{DownloadManager, PieceDownload, DownloadStats, FileDownloadManager};

// Re-export drive types
#[cfg(feature = "gdrive")]
pub use drive::{DriveClient, DriveStorage, DriveFile};

// Re-export DriveDownloadManager type alias
#[cfg(feature = "gdrive")]
pub use download::DriveDownloadManager;
