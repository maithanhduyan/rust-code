//! # Error Module
//!
//! Định nghĩa các domain errors cho Simbank sử dụng thiserror.

use crate::wallet::WalletType;
use rust_decimal::Decimal;
use thiserror::Error;

/// Core domain errors.
///
/// Các lỗi nghiệp vụ cốt lõi, không liên quan đến infrastructure.
#[derive(Debug, Error)]
pub enum CoreError {
    // === Money errors ===
    #[error("Insufficient balance: need {needed}, available {available}")]
    InsufficientBalance { needed: Decimal, available: Decimal },

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Currency mismatch: expected {expected}, got {actual}")]
    CurrencyMismatch { expected: String, actual: String },

    #[error("Unknown currency: {0}")]
    UnknownCurrency(String),

    // === Account errors ===
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Account is frozen: {0}")]
    AccountFrozen(String),

    #[error("Account is closed: {0}")]
    AccountClosed(String),

    #[error("Account already exists: {0}")]
    AccountAlreadyExists(String),

    // === Wallet errors ===
    #[error("Wallet not found: {account_id} - {wallet_type}")]
    WalletNotFound {
        account_id: String,
        wallet_type: WalletType,
    },

    #[error("Wallet is frozen: {0}")]
    WalletFrozen(String),

    #[error("Invalid wallet type for operation: {0}")]
    InvalidWalletType(String),

    #[error("Cannot transfer to same wallet")]
    SameWalletTransfer,

    // === Person errors ===
    #[error("Person not found: {0}")]
    PersonNotFound(String),

    #[error("Person already exists: {0}")]
    PersonAlreadyExists(String),

    #[error("Invalid person type: {0}")]
    InvalidPersonType(String),

    // === Permission errors ===
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Operation requires approval")]
    RequiresApproval,

    // === Validation errors ===
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Invalid ID format: {0}")]
    InvalidIdFormat(String),

    // === AML errors ===
    #[error("Transaction blocked by AML: {0}")]
    AmlBlocked(String),

    #[error("Transaction flagged for review: {0}")]
    AmlFlagged(String),
}

/// Result type alias với CoreError
pub type CoreResult<T> = Result<T, CoreError>;

impl CoreError {
    /// Kiểm tra có phải lỗi insufficient balance không
    pub fn is_insufficient_balance(&self) -> bool {
        matches!(self, CoreError::InsufficientBalance { .. })
    }

    /// Kiểm tra có phải lỗi permission không
    pub fn is_permission_error(&self) -> bool {
        matches!(
            self,
            CoreError::PermissionDenied(_) | CoreError::RequiresApproval
        )
    }

    /// Kiểm tra có phải lỗi AML không
    pub fn is_aml_error(&self) -> bool {
        matches!(self, CoreError::AmlBlocked(_) | CoreError::AmlFlagged(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_error_display() {
        let err = CoreError::InsufficientBalance {
            needed: dec!(1000),
            available: dec!(500),
        };
        assert_eq!(
            err.to_string(),
            "Insufficient balance: need 1000, available 500"
        );

        let err = CoreError::AccountNotFound("ACC_001".to_string());
        assert_eq!(err.to_string(), "Account not found: ACC_001");
    }

    #[test]
    fn test_error_checks() {
        let err = CoreError::InsufficientBalance {
            needed: dec!(100),
            available: dec!(50),
        };
        assert!(err.is_insufficient_balance());

        let err = CoreError::PermissionDenied("test".to_string());
        assert!(err.is_permission_error());

        let err = CoreError::AmlBlocked("suspicious".to_string());
        assert!(err.is_aml_error());
    }

    #[test]
    fn test_wallet_not_found() {
        let err = CoreError::WalletNotFound {
            account_id: "ACC_001".to_string(),
            wallet_type: WalletType::Spot,
        };
        assert!(err.to_string().contains("ACC_001"));
        assert!(err.to_string().contains("spot"));
    }
}
