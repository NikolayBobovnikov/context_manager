use std::path::PathBuf;
use crate::file_handler::FileNode;
use crate::error::AppError;

/// Events sent from background threads to the main UI thread
#[derive(Debug)]
pub enum AppEvent {
    /// Directory scan completed
    DirectoryScanComplete(Result<FileNode, AppError>),
    /// File modified and debounced
    FileModifiedDebounced(PathBuf),
    /// Document generation completed (renamed)
    DocumentGenerationComplete(Result<(), AppError>),
    /// Partial document update completed (renamed)
    PartialDocumentUpdateComplete(Result<(), AppError>),
    /// File watcher encountered an error
    #[allow(dead_code)]
    WatcherError(AppError),
    /// Status message to display to user
    #[allow(dead_code)]
    StatusMessage(String),
    /// Error message to display to user
    #[allow(dead_code)]
    ErrorMessage(String),
} 