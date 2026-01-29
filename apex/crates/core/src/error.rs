//! Error types for Apex
//!
//! All errors are non-panicking and propagate via Result.

use thiserror::Error;

/// Core proxy errors
#[derive(Error, Debug)]
pub enum ProxyError {
    /// Backend is not available
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    /// No healthy backend found
    #[error("no healthy backend available")]
    NoHealthyBackend,

    /// Route not found
    #[error("route not found for host: {host}, path: {path}")]
    RouteNotFound {
        /// The requested host
        host: String,
        /// The requested path
        path: String,
    },

    /// Invalid request
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// Connection error
    #[error("connection error: {0}")]
    ConnectionError(String),

    /// Timeout
    #[error("request timeout")]
    Timeout,

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}

impl ProxyError {
    /// Returns the appropriate HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            ProxyError::BackendUnavailable(_) => 503,
            ProxyError::NoHealthyBackend => 503,
            ProxyError::RouteNotFound { .. } => 404,
            ProxyError::InvalidRequest(_) => 400,
            ProxyError::ConnectionError(_) => 502,
            ProxyError::Timeout => 504,
            ProxyError::Internal(_) => 500,
        }
    }
}

/// Result type alias using ProxyError
pub type Result<T> = std::result::Result<T, ProxyError>;
