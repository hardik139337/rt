//! Storage module
//!
//! Handles file storage, resume capability, and download management.

pub mod piece;
pub mod file;
pub mod resume;
pub mod download;

// Re-export piece types
pub use piece::{Piece, Block, PieceStorage, PieceStatus};

// Re-export file storage types
pub use file::{FileStorage, FileEntry, ResumeData as FileResumeData, PieceState as FilePieceState};

// Re-export resume types
pub use resume::{ResumeData, PieceState, ResumeManager};

// Re-export download types
pub use download::{DownloadManager, PieceDownload, DownloadStats};
