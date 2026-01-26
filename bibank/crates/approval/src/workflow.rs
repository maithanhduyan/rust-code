//! Approval workflow logic

use crate::pending::{ApprovalStatus, CollectedSignature, PendingApproval};
use crate::store::{ApprovalStore, StoreError};
use rust_decimal::Decimal;
use thiserror::Error;

/// Configuration for the approval workflow
#[derive(Debug, Clone)]
pub struct ApprovalConfig {
    /// Number of signatures required (N in N-of-M)
    pub required_signatures: u8,

    /// Total possible signers (M in N-of-M)
    pub total_signers: u8,

    /// Hours before approval expires
    pub expiry_hours: i64,

    /// Withdrawal threshold requiring multi-sig
    pub withdrawal_threshold: Decimal,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            required_signatures: 2,
            total_signers: 3,
            expiry_hours: 24,
            withdrawal_threshold: Decimal::new(100_000, 0), // 100,000 USDT
        }
    }
}

/// Errors from the approval workflow
#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("Store error: {0}")]
    Store(#[from] StoreError),

    #[error("Approval not found: {0}")]
    NotFound(String),

    #[error("Approval already {0}")]
    AlreadyProcessed(String),

    #[error("Approval has expired")]
    Expired,

    #[error("Duplicate signature from signer: {0}")]
    DuplicateSignature(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Cannot sign rejected approval")]
    CannotSignRejected,
}

/// Multi-signature approval workflow
pub struct ApprovalWorkflow {
    store: ApprovalStore,
    config: ApprovalConfig,
}

impl ApprovalWorkflow {
    /// Create a new workflow with the given store and config
    pub fn new(store: ApprovalStore, config: ApprovalConfig) -> Self {
        Self { store, config }
    }

    /// Create a new workflow with default config
    pub fn with_store(store: ApprovalStore) -> Self {
        Self::new(store, ApprovalConfig::default())
    }

    /// Get the configuration
    pub fn config(&self) -> &ApprovalConfig {
        &self.config
    }

    /// Create a new pending approval for an unsigned entry (as JSON)
    pub fn create_approval(&self, unsigned_entry_json: String) -> Result<PendingApproval, ApprovalError> {
        let approval = PendingApproval::new(
            unsigned_entry_json,
            self.config.required_signatures,
            self.config.expiry_hours,
        );

        self.store.save(&approval)?;
        Ok(approval)
    }

    /// Get a pending approval by ID
    pub fn get_approval(&self, id: &str) -> Result<PendingApproval, ApprovalError> {
        let mut approval = self.store.get(id)?;

        // Check and update expiry status
        if approval.status == ApprovalStatus::Pending && approval.is_expired() {
            approval.status = ApprovalStatus::Expired;
            self.store.save(&approval)?;
        }

        Ok(approval)
    }

    /// Add a signature to a pending approval
    /// Returns the updated approval, and whether it's now fully approved
    pub fn sign_approval(
        &self,
        id: &str,
        signature: CollectedSignature,
    ) -> Result<(PendingApproval, bool), ApprovalError> {
        let mut approval = self.store.get(id)?;

        // Check status
        match approval.status {
            ApprovalStatus::Pending => {}
            ApprovalStatus::Approved => {
                return Err(ApprovalError::AlreadyProcessed("approved".to_string()));
            }
            ApprovalStatus::Rejected => {
                return Err(ApprovalError::CannotSignRejected);
            }
            ApprovalStatus::Expired => {
                return Err(ApprovalError::Expired);
            }
        }

        // Check if expired
        if approval.is_expired() {
            approval.status = ApprovalStatus::Expired;
            self.store.save(&approval)?;
            return Err(ApprovalError::Expired);
        }

        // Check for duplicate signature
        if approval.collected_signatures.iter().any(|s| s.signer_id == signature.signer_id) {
            return Err(ApprovalError::DuplicateSignature(signature.signer_id));
        }

        // Add signature
        approval.add_signature(signature);

        // Check if fully approved
        let is_approved = approval.has_enough_signatures();
        if is_approved {
            approval.status = ApprovalStatus::Approved;
        }

        self.store.save(&approval)?;

        Ok((approval, is_approved))
    }

    /// Reject a pending approval
    pub fn reject_approval(&self, id: &str, reason: Option<&str>) -> Result<PendingApproval, ApprovalError> {
        let mut approval = self.store.get(id)?;

        if approval.status != ApprovalStatus::Pending {
            return Err(ApprovalError::AlreadyProcessed(approval.status.as_str().to_string()));
        }

        approval.status = ApprovalStatus::Rejected;
        approval.rejection_reason = reason.map(|s| s.to_string());
        self.store.save(&approval)?;

        Ok(approval)
    }

    /// List all pending approvals
    pub fn list_pending(&self) -> Result<Vec<PendingApproval>, ApprovalError> {
        // First, expire old approvals
        self.store.expire_old_approvals()?;

        Ok(self.store.list_by_status(ApprovalStatus::Pending)?)
    }

    /// List all approvals regardless of status
    pub fn list_all(&self) -> Result<Vec<PendingApproval>, ApprovalError> {
        Ok(self.store.list_all()?)
    }

    /// Check if an intent type requires multi-sig approval
    pub fn requires_approval_for_intent(intent: &str) -> bool {
        // Adjustment always requires approval
        intent == "Adjustment"
    }

    /// Get statistics about pending approvals
    pub fn get_stats(&self) -> Result<ApprovalStats, ApprovalError> {
        Ok(ApprovalStats {
            pending: self.store.count_by_status(ApprovalStatus::Pending)?,
            approved: self.store.count_by_status(ApprovalStatus::Approved)?,
            rejected: self.store.count_by_status(ApprovalStatus::Rejected)?,
            expired: self.store.count_by_status(ApprovalStatus::Expired)?,
        })
    }
}

/// Statistics about approvals
#[derive(Debug, Clone)]
pub struct ApprovalStats {
    pub pending: usize,
    pub approved: usize,
    pub rejected: usize,
    pub expired: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_store() -> ApprovalStore {
        ApprovalStore::in_memory().unwrap()
    }

    fn create_test_entry_json() -> String {
        r#"{"intent":"Adjustment","correlation_id":"test-adj-001"}"#.to_string()
    }

    #[test]
    fn test_create_approval() {
        let store = create_test_store();
        let workflow = ApprovalWorkflow::with_store(store);
        let entry_json = create_test_entry_json();

        let approval = workflow.create_approval(entry_json).unwrap();

        assert!(approval.id.starts_with("APPR-"));
        assert_eq!(approval.status, ApprovalStatus::Pending);
        assert_eq!(approval.required_signatures, 2);
    }

    #[test]
    fn test_sign_approval() {
        let store = create_test_store();
        let workflow = ApprovalWorkflow::with_store(store);
        let entry_json = create_test_entry_json();

        let approval = workflow.create_approval(entry_json).unwrap();
        let id = approval.id.clone();

        // First signature
        let sig1 = CollectedSignature {
            signer_id: "operator1".to_string(),
            public_key: "pk1".to_string(),
            signature: "sig1".to_string(),
            signed_at: Utc::now(),
        };
        let (approval, is_approved) = workflow.sign_approval(&id, sig1).unwrap();
        assert!(!is_approved);
        assert_eq!(approval.collected_signatures.len(), 1);

        // Second signature - should complete approval
        let sig2 = CollectedSignature {
            signer_id: "operator2".to_string(),
            public_key: "pk2".to_string(),
            signature: "sig2".to_string(),
            signed_at: Utc::now(),
        };
        let (approval, is_approved) = workflow.sign_approval(&id, sig2).unwrap();
        assert!(is_approved);
        assert_eq!(approval.status, ApprovalStatus::Approved);
    }

    #[test]
    fn test_duplicate_signature_rejected() {
        let store = create_test_store();
        let workflow = ApprovalWorkflow::with_store(store);
        let entry_json = create_test_entry_json();

        let approval = workflow.create_approval(entry_json).unwrap();
        let id = approval.id.clone();

        let sig1 = CollectedSignature {
            signer_id: "operator1".to_string(),
            public_key: "pk1".to_string(),
            signature: "sig1".to_string(),
            signed_at: Utc::now(),
        };
        workflow.sign_approval(&id, sig1.clone()).unwrap();

        // Try to sign again with same signer
        let result = workflow.sign_approval(&id, sig1);
        assert!(matches!(result, Err(ApprovalError::DuplicateSignature(_))));
    }

    #[test]
    fn test_reject_approval() {
        let store = create_test_store();
        let workflow = ApprovalWorkflow::with_store(store);
        let entry_json = create_test_entry_json();

        let approval = workflow.create_approval(entry_json).unwrap();
        let id = approval.id.clone();

        let approval = workflow.reject_approval(&id, Some("Invalid adjustment")).unwrap();

        assert_eq!(approval.status, ApprovalStatus::Rejected);
        assert_eq!(approval.rejection_reason, Some("Invalid adjustment".to_string()));
    }

    #[test]
    fn test_cannot_sign_rejected() {
        let store = create_test_store();
        let workflow = ApprovalWorkflow::with_store(store);
        let entry_json = create_test_entry_json();

        let approval = workflow.create_approval(entry_json).unwrap();
        let id = approval.id.clone();

        workflow.reject_approval(&id, None).unwrap();

        let sig = CollectedSignature {
            signer_id: "operator1".to_string(),
            public_key: "pk1".to_string(),
            signature: "sig1".to_string(),
            signed_at: Utc::now(),
        };

        let result = workflow.sign_approval(&id, sig);
        assert!(matches!(result, Err(ApprovalError::CannotSignRejected)));
    }

    #[test]
    fn test_requires_approval_for_intent() {
        assert!(ApprovalWorkflow::requires_approval_for_intent("Adjustment"));
        assert!(!ApprovalWorkflow::requires_approval_for_intent("Deposit"));
        assert!(!ApprovalWorkflow::requires_approval_for_intent("Transfer"));
    }

    #[test]
    fn test_get_stats() {
        let store = create_test_store();
        let workflow = ApprovalWorkflow::with_store(store);

        // Create some approvals
        for i in 0..3 {
            let entry_json = format!(r#"{{"intent":"Adjustment","correlation_id":"test-{}"}}"#, i);
            workflow.create_approval(entry_json).unwrap();
        }

        let stats = workflow.get_stats().unwrap();
        assert_eq!(stats.pending, 3);
        assert_eq!(stats.approved, 0);
        assert_eq!(stats.rejected, 0);
    }
}
