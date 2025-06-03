use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO Error: {details} (Path: {path:?})")]
    Io {
        source: std::io::Error,
        path: Option<PathBuf>, // Path associated with the IO error, if any
        details: String,       // Contextual information about the operation
    },
    #[error("File Watcher Error (notify): {0}")]
    Notify(#[from] notify::Error),
    #[error("Error building ignore rules (ignore crate): {0}")]
    IgnoreBuild(#[from] ignore::Error),
    #[error("Invalid directory selected: {0}")]
    InvalidDirectory(String),
    #[error("File path not found: {0}")]
    PathNotFound(PathBuf),
    #[error("Failed to strip prefix '{prefix:?}' from path '{path:?}'")]
    StripPrefixError { prefix: PathBuf, path: PathBuf },
    /// For channel send errors in multi-threaded scenarios
    #[allow(dead_code)]
    #[error("Channel send error: {0}")]
    ChannelSend(String), // For mpsc send errors, with context
    #[error("Markdown generation error: {0}")]
    MarkdownGeneration(String),
    /// Generic operation failure
    #[allow(dead_code)]
    #[error("Operation failed: {0}")]
    OperationFailed(String), // Generic failure
    #[allow(dead_code)]
    #[error("Error handling non-UTF8 content for file {path:?}: {details}")]
    NonUtf8Content { path: PathBuf, details: String },
    #[error("Permissions error accessing {path:?}: {details}")]
    PermissionsError { path: PathBuf, details: String },
    #[error("Failed to create or persist temporary file for atomic write at {path:?}: {details}")]
    AtomicWriteError { path: PathBuf, details: String },
    /// Symlink handling errors
    #[allow(dead_code)]
    #[error("Symlink error for {path:?}: {details}")]
    SymlinkError { path: PathBuf, details: String },
}

// Helper constructor for detailed IO errors
impl AppError {
    pub fn new_io_error(source: std::io::Error, path: Option<PathBuf>, details: String) -> Self {
        AppError::Io { source, path, details }
    }
}

pub type Result<T, E = AppError> = std::result::Result<T, E>; 