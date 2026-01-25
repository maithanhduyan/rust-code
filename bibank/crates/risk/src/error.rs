//! Risk engine errors

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum RiskError {
    #[error("Insufficient balance for {account}: available {available}, required {required}")]
    InsufficientBalance {
        account: String,
        available: String,
        required: String,
    },

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Risk check failed: {0}")]
    CheckFailed(String),
}
