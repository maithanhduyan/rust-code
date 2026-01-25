//! Event store errors

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EventError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Event store not initialized")]
    NotInitialized,

    #[error("Invalid event file: {0}")]
    InvalidFile(String),
}
