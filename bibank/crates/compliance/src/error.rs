//! Compliance errors

use thiserror::Error;

/// Errors from the Compliance Engine
#[derive(Debug, Error)]
pub enum ComplianceError {
    #[error("Failed to write to compliance ledger: {0}")]
    LedgerWriteError(String),

    #[error("Failed to read compliance ledger: {0}")]
    LedgerReadError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("External service error: {0}")]
    ExternalServiceError(String),

    #[error("External service timeout after {0}ms")]
    ExternalServiceTimeout(u64),

    #[error("Rule evaluation error: {0}")]
    RuleError(String),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),

    #[error("Review not found: {0}")]
    ReviewNotFound(String),

    #[error("Review already resolved: {0}")]
    ReviewAlreadyResolved(String),

    #[error("Review expired: {0}")]
    ReviewExpired(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

/// Result type for compliance operations
pub type ComplianceResult<T> = Result<T, ComplianceError>;
