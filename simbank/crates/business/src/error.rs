//! Business layer errors
//!
//! Uses anyhow for error aggregation with custom error types.

use rust_decimal::Decimal;
use thiserror::Error;

/// Business operation errors
#[derive(Debug, Error)]
pub enum BusinessError {
    // === Validation errors ===
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
    },

    #[error("Currency mismatch: expected {expected}, got {actual}")]
    CurrencyMismatch { expected: String, actual: String },

    // === Permission errors ===
    #[error("Operation not permitted for {person_type}: {operation}")]
    OperationNotPermitted {
        person_type: String,
        operation: String,
    },

    #[error("Account not active: {account_id} (status: {status})")]
    AccountNotActive { account_id: String, status: String },

    #[error("Wallet not active: {wallet_id} (status: {status})")]
    WalletNotActive { wallet_id: String, status: String },

    // === Not found errors ===
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Wallet not found: {0}")]
    WalletNotFound(String),

    #[error("Person not found: {0}")]
    PersonNotFound(String),

    #[error("Currency not found: {0}")]
    CurrencyNotFound(String),

    // === AML errors ===
    #[error("Transaction requires approval: {reason}")]
    RequiresApproval { reason: String },

    #[error("Transaction blocked by AML: {reason}")]
    AmlBlocked { reason: String },

    // === Wrapped errors ===
    #[error("Persistence error: {0}")]
    Persistence(#[from] simbank_persistence::PersistenceError),

    #[error("Core error: {0}")]
    Core(#[from] simbank_core::CoreError),
}

/// Result type alias for business operations
pub type BusinessResult<T> = anyhow::Result<T>;

impl BusinessError {
    /// Create insufficient balance error
    pub fn insufficient_balance(required: Decimal, available: Decimal) -> Self {
        Self::InsufficientBalance {
            required,
            available,
        }
    }

    /// Create operation not permitted error
    pub fn not_permitted(person_type: &str, operation: &str) -> Self {
        Self::OperationNotPermitted {
            person_type: person_type.to_string(),
            operation: operation.to_string(),
        }
    }

    /// Create account not active error
    pub fn account_not_active(account_id: &str, status: &str) -> Self {
        Self::AccountNotActive {
            account_id: account_id.to_string(),
            status: status.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_insufficient_balance_error() {
        let err = BusinessError::insufficient_balance(dec!(100), dec!(50));
        assert!(err.to_string().contains("required 100"));
        assert!(err.to_string().contains("available 50"));
    }

    #[test]
    fn test_not_permitted_error() {
        let err = BusinessError::not_permitted("Auditor", "deposit");
        assert!(err.to_string().contains("Auditor"));
        assert!(err.to_string().contains("deposit"));
    }
}
