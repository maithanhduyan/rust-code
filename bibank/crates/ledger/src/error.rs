//! Ledger errors

use rust_decimal::Decimal;
use thiserror::Error;

/// Errors that can occur in ledger operations
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    #[error("Invalid account format: {0}")]
    InvalidAccountFormat(String),

    #[error("Unknown account category: {0}")]
    UnknownCategory(String),

    #[error("Entry must have at least 2 postings for double-entry")]
    InsufficientPostings,

    #[error("Entry unbalanced for asset {asset}: imbalance {imbalance}")]
    UnbalancedEntry { asset: String, imbalance: Decimal },

    #[error("correlation_id cannot be empty")]
    EmptyCorrelationId,

    #[error("Genesis entry must have sequence = 1")]
    InvalidGenesisSequence,

    #[error("Genesis entry must have prev_hash = 'GENESIS'")]
    InvalidGenesisPrevHash,

    #[error("Broken hash chain at sequence {sequence}: expected {expected}, got {actual}")]
    BrokenHashChain {
        sequence: u64,
        expected: String,
        actual: String,
    },

    #[error("Sequence must be strictly increasing: expected {expected}, got {actual}")]
    InvalidSequence { expected: u64, actual: u64 },
}
