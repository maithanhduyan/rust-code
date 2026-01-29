//! Error types cho core library

use thiserror::Error;

/// Custom error type cho core library
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type alias sử dụng CoreError
pub type Result<T> = std::result::Result<T, CoreError>;
