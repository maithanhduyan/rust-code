use std::fmt;

/// Error types for the chat application
#[derive(Debug)]
pub enum ChatError {
    /// WebSocket communication error
    WebSocketError(String),
    /// Client not found
    ClientNotFound(String),
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::WebSocketError(msg) => write!(f, "WebSocket error: {}", msg),
            ChatError::ClientNotFound(id) => write!(f, "Client not found: {}", id),
        }
    }
}

impl std::error::Error for ChatError {}
