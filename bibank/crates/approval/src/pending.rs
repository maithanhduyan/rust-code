//! Pending approval data structures

use bibank_ledger::EntrySignature;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Status of a pending approval
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    /// Awaiting required signatures
    Pending,
    /// Required signatures collected, entry submitted to ledger
    Approved,
    /// Explicitly rejected by an operator
    Rejected,
    /// Expired due to timeout (24h default)
    Expired,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApprovalStatus::Pending => "pending",
            ApprovalStatus::Approved => "approved",
            ApprovalStatus::Rejected => "rejected",
            ApprovalStatus::Expired => "expired",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(ApprovalStatus::Pending),
            "approved" => Some(ApprovalStatus::Approved),
            "rejected" => Some(ApprovalStatus::Rejected),
            "expired" => Some(ApprovalStatus::Expired),
            _ => None,
        }
    }
}

/// A pending approval awaiting signatures
///
/// Note: We store the serialized JSON of the unsigned entry rather than
/// the struct itself, since UnsignedEntry doesn't implement Serialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Unique identifier for the approval request
    pub id: String,

    /// JSON-serialized unsigned entry data for storage
    /// Format: { intent, correlation_id, postings, metadata }
    pub unsigned_entry_json: String,

    /// SHA256 hash of the unsigned entry JSON for verification
    pub unsigned_entry_hash: String,

    /// Number of signatures required (e.g., 2 for 2-of-3)
    pub required_signatures: u8,

    /// Signatures collected so far (serialized for storage)
    pub collected_signatures: Vec<CollectedSignature>,

    /// When the approval request was created
    pub created_at: DateTime<Utc>,

    /// When the approval expires
    pub expires_at: DateTime<Utc>,

    /// Current status
    pub status: ApprovalStatus,

    /// Optional reason for rejection
    pub rejection_reason: Option<String>,
}

/// A simplified signature record for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectedSignature {
    pub signer_id: String,
    pub public_key: String,
    pub signature: String,
    pub signed_at: DateTime<Utc>,
}

impl From<&EntrySignature> for CollectedSignature {
    fn from(sig: &EntrySignature) -> Self {
        Self {
            signer_id: sig.signer_id.clone(),
            public_key: sig.public_key.clone(),
            signature: sig.signature.clone(),
            signed_at: sig.signed_at,
        }
    }
}

impl PendingApproval {
    /// Create a new pending approval from unsigned entry JSON
    pub fn new(
        unsigned_entry_json: String,
        required_signatures: u8,
        expiry_hours: i64,
    ) -> Self {
        let id = format!("APPR-{}", uuid::Uuid::new_v4().to_string()[..8].to_uppercase());
        let hash = compute_hash(&unsigned_entry_json);
        let now = Utc::now();

        Self {
            id,
            unsigned_entry_json,
            unsigned_entry_hash: hash,
            required_signatures,
            collected_signatures: Vec::new(),
            created_at: now,
            expires_at: now + chrono::Duration::hours(expiry_hours),
            status: ApprovalStatus::Pending,
            rejection_reason: None,
        }
    }

    /// Check if the approval has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Check if enough signatures have been collected
    pub fn has_enough_signatures(&self) -> bool {
        self.collected_signatures.len() >= self.required_signatures as usize
    }

    /// Get the number of remaining signatures needed
    pub fn signatures_remaining(&self) -> usize {
        let collected = self.collected_signatures.len();
        let required = self.required_signatures as usize;
        if collected >= required {
            0
        } else {
            required - collected
        }
    }

    /// Add a signature (returns false if already signed by this signer)
    pub fn add_signature(&mut self, signature: CollectedSignature) -> bool {
        // Check if this signer has already signed
        if self.collected_signatures.iter().any(|s| s.signer_id == signature.signer_id) {
            return false;
        }

        self.collected_signatures.push(signature);
        true
    }

    /// Get list of signer IDs that have signed
    pub fn signers(&self) -> Vec<&str> {
        self.collected_signatures.iter().map(|s| s.signer_id.as_str()).collect()
    }
}

/// Compute SHA256 hash of a string
fn compute_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry_json() -> String {
        r#"{"intent":"Adjustment","correlation_id":"test-adj-001","postings":[],"metadata":{}}"#.to_string()
    }

    #[test]
    fn test_pending_approval_creation() {
        let entry_json = create_test_entry_json();
        let approval = PendingApproval::new(entry_json, 2, 24);

        assert!(approval.id.starts_with("APPR-"));
        assert_eq!(approval.required_signatures, 2);
        assert_eq!(approval.status, ApprovalStatus::Pending);
        assert_eq!(approval.collected_signatures.len(), 0);
        assert!(!approval.is_expired());
    }

    #[test]
    fn test_signatures_remaining() {
        let entry_json = create_test_entry_json();
        let mut approval = PendingApproval::new(entry_json, 2, 24);

        assert_eq!(approval.signatures_remaining(), 2);
        assert!(!approval.has_enough_signatures());

        // Add first signature
        let sig1 = CollectedSignature {
            signer_id: "operator1".to_string(),
            public_key: "pk1".to_string(),
            signature: "sig1".to_string(),
            signed_at: Utc::now(),
        };
        assert!(approval.add_signature(sig1));
        assert_eq!(approval.signatures_remaining(), 1);

        // Add second signature
        let sig2 = CollectedSignature {
            signer_id: "operator2".to_string(),
            public_key: "pk2".to_string(),
            signature: "sig2".to_string(),
            signed_at: Utc::now(),
        };
        assert!(approval.add_signature(sig2));
        assert_eq!(approval.signatures_remaining(), 0);
        assert!(approval.has_enough_signatures());
    }

    #[test]
    fn test_duplicate_signature_rejected() {
        let entry_json = create_test_entry_json();
        let mut approval = PendingApproval::new(entry_json, 2, 24);

        let sig1 = CollectedSignature {
            signer_id: "operator1".to_string(),
            public_key: "pk1".to_string(),
            signature: "sig1".to_string(),
            signed_at: Utc::now(),
        };

        assert!(approval.add_signature(sig1.clone()));
        // Duplicate should be rejected
        assert!(!approval.add_signature(sig1));
        assert_eq!(approval.collected_signatures.len(), 1);
    }

    #[test]
    fn test_signers_list() {
        let entry_json = create_test_entry_json();
        let mut approval = PendingApproval::new(entry_json, 3, 24);

        approval.add_signature(CollectedSignature {
            signer_id: "alice".to_string(),
            public_key: "pk_a".to_string(),
            signature: "sig_a".to_string(),
            signed_at: Utc::now(),
        });
        approval.add_signature(CollectedSignature {
            signer_id: "bob".to_string(),
            public_key: "pk_b".to_string(),
            signature: "sig_b".to_string(),
            signed_at: Utc::now(),
        });

        let signers = approval.signers();
        assert_eq!(signers.len(), 2);
        assert!(signers.contains(&"alice"));
        assert!(signers.contains(&"bob"));
    }

    #[test]
    fn test_approval_status_serialization() {
        assert_eq!(ApprovalStatus::Pending.as_str(), "pending");
        assert_eq!(ApprovalStatus::Approved.as_str(), "approved");
        assert_eq!(ApprovalStatus::Rejected.as_str(), "rejected");
        assert_eq!(ApprovalStatus::Expired.as_str(), "expired");

        assert_eq!(ApprovalStatus::from_str("pending"), Some(ApprovalStatus::Pending));
        assert_eq!(ApprovalStatus::from_str("invalid"), None);
    }

    #[test]
    fn test_compute_hash() {
        let hash1 = compute_hash("test data");
        let hash2 = compute_hash("test data");
        let hash3 = compute_hash("different data");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA256 hex = 64 chars
    }
}
