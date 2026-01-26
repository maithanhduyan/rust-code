//! Hook errors

use thiserror::Error;

/// Errors from hooks
#[derive(Debug, Error)]
pub enum HookError {
    #[error("Pre-validation rejected: {reason} (code: {code})")]
    Rejected { reason: String, code: String },

    #[error("Hook timeout after {0}ms")]
    Timeout(u64),

    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for hook operations
pub type HookResult<T> = Result<T, HookError>;

impl HookError {
    /// Create a rejection error
    pub fn rejected(reason: impl Into<String>, code: impl Into<String>) -> Self {
        HookError::Rejected {
            reason: reason.into(),
            code: code.into(),
        }
    }

    /// Check if this is a rejection
    pub fn is_rejection(&self) -> bool {
        matches!(self, HookError::Rejected { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejected_error() {
        let err = HookError::rejected("Sanctions match", "OFAC-001");
        assert!(err.is_rejection());
        assert!(err.to_string().contains("Sanctions match"));
        assert!(err.to_string().contains("OFAC-001"));
    }

    #[test]
    fn test_timeout_error() {
        let err = HookError::Timeout(500);
        assert!(!err.is_rejection());
        assert!(err.to_string().contains("500ms"));
    }
}
