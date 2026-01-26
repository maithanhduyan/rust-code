//! Oracle error types

use thiserror::Error;

/// Oracle-related errors
#[derive(Debug, Error)]
pub enum OracleError {
    /// Trading pair not found
    #[error("Trading pair not found: {pair}")]
    PairNotFound { pair: String },

    /// Price data is stale (older than threshold)
    #[error("Stale price for {pair}: last update was {last_update}, threshold is {threshold_secs}s")]
    StalePrice {
        pair: String,
        last_update: String,
        threshold_secs: u64,
    },

    /// Price data is invalid
    #[error("Invalid price for {pair}: {reason}")]
    InvalidPrice { pair: String, reason: String },

    /// External oracle connection failed
    #[error("Oracle connection failed: {source}")]
    ConnectionFailed {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
