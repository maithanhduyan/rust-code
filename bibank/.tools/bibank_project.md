---
date: 2026-01-27 18:22:25 
---

# Cấu trúc Dự án như sau:

```
./bibank
├── Cargo.toml
└── crates
    ├── approval
    │   ├── Cargo.toml
    │   └── src
    │       ├── lib.rs
    │       ├── pending.rs
    │       ├── store.rs
    │       └── workflow.rs
    ├── bus
    │   ├── Cargo.toml
    │   └── src
    │       ├── channel.rs
    │       ├── error.rs
    │       ├── event.rs
    │       ├── lib.rs
    │       └── subscriber.rs
    ├── compliance
    │   ├── Cargo.toml
    │   └── src
    │       ├── config.rs
    │       ├── decision.rs
    │       ├── engine.rs
    │       ├── error.rs
    │       ├── event.rs
    │       ├── ledger.rs
    │       ├── lib.rs
    │       └── state.rs
    ├── core
    │   ├── Cargo.toml
    │   └── src
    │       ├── amount.rs
    │       ├── currency.rs
    │       └── lib.rs
    ├── dsl
    │   ├── Cargo.toml
    │   └── src
    │       ├── evaluator.rs
    │       ├── lib.rs
    │       ├── macros.rs
    │       └── types.rs
    ├── events
    │   ├── Cargo.toml
    │   └── src
    │       ├── error.rs
    │       ├── lib.rs
    │       ├── reader.rs
    │       └── store.rs
    ├── hooks
    │   ├── Cargo.toml
    │   └── src
    │       ├── aml.rs
    │       ├── context.rs
    │       ├── error.rs
    │       ├── executor.rs
    │       ├── lib.rs
    │       ├── registry.rs
    │       └── traits.rs
    ├── ledger
    │   ├── Cargo.toml
    │   └── src
    │       ├── account.rs
    │       ├── entry.rs
    │       ├── error.rs
    │       ├── hash.rs
    │       ├── lib.rs
    │       ├── signature.rs
    │       └── validation.rs
    ├── matching
    │   ├── Cargo.toml
    │   └── src
    │       ├── engine.rs
    │       ├── error.rs
    │       ├── fill.rs
    │       ├── lib.rs
    │       ├── order.rs
    │       └── orderbook.rs
    ├── oracle
    │   ├── Cargo.toml
    │   └── src
    │       ├── error.rs
    │       ├── lib.rs
    │       ├── mock.rs
    │       └── types.rs
    ├── projection
    │   ├── Cargo.toml
    │   └── src
    │       ├── balance.rs
    │       ├── engine.rs
    │       ├── error.rs
    │       ├── lib.rs
    │       └── trade.rs
    ├── risk
    │   ├── Cargo.toml
    │   └── src
    │       ├── engine.rs
    │       ├── error.rs
    │       ├── interest.rs
    │       ├── lib.rs
    │       ├── liquidation.rs
    │       └── state.rs
    └── rpc
        ├── Cargo.toml
        └── src
            ├── commands.rs
            ├── context.rs
            ├── lib.rs
            └── main.rs
```

# Danh sách chi tiết các file:

## File ./bibank\crates\approval\src\lib.rs:
```rust
//! # BiBank Approval Module
//!
//! Multi-signature approval workflow for critical operations.
//!
//! ## Scope
//! - `Adjustment` intent entries
//! - Withdrawals > threshold (e.g., 100,000 USDT)
//! - System parameter changes
//!
//! ## Features
//! - 2-of-3 multi-sig by default
//! - 24h expiry for pending approvals
//! - SQLite storage for pending state
//! - No ledger entry until approved

mod pending;
mod store;
mod workflow;

pub use pending::{ApprovalStatus, CollectedSignature, PendingApproval};
pub use store::{ApprovalStore, StoreError};
pub use workflow::{ApprovalWorkflow, ApprovalError, ApprovalConfig, ApprovalStats};

```

## File ./bibank\crates\approval\src\pending.rs:
```rust
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

```

## File ./bibank\crates\approval\src\store.rs:
```rust
//! SQLite storage for pending approvals

use crate::pending::{ApprovalStatus, PendingApproval};
use rusqlite::{Connection, params};
use std::path::Path;
use thiserror::Error;

/// Errors from the approval store
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Approval not found: {0}")]
    NotFound(String),
}

/// SQLite storage for pending approvals
pub struct ApprovalStore {
    conn: Connection,
}

impl ApprovalStore {
    /// Create a new store with the given database path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Create an in-memory store (for testing)
    pub fn in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<(), StoreError> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS pending_approvals (
                id TEXT PRIMARY KEY,
                unsigned_entry_json TEXT NOT NULL,
                unsigned_entry_hash TEXT NOT NULL,
                required_signatures INTEGER NOT NULL,
                collected_signatures_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                status TEXT NOT NULL,
                rejection_reason TEXT
            )",
            [],
        )?;

        // Index for efficient status queries
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_pending_approvals_status
             ON pending_approvals(status)",
            [],
        )?;

        Ok(())
    }

    /// Save a pending approval
    pub fn save(&self, approval: &PendingApproval) -> Result<(), StoreError> {
        let collected_signatures_json = serde_json::to_string(&approval.collected_signatures)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO pending_approvals
             (id, unsigned_entry_json, unsigned_entry_hash, required_signatures,
              collected_signatures_json, created_at, expires_at, status, rejection_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                approval.id,
                approval.unsigned_entry_json,
                approval.unsigned_entry_hash,
                approval.required_signatures,
                collected_signatures_json,
                approval.created_at.to_rfc3339(),
                approval.expires_at.to_rfc3339(),
                approval.status.as_str(),
                approval.rejection_reason,
            ],
        )?;

        Ok(())
    }

    /// Get a pending approval by ID
    pub fn get(&self, id: &str) -> Result<PendingApproval, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, unsigned_entry_json, unsigned_entry_hash, required_signatures,
                    collected_signatures_json, created_at, expires_at, status, rejection_reason
             FROM pending_approvals WHERE id = ?1"
        )?;

        let approval = stmt.query_row(params![id], |row| {
            let collected_signatures_json: String = row.get(4)?;
            let created_at_str: String = row.get(5)?;
            let expires_at_str: String = row.get(6)?;
            let status_str: String = row.get(7)?;

            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u8>(3)?,
                collected_signatures_json,
                created_at_str,
                expires_at_str,
                status_str,
                row.get::<_, Option<String>>(8)?,
            ))
        }).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound(id.to_string()),
            other => StoreError::Database(other),
        })?;

        let collected_signatures = serde_json::from_str(&approval.4)
            .map_err(StoreError::Serialization)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&approval.5)
            .map_err(|_| StoreError::NotFound("invalid date".to_string()))?
            .with_timezone(&chrono::Utc);
        let expires_at = chrono::DateTime::parse_from_rfc3339(&approval.6)
            .map_err(|_| StoreError::NotFound("invalid date".to_string()))?
            .with_timezone(&chrono::Utc);
        let status = ApprovalStatus::from_str(&approval.7)
            .ok_or_else(|| StoreError::NotFound("invalid status".to_string()))?;

        Ok(PendingApproval {
            id: approval.0,
            unsigned_entry_json: approval.1,
            unsigned_entry_hash: approval.2,
            required_signatures: approval.3,
            collected_signatures,
            created_at,
            expires_at,
            status,
            rejection_reason: approval.8,
        })
    }

    /// List all pending approvals with a specific status
    pub fn list_by_status(&self, status: ApprovalStatus) -> Result<Vec<PendingApproval>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM pending_approvals WHERE status = ?1 ORDER BY created_at DESC"
        )?;

        let ids: Vec<String> = stmt
            .query_map(params![status.as_str()], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut approvals = Vec::new();
        for id in ids {
            approvals.push(self.get(&id)?);
        }

        Ok(approvals)
    }

    /// List all pending approvals (any status)
    pub fn list_all(&self) -> Result<Vec<PendingApproval>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM pending_approvals ORDER BY created_at DESC"
        )?;

        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut approvals = Vec::new();
        for id in ids {
            approvals.push(self.get(&id)?);
        }

        Ok(approvals)
    }

    /// Update the status of an approval
    pub fn update_status(
        &self,
        id: &str,
        status: ApprovalStatus,
        rejection_reason: Option<&str>,
    ) -> Result<(), StoreError> {
        let rows = self.conn.execute(
            "UPDATE pending_approvals SET status = ?1, rejection_reason = ?2 WHERE id = ?3",
            params![status.as_str(), rejection_reason, id],
        )?;

        if rows == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }

        Ok(())
    }

    /// Delete an approval by ID
    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        let rows = self.conn.execute(
            "DELETE FROM pending_approvals WHERE id = ?1",
            params![id],
        )?;

        if rows == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }

        Ok(())
    }

    /// Count approvals by status
    pub fn count_by_status(&self, status: ApprovalStatus) -> Result<usize, StoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pending_approvals WHERE status = ?1",
            params![status.as_str()],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    /// Mark expired approvals
    pub fn expire_old_approvals(&self) -> Result<usize, StoreError> {
        let now = chrono::Utc::now().to_rfc3339();
        let rows = self.conn.execute(
            "UPDATE pending_approvals
             SET status = 'expired'
             WHERE status = 'pending' AND expires_at < ?1",
            params![now],
        )?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry_json() -> String {
        r#"{"intent":"Adjustment","correlation_id":"test-adj-001"}"#.to_string()
    }

    #[test]
    fn test_store_save_and_get() {
        let store = ApprovalStore::in_memory().unwrap();
        let entry_json = create_test_entry_json();
        let approval = PendingApproval::new(entry_json, 2, 24);
        let id = approval.id.clone();

        store.save(&approval).unwrap();
        let retrieved = store.get(&id).unwrap();

        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.required_signatures, 2);
        assert_eq!(retrieved.status, ApprovalStatus::Pending);
    }

    #[test]
    fn test_store_list_by_status() {
        let store = ApprovalStore::in_memory().unwrap();

        // Create 3 approvals
        for i in 0..3 {
            let entry_json = format!(r#"{{"intent":"Adjustment","correlation_id":"test-{}"}}"#, i);
            let approval = PendingApproval::new(entry_json, 2, 24);
            store.save(&approval).unwrap();
        }

        let pending = store.list_by_status(ApprovalStatus::Pending).unwrap();
        assert_eq!(pending.len(), 3);
    }

    #[test]
    fn test_store_update_status() {
        let store = ApprovalStore::in_memory().unwrap();
        let entry_json = create_test_entry_json();
        let approval = PendingApproval::new(entry_json, 2, 24);
        let id = approval.id.clone();

        store.save(&approval).unwrap();
        store.update_status(&id, ApprovalStatus::Approved, None).unwrap();

        let retrieved = store.get(&id).unwrap();
        assert_eq!(retrieved.status, ApprovalStatus::Approved);
    }

    #[test]
    fn test_store_delete() {
        let store = ApprovalStore::in_memory().unwrap();
        let entry_json = create_test_entry_json();
        let approval = PendingApproval::new(entry_json, 2, 24);
        let id = approval.id.clone();

        store.save(&approval).unwrap();
        store.delete(&id).unwrap();

        let result = store.get(&id);
        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }

    #[test]
    fn test_store_count_by_status() {
        let store = ApprovalStore::in_memory().unwrap();

        for i in 0..5 {
            let entry_json = format!(r#"{{"intent":"Adjustment","correlation_id":"count-test-{}"}}"#, i);
            let approval = PendingApproval::new(entry_json, 2, 24);
            store.save(&approval).unwrap();
        }

        let count = store.count_by_status(ApprovalStatus::Pending).unwrap();
        assert_eq!(count, 5);
    }
}

```

## File ./bibank\crates\approval\src\workflow.rs:
```rust
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

```

## File ./bibank\crates\bus\src\channel.rs:
```rust
//! Async event bus implementation
//!
//! Phase 2: Async broadcast channel with subscriber management

use crate::error::BusError;
use crate::event::LedgerEvent;
use crate::subscriber::EventSubscriber;
use bibank_events::EventReader;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Default channel capacity
const DEFAULT_CAPACITY: usize = 1024;

/// Async event bus for distributing ledger events
///
/// The bus uses a broadcast channel for non-blocking event distribution.
/// Subscribers receive events asynchronously and can fail independently.
pub struct EventBus {
    /// Journal path for replay
    journal_path: std::path::PathBuf,
    /// Broadcast sender
    sender: broadcast::Sender<LedgerEvent>,
    /// Registered subscribers
    subscribers: Vec<Arc<dyn EventSubscriber>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new(journal_path: impl AsRef<Path>) -> Self {
        let (sender, _) = broadcast::channel(DEFAULT_CAPACITY);
        Self {
            journal_path: journal_path.as_ref().to_path_buf(),
            sender,
            subscribers: Vec::new(),
        }
    }

    /// Create with custom capacity
    pub fn with_capacity(journal_path: impl AsRef<Path>, capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            journal_path: journal_path.as_ref().to_path_buf(),
            sender,
            subscribers: Vec::new(),
        }
    }

    /// Register a subscriber
    pub fn subscribe(&mut self, subscriber: Arc<dyn EventSubscriber>) {
        info!("Registering subscriber: {}", subscriber.name());
        self.subscribers.push(subscriber);
    }

    /// Publish an event to all subscribers
    ///
    /// This is non-blocking - the event is sent to the broadcast channel
    /// and subscribers process it asynchronously.
    pub async fn publish(&self, event: LedgerEvent) -> Result<(), BusError> {
        debug!("Publishing event to {} subscribers", self.subscribers.len());

        // Send to broadcast channel (for any channel receivers)
        let _ = self.sender.send(event.clone());

        // Also directly notify registered subscribers
        for subscriber in &self.subscribers {
            if let Err(e) = subscriber.handle(&event).await {
                // Log but don't fail - subscriber failures don't block ledger
                warn!(
                    "Subscriber '{}' failed to handle event: {}",
                    subscriber.name(),
                    e
                );
            }
        }

        Ok(())
    }

    /// Replay events from a sequence number
    ///
    /// Reads from JSONL files and publishes to subscribers.
    pub async fn replay_from(&self, from_sequence: u64) -> Result<usize, BusError> {
        info!("Starting replay from sequence {}", from_sequence);

        // Notify subscribers of replay start
        let start_event = LedgerEvent::replay_started(from_sequence);
        for subscriber in &self.subscribers {
            if let Err(e) = subscriber.on_replay_start().await {
                error!("Subscriber '{}' failed on replay start: {}", subscriber.name(), e);
            }
        }
        let _ = self.sender.send(start_event);

        // Read entries from JSONL
        let reader = EventReader::from_directory(&self.journal_path)?;
        let entries = reader.read_all()?;

        let mut count = 0;
        for entry in entries {
            if entry.sequence >= from_sequence {
                let event = LedgerEvent::entry_committed(entry);
                self.publish(event).await?;
                count += 1;
            }
        }

        // Notify subscribers of replay complete
        let complete_event = LedgerEvent::replay_completed(count);
        for subscriber in &self.subscribers {
            if let Err(e) = subscriber.on_replay_complete().await {
                error!("Subscriber '{}' failed on replay complete: {}", subscriber.name(), e);
            }
        }
        let _ = self.sender.send(complete_event);

        info!("Replay completed: {} entries", count);
        Ok(count)
    }

    /// Get a receiver for the broadcast channel
    ///
    /// This can be used by external consumers that want to receive events.
    pub fn receiver(&self) -> broadcast::Receiver<LedgerEvent> {
        self.sender.subscribe()
    }

    /// Get an event reader for direct JSONL access
    pub fn reader(&self) -> Result<EventReader, bibank_events::EventError> {
        EventReader::from_directory(&self.journal_path)
    }

    /// Get the journal path
    pub fn journal_path(&self) -> &Path {
        &self.journal_path
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    struct CountingSubscriber {
        name: String,
        count: AtomicUsize,
    }

    impl CountingSubscriber {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                count: AtomicUsize::new(0),
            }
        }

        fn count(&self) -> usize {
            self.count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl EventSubscriber for CountingSubscriber {
        fn name(&self) -> &str {
            &self.name
        }

        async fn handle(&self, _event: &LedgerEvent) -> Result<(), BusError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_publish_to_subscriber() {
        let dir = tempdir().unwrap();
        let mut bus = EventBus::new(dir.path());

        let subscriber = Arc::new(CountingSubscriber::new("test"));
        bus.subscribe(subscriber.clone());

        // Create a dummy event
        let event = LedgerEvent::ChainVerified {
            last_sequence: 1,
            last_hash: "abc".to_string(),
        };

        bus.publish(event).await.unwrap();

        assert_eq!(subscriber.count(), 1);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let dir = tempdir().unwrap();
        let mut bus = EventBus::new(dir.path());

        let sub1 = Arc::new(CountingSubscriber::new("sub1"));
        let sub2 = Arc::new(CountingSubscriber::new("sub2"));

        bus.subscribe(sub1.clone());
        bus.subscribe(sub2.clone());

        let event = LedgerEvent::ChainVerified {
            last_sequence: 1,
            last_hash: "abc".to_string(),
        };

        bus.publish(event).await.unwrap();

        assert_eq!(sub1.count(), 1);
        assert_eq!(sub2.count(), 1);
    }

    struct FailingSubscriber;

    #[async_trait]
    impl EventSubscriber for FailingSubscriber {
        fn name(&self) -> &str {
            "failing"
        }

        async fn handle(&self, _event: &LedgerEvent) -> Result<(), BusError> {
            Err(BusError::SubscriberFailed {
                name: "failing".to_string(),
                reason: "intentional failure".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn test_subscriber_failure_does_not_block() {
        let dir = tempdir().unwrap();
        let mut bus = EventBus::new(dir.path());

        let failing = Arc::new(FailingSubscriber);
        let counting = Arc::new(CountingSubscriber::new("counting"));

        bus.subscribe(failing);
        bus.subscribe(counting.clone());

        let event = LedgerEvent::ChainVerified {
            last_sequence: 1,
            last_hash: "abc".to_string(),
        };

        // Should not error even though one subscriber fails
        let result = bus.publish(event).await;
        assert!(result.is_ok());

        // The counting subscriber should still have received the event
        assert_eq!(counting.count(), 1);
    }
}

```

## File ./bibank\crates\bus\src\error.rs:
```rust
//! Event bus errors

use thiserror::Error;

/// Errors that can occur in the event bus
#[derive(Error, Debug)]
pub enum BusError {
    #[error("Failed to send event: {0}")]
    SendFailed(String),

    #[error("Subscriber '{name}' failed: {reason}")]
    SubscriberFailed { name: String, reason: String },

    #[error("Replay failed: {0}")]
    ReplayFailed(String),

    #[error("Event store error: {0}")]
    EventStoreError(#[from] bibank_events::EventError),

    #[error("Channel closed")]
    ChannelClosed,
}

```

## File ./bibank\crates\bus\src\event.rs:
```rust
//! Ledger events for pub/sub distribution

use bibank_ledger::JournalEntry;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events emitted by the ledger system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LedgerEvent {
    /// A journal entry was committed
    EntryCommitted {
        /// The committed entry
        entry: JournalEntry,
        /// When the event was published
        timestamp: DateTime<Utc>,
    },

    /// Hash chain was verified
    ChainVerified {
        /// Last verified sequence number
        last_sequence: u64,
        /// Hash of the last entry
        last_hash: String,
    },

    /// Replay has started
    ReplayStarted {
        /// Starting sequence number
        from_sequence: u64,
    },

    /// Replay has completed
    ReplayCompleted {
        /// Number of entries replayed
        entries_count: usize,
    },
}

impl LedgerEvent {
    /// Create an EntryCommitted event
    pub fn entry_committed(entry: JournalEntry) -> Self {
        Self::EntryCommitted {
            entry,
            timestamp: Utc::now(),
        }
    }

    /// Create a ChainVerified event
    pub fn chain_verified(last_sequence: u64, last_hash: String) -> Self {
        Self::ChainVerified {
            last_sequence,
            last_hash,
        }
    }

    /// Create a ReplayStarted event
    pub fn replay_started(from_sequence: u64) -> Self {
        Self::ReplayStarted { from_sequence }
    }

    /// Create a ReplayCompleted event
    pub fn replay_completed(entries_count: usize) -> Self {
        Self::ReplayCompleted { entries_count }
    }
}

```

## File ./bibank\crates\bus\src\lib.rs:
```rust
//! BiBank Event Bus - In-process async event distribution
//!
//! Distributes committed events to subscribers (projections, etc.)
//!
//! # Phase 2 Features
//! - Async pub/sub with tokio broadcast channel
//! - EventSubscriber trait for custom handlers
//! - Replay from JSONL (Source of Truth)
//! - No retention in bus - events only in JSONL

pub mod channel;
pub mod error;
pub mod event;
pub mod subscriber;

pub use channel::EventBus;
pub use error::BusError;
pub use event::LedgerEvent;
pub use subscriber::EventSubscriber;

```

## File ./bibank\crates\bus\src\subscriber.rs:
```rust
//! Event subscriber trait for async event handling

use crate::event::LedgerEvent;
use crate::error::BusError;
use async_trait::async_trait;

/// Trait for event subscribers
///
/// Subscribers receive events from the event bus and process them asynchronously.
/// Each subscriber should be idempotent (handle duplicate events gracefully).
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Get the subscriber name (for logging)
    fn name(&self) -> &str;

    /// Handle a ledger event
    ///
    /// This is called for each event published to the bus.
    /// The subscriber should process the event and return Ok(()) on success.
    async fn handle(&self, event: &LedgerEvent) -> Result<(), BusError>;

    /// Called when replay starts (optional)
    ///
    /// Subscribers can use this to prepare for bulk event processing.
    async fn on_replay_start(&self) -> Result<(), BusError> {
        Ok(())
    }

    /// Called when replay completes (optional)
    ///
    /// Subscribers can use this to finalize state after bulk processing.
    async fn on_replay_complete(&self) -> Result<(), BusError> {
        Ok(())
    }

    /// Get the last processed sequence (for replay optimization)
    ///
    /// Returns None if the subscriber doesn't track sequence numbers.
    fn last_processed_sequence(&self) -> Option<u64> {
        None
    }
}

```

## File ./bibank\crates\compliance\src\config.rs:
```rust
//! Compliance configuration with configurable thresholds
//!
//! All thresholds are configurable via file/env, not hardcoded.
//! This allows production tuning without recompilation.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the Compliance Engine
///
/// All thresholds can be overridden via environment variables or config file.
/// Defaults are conservative (stricter limits).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConfig {
    // === Thresholds ===
    /// Large transaction threshold (triggers FLAG)
    #[serde(default = "default_large_tx_threshold")]
    pub large_tx_threshold: Decimal,

    /// Currency Transaction Report threshold (regulatory requirement)
    #[serde(default = "default_ctr_threshold")]
    pub ctr_threshold: Decimal,

    /// Structuring detection threshold (just below CTR)
    #[serde(default = "default_structuring_threshold")]
    pub structuring_threshold: Decimal,

    /// Number of transactions to trigger structuring alert
    #[serde(default = "default_structuring_tx_count")]
    pub structuring_tx_count: u32,

    /// Account age considered "new" (in days)
    #[serde(default = "default_new_account_days")]
    pub new_account_days: i64,

    // === Velocity Windows ===
    /// Time window for velocity checks (in minutes)
    #[serde(default = "default_velocity_window_minutes")]
    pub velocity_window_minutes: u32,

    /// Transaction count threshold for velocity alert
    #[serde(default = "default_velocity_tx_threshold")]
    pub velocity_tx_threshold: u32,

    // === External Services ===
    /// Timeout for external service calls (KYC, Watchlist)
    #[serde(default = "default_external_timeout_ms")]
    pub external_timeout_ms: u64,

    /// Cache TTL for external data
    #[serde(default = "default_external_cache_ttl_secs")]
    pub external_cache_ttl_secs: u64,

    /// Policy when external service fails
    #[serde(default)]
    pub external_fail_policy: FailPolicy,

    // === Review Settings ===
    /// Hours until flagged transaction expires (auto-reject)
    #[serde(default = "default_review_expiry_hours")]
    pub review_expiry_hours: u64,
}

/// Policy when external service (KYC, Watchlist) fails
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailPolicy {
    /// Block transaction if external check fails (SAFER - DEFAULT)
    /// Rationale: False positive > False negative for compliance
    #[default]
    FailClosed,

    /// Allow transaction but flag for review (RISKIER)
    /// Use only when external service is unreliable
    FailOpen,
}

// Default value functions for serde
fn default_large_tx_threshold() -> Decimal {
    Decimal::new(10_000, 0)
}

fn default_ctr_threshold() -> Decimal {
    Decimal::new(10_000, 0)
}

fn default_structuring_threshold() -> Decimal {
    Decimal::new(9_000, 0)
}

fn default_structuring_tx_count() -> u32 {
    3
}

fn default_new_account_days() -> i64 {
    7
}

fn default_velocity_window_minutes() -> u32 {
    60
}

fn default_velocity_tx_threshold() -> u32 {
    5
}

fn default_external_timeout_ms() -> u64 {
    500
}

fn default_external_cache_ttl_secs() -> u64 {
    300 // 5 minutes
}

fn default_review_expiry_hours() -> u64 {
    72 // 3 days
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            large_tx_threshold: default_large_tx_threshold(),
            ctr_threshold: default_ctr_threshold(),
            structuring_threshold: default_structuring_threshold(),
            structuring_tx_count: default_structuring_tx_count(),
            new_account_days: default_new_account_days(),
            velocity_window_minutes: default_velocity_window_minutes(),
            velocity_tx_threshold: default_velocity_tx_threshold(),
            external_timeout_ms: default_external_timeout_ms(),
            external_cache_ttl_secs: default_external_cache_ttl_secs(),
            external_fail_policy: FailPolicy::default(),
            review_expiry_hours: default_review_expiry_hours(),
        }
    }
}

impl ComplianceConfig {
    /// Load configuration from JSON file
    pub fn from_file(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })
    }

    /// Get external timeout as Duration
    pub fn external_timeout(&self) -> Duration {
        Duration::from_millis(self.external_timeout_ms)
    }

    /// Get cache TTL as Duration
    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.external_cache_ttl_secs)
    }

    /// Get review expiry as chrono Duration
    pub fn review_expiry(&self) -> chrono::Duration {
        chrono::Duration::hours(self.review_expiry_hours as i64)
    }

    /// Get new account threshold as chrono Duration
    pub fn new_account_threshold(&self) -> chrono::Duration {
        chrono::Duration::days(self.new_account_days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ComplianceConfig::default();

        assert_eq!(config.large_tx_threshold, Decimal::new(10_000, 0));
        assert_eq!(config.ctr_threshold, Decimal::new(10_000, 0));
        assert_eq!(config.structuring_threshold, Decimal::new(9_000, 0));
        assert_eq!(config.structuring_tx_count, 3);
        assert_eq!(config.new_account_days, 7);
        assert_eq!(config.velocity_window_minutes, 60);
        assert_eq!(config.velocity_tx_threshold, 5);
        assert_eq!(config.external_timeout_ms, 500);
        assert_eq!(config.external_cache_ttl_secs, 300);
        assert_eq!(config.external_fail_policy, FailPolicy::FailClosed);
        assert_eq!(config.review_expiry_hours, 72);
    }

    #[test]
    fn test_fail_policy_default_is_closed() {
        let policy = FailPolicy::default();
        assert_eq!(policy, FailPolicy::FailClosed);
    }

    #[test]
    fn test_config_serialization() {
        let config = ComplianceConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();

        // Should contain our fields
        assert!(json.contains("large_tx_threshold"));
        assert!(json.contains("fail_closed"));

        // Should be deserializable
        let parsed: ComplianceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.large_tx_threshold, config.large_tx_threshold);
    }

    #[test]
    fn test_config_partial_json() {
        // Should use defaults for missing fields
        let json = r#"{ "large_tx_threshold": "5000" }"#;
        let config: ComplianceConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.large_tx_threshold, Decimal::new(5_000, 0));
        assert_eq!(config.ctr_threshold, Decimal::new(10_000, 0)); // default
    }

    #[test]
    fn test_duration_helpers() {
        let config = ComplianceConfig::default();

        assert_eq!(config.external_timeout(), Duration::from_millis(500));
        assert_eq!(config.cache_ttl(), Duration::from_secs(300));
        assert_eq!(config.review_expiry(), chrono::Duration::hours(72));
        assert_eq!(config.new_account_threshold(), chrono::Duration::days(7));
    }
}

```

## File ./bibank\crates\compliance\src\decision.rs:
```rust
//! AML Decision types with formal lattice ordering
//!
//! Decisions follow a formal ordering for aggregation:
//! `Approved < Flagged(L1) < Flagged(L2) < ... < Blocked`
//!
//! Aggregation: `max(all_decisions)` - most restrictive wins

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Risk score levels - ordered from lowest to highest
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskScore {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl PartialOrd for RiskScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RiskScore {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl Default for RiskScore {
    fn default() -> Self {
        RiskScore::Low
    }
}

/// Approval level required for flagged transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLevel {
    /// Single compliance officer
    L1 = 1,
    /// Senior compliance officer
    L2 = 2,
    /// Head of compliance
    L3 = 3,
    /// Board level (for critical cases)
    L4 = 4,
}

impl PartialOrd for ApprovalLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ApprovalLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl Default for ApprovalLevel {
    fn default() -> Self {
        ApprovalLevel::L1
    }
}

/// AML Decision - Formal Lattice
///
/// Ordering: Approved < Flagged < Blocked
/// For Flagged decisions, higher ApprovalLevel = more restrictive
///
/// Aggregation uses `max()` - most restrictive decision wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AmlDecision {
    /// Transaction approved, continue (lowest in lattice)
    Approved,

    /// Transaction flagged, requires manual review
    Flagged {
        reason: String,
        risk_score: RiskScore,
        required_approval: ApprovalLevel,
    },

    /// Transaction blocked (highest in lattice)
    Blocked {
        reason: String,
        compliance_code: String,
    },
}

impl AmlDecision {
    /// Create a new Flagged decision
    pub fn flagged(reason: impl Into<String>, risk_score: RiskScore, level: ApprovalLevel) -> Self {
        AmlDecision::Flagged {
            reason: reason.into(),
            risk_score,
            required_approval: level,
        }
    }

    /// Create a new Blocked decision
    pub fn blocked(reason: impl Into<String>, code: impl Into<String>) -> Self {
        AmlDecision::Blocked {
            reason: reason.into(),
            compliance_code: code.into(),
        }
    }

    /// Check if transaction is approved
    pub fn is_approved(&self) -> bool {
        matches!(self, AmlDecision::Approved)
    }

    /// Check if transaction is flagged
    pub fn is_flagged(&self) -> bool {
        matches!(self, AmlDecision::Flagged { .. })
    }

    /// Check if transaction is blocked
    pub fn is_blocked(&self) -> bool {
        matches!(self, AmlDecision::Blocked { .. })
    }

    /// Get numeric value for ordering
    fn order_value(&self) -> u8 {
        match self {
            AmlDecision::Approved => 0,
            AmlDecision::Flagged { required_approval, .. } => *required_approval as u8,
            AmlDecision::Blocked { .. } => 10, // Always highest
        }
    }

    /// Aggregate multiple decisions: take the most restrictive
    ///
    /// This is the core of the formal lattice - we always escalate
    /// to the highest restriction level.
    pub fn aggregate(decisions: impl IntoIterator<Item = AmlDecision>) -> AmlDecision {
        decisions
            .into_iter()
            .max()
            .unwrap_or(AmlDecision::Approved)
    }
}

impl PartialOrd for AmlDecision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AmlDecision {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_value().cmp(&other.order_value())
    }
}

impl Default for AmlDecision {
    fn default() -> Self {
        AmlDecision::Approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_score_ordering() {
        assert!(RiskScore::Low < RiskScore::Medium);
        assert!(RiskScore::Medium < RiskScore::High);
        assert!(RiskScore::High < RiskScore::Critical);
    }

    #[test]
    fn test_approval_level_ordering() {
        assert!(ApprovalLevel::L1 < ApprovalLevel::L2);
        assert!(ApprovalLevel::L2 < ApprovalLevel::L3);
        assert!(ApprovalLevel::L3 < ApprovalLevel::L4);
    }

    #[test]
    fn test_aml_decision_ordering() {
        let approved = AmlDecision::Approved;
        let flagged_l1 = AmlDecision::flagged("test", RiskScore::Low, ApprovalLevel::L1);
        let flagged_l2 = AmlDecision::flagged("test", RiskScore::High, ApprovalLevel::L2);
        let blocked = AmlDecision::blocked("test", "AML-001");

        // Approved < Flagged < Blocked
        assert!(approved < flagged_l1);
        assert!(flagged_l1 < flagged_l2);
        assert!(flagged_l2 < blocked);
    }

    #[test]
    fn test_aggregate_empty() {
        let result = AmlDecision::aggregate(vec![]);
        assert_eq!(result, AmlDecision::Approved);
    }

    #[test]
    fn test_aggregate_all_approved() {
        let decisions = vec![
            AmlDecision::Approved,
            AmlDecision::Approved,
            AmlDecision::Approved,
        ];
        let result = AmlDecision::aggregate(decisions);
        assert_eq!(result, AmlDecision::Approved);
    }

    #[test]
    fn test_aggregate_takes_most_restrictive() {
        let decisions = vec![
            AmlDecision::Approved,
            AmlDecision::flagged("test1", RiskScore::Low, ApprovalLevel::L1),
            AmlDecision::flagged("test2", RiskScore::High, ApprovalLevel::L2),
        ];
        let result = AmlDecision::aggregate(decisions);

        // Should take L2 flagged (highest)
        assert!(result.is_flagged());
        if let AmlDecision::Flagged { required_approval, .. } = result {
            assert_eq!(required_approval, ApprovalLevel::L2);
        }
    }

    #[test]
    fn test_aggregate_blocked_wins() {
        let decisions = vec![
            AmlDecision::Approved,
            AmlDecision::flagged("test", RiskScore::Critical, ApprovalLevel::L4),
            AmlDecision::blocked("blocked", "AML-001"),
        ];
        let result = AmlDecision::aggregate(decisions);

        assert!(result.is_blocked());
    }

    #[test]
    fn test_decision_serialization() {
        let flagged = AmlDecision::flagged("Large TX", RiskScore::High, ApprovalLevel::L2);
        let json = serde_json::to_string(&flagged).unwrap();

        assert!(json.contains("flagged"));
        assert!(json.contains("Large TX"));
        assert!(json.contains("high"));
        assert!(json.contains("l2"));

        let parsed: AmlDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, flagged);
    }

    #[test]
    fn test_blocked_serialization() {
        let blocked = AmlDecision::blocked("Sanctions match", "OFAC-001");
        let json = serde_json::to_string(&blocked).unwrap();

        assert!(json.contains("blocked"));
        assert!(json.contains("OFAC-001"));
    }
}

```

## File ./bibank\crates\compliance\src\engine.rs:
```rust
//! Compliance Engine - Main orchestrator
//!
//! Coordinates rule evaluation, decision aggregation, and ledger writes.

use chrono::Utc;
use rust_decimal::Decimal;

use crate::config::ComplianceConfig;
use crate::decision::{AmlDecision, ApprovalLevel, RiskScore};
use crate::error::ComplianceResult;
use crate::event::{ComplianceEvent, ReviewDecision};
use crate::ledger::ComplianceLedger;
use crate::state::ComplianceState;

/// Transaction context for rule evaluation
#[derive(Debug, Clone)]
pub struct TransactionContext {
    /// Unique transaction ID
    pub correlation_id: String,
    /// User initiating the transaction
    pub user_id: String,
    /// Transaction type
    pub intent: String,
    /// Amount
    pub amount: Decimal,
    /// Asset (e.g., "USDT", "BTC")
    pub asset: String,
    /// Account age in days (if known)
    pub account_age_days: Option<i64>,
}

/// Result of a compliance check
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// The final aggregated decision
    pub decision: AmlDecision,
    /// Rules that were triggered
    pub rules_triggered: Vec<String>,
    /// Risk score (if any)
    pub risk_score: Option<RiskScore>,
}

/// Main Compliance Engine
///
/// Orchestrates:
/// - Rule evaluation
/// - Decision aggregation (lattice max)
/// - Compliance Ledger writes
/// - In-memory state updates
pub struct ComplianceEngine {
    /// Configuration (thresholds, etc.)
    config: ComplianceConfig,
    /// Compliance Ledger (append-only)
    ledger: ComplianceLedger,
    /// In-memory state (sliding window)
    state: ComplianceState,
}

impl ComplianceEngine {
    /// Create a new Compliance Engine
    pub fn new(config: ComplianceConfig, ledger: ComplianceLedger) -> Self {
        Self {
            config,
            ledger,
            state: ComplianceState::new(),
        }
    }

    /// Create an engine with in-memory ledger (for testing)
    pub fn in_memory() -> Self {
        Self::new(ComplianceConfig::default(), ComplianceLedger::in_memory())
    }

    /// Check a transaction against all rules
    pub fn check_transaction(&mut self, ctx: &TransactionContext) -> ComplianceResult<CheckResult> {
        let mut decisions = Vec::new();
        let mut rules_triggered = Vec::new();

        // === BLOCK Rules (Pre-commit) ===
        // These would reject the transaction immediately

        // Rule: KYC limit check (simplified - would need real KYC data)
        // Skipped in this implementation - would integrate with KYC provider

        // === FLAG Rules (Post-commit) ===

        // Rule: Large transaction
        if ctx.amount >= self.config.large_tx_threshold {
            rules_triggered.push("LARGE_TX_ALERT".to_string());
            decisions.push(AmlDecision::flagged(
                format!("Large transaction: {} {}", ctx.amount, ctx.asset),
                RiskScore::Medium,
                ApprovalLevel::L1,
            ));
        }

        // Rule: CTR threshold
        if ctx.amount >= self.config.ctr_threshold {
            rules_triggered.push("CTR_THRESHOLD".to_string());
            // CTR is a reporting requirement, not necessarily a flag
            // But we log it
        }

        // Rule: Structuring detection
        let tx_count = self.state.tx_count_in_last(&ctx.user_id, self.config.velocity_window_minutes);
        let volume = self.state.volume_in_last(&ctx.user_id, &ctx.asset, self.config.velocity_window_minutes);

        if tx_count >= self.config.structuring_tx_count
            && volume >= self.config.structuring_threshold
            && volume < self.config.ctr_threshold
        {
            rules_triggered.push("STRUCTURING_DETECTION".to_string());
            decisions.push(AmlDecision::flagged(
                "Potential structuring pattern detected",
                RiskScore::High,
                ApprovalLevel::L2,
            ));
        }

        // Rule: Velocity check
        if tx_count >= self.config.velocity_tx_threshold {
            rules_triggered.push("VELOCITY_ALERT".to_string());
            decisions.push(AmlDecision::flagged(
                format!("High transaction velocity: {} tx in {}min", tx_count, self.config.velocity_window_minutes),
                RiskScore::Medium,
                ApprovalLevel::L1,
            ));
        }

        // Rule: New account large transaction
        if let Some(age_days) = ctx.account_age_days {
            if age_days < self.config.new_account_days
                && ctx.amount >= self.config.large_tx_threshold / Decimal::new(2, 0)
            {
                rules_triggered.push("NEW_ACCOUNT_LARGE_TX".to_string());
                decisions.push(AmlDecision::flagged(
                    format!("New account ({} days) large transaction", age_days),
                    RiskScore::Medium,
                    ApprovalLevel::L1,
                ));
            }
        }

        // Aggregate decisions (most restrictive wins)
        let decision = AmlDecision::aggregate(decisions);

        // Determine risk score from decision
        let risk_score = match &decision {
            AmlDecision::Flagged { risk_score, .. } => Some(*risk_score),
            AmlDecision::Blocked { .. } => Some(RiskScore::Critical),
            AmlDecision::Approved => None,
        };

        // Write to compliance ledger
        let event = ComplianceEvent::CheckPerformed {
            id: uuid::Uuid::new_v4().to_string(),
            correlation_id: ctx.correlation_id.clone(),
            user_id: ctx.user_id.clone(),
            decision: decision.clone(),
            rules_triggered: rules_triggered.clone(),
            risk_score,
            timestamp: Utc::now(),
        };
        self.ledger.append(&event)?;

        // Update in-memory state
        self.state.record_transaction(&ctx.user_id, &ctx.asset, ctx.amount);

        // If flagged, also write a TransactionFlagged event
        if let AmlDecision::Flagged { reason, required_approval, .. } = &decision {
            let expires_at = Utc::now() + self.config.review_expiry();
            let flag_event = ComplianceEvent::transaction_flagged(
                &ctx.correlation_id,
                &ctx.user_id,
                reason,
                *required_approval,
                expires_at,
            );
            self.ledger.append(&flag_event)?;
        }

        Ok(CheckResult {
            decision,
            rules_triggered,
            risk_score,
        })
    }

    /// Record a review decision
    pub fn record_review(
        &mut self,
        flag_id: &str,
        decision: ReviewDecision,
        reviewer_id: &str,
        notes: &str,
    ) -> ComplianceResult<()> {
        let event = ComplianceEvent::review_completed(flag_id, decision, reviewer_id, notes);
        self.ledger.append(&event)?;
        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &ComplianceConfig {
        &self.config
    }

    /// Get the in-memory state (for queries)
    pub fn state(&self) -> &ComplianceState {
        &self.state
    }

    /// Rebuild state from ledger (for startup/recovery)
    pub fn rebuild_state(&mut self) -> ComplianceResult<usize> {
        self.state.clear();

        let events = self.ledger.read_all()?;
        let count = events.len();

        for event in events {
            if let ComplianceEvent::CheckPerformed { user_id, timestamp, .. } = event {
                // We don't have amount/asset in the event, so we just record presence
                // In a real implementation, we'd store more data
                self.state.record_transaction_at(&user_id, "UNKNOWN", Decimal::ZERO, timestamp);
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_ctx(amount: Decimal) -> TransactionContext {
        TransactionContext {
            correlation_id: uuid::Uuid::new_v4().to_string(),
            user_id: "USER-001".to_string(),
            intent: "Deposit".to_string(),
            amount,
            asset: "USDT".to_string(),
            account_age_days: Some(30),
        }
    }

    #[test]
    fn test_small_transaction_approved() {
        let mut engine = ComplianceEngine::in_memory();

        let ctx = create_ctx(dec!(100));
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.decision.is_approved());
        assert!(result.rules_triggered.is_empty());
    }

    #[test]
    fn test_large_transaction_flagged() {
        let mut engine = ComplianceEngine::in_memory();

        let ctx = create_ctx(dec!(15000));
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.decision.is_flagged());
        assert!(result.rules_triggered.contains(&"LARGE_TX_ALERT".to_string()));
        assert!(result.rules_triggered.contains(&"CTR_THRESHOLD".to_string()));
    }

    #[test]
    fn test_velocity_check() {
        let mut engine = ComplianceEngine::in_memory();

        // Make 5 transactions (velocity threshold)
        for i in 0..5 {
            let ctx = TransactionContext {
                correlation_id: format!("TX-{}", i),
                user_id: "USER-001".to_string(),
                intent: "Deposit".to_string(),
                amount: dec!(100),
                asset: "USDT".to_string(),
                account_age_days: Some(30),
            };
            engine.check_transaction(&ctx).unwrap();
        }

        // 6th transaction should trigger velocity alert
        let ctx = create_ctx(dec!(100));
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.rules_triggered.contains(&"VELOCITY_ALERT".to_string()));
    }

    #[test]
    fn test_structuring_detection() {
        let mut engine = ComplianceEngine::in_memory();

        // Make 3 transactions totaling just under CTR threshold
        for i in 0..3 {
            let ctx = TransactionContext {
                correlation_id: format!("TX-{}", i),
                user_id: "USER-001".to_string(),
                intent: "Deposit".to_string(),
                amount: dec!(3000), // 3 x 3000 = 9000 (under 10000 CTR)
                asset: "USDT".to_string(),
                account_age_days: Some(30),
            };
            engine.check_transaction(&ctx).unwrap();
        }

        // 4th transaction should trigger structuring
        let ctx = TransactionContext {
            correlation_id: "TX-3".to_string(),
            user_id: "USER-001".to_string(),
            intent: "Deposit".to_string(),
            amount: dec!(500),
            asset: "USDT".to_string(),
            account_age_days: Some(30),
        };
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.rules_triggered.contains(&"STRUCTURING_DETECTION".to_string()));
    }

    #[test]
    fn test_new_account_large_tx() {
        let mut engine = ComplianceEngine::in_memory();

        // New account (3 days) with large-ish transaction
        let ctx = TransactionContext {
            correlation_id: "TX-001".to_string(),
            user_id: "NEW-USER".to_string(),
            intent: "Deposit".to_string(),
            amount: dec!(6000), // >= 10000/2 = 5000
            asset: "USDT".to_string(),
            account_age_days: Some(3), // < 7 days
        };
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.rules_triggered.contains(&"NEW_ACCOUNT_LARGE_TX".to_string()));
    }

    #[test]
    fn test_decision_aggregation() {
        let mut engine = ComplianceEngine::in_memory();

        // Transaction that triggers multiple rules
        // Large + new account = should take highest level
        let ctx = TransactionContext {
            correlation_id: "TX-001".to_string(),
            user_id: "NEW-USER".to_string(),
            intent: "Deposit".to_string(),
            amount: dec!(15000), // Large
            asset: "USDT".to_string(),
            account_age_days: Some(2), // New account
        };
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.decision.is_flagged());
        // Multiple rules triggered
        assert!(result.rules_triggered.len() >= 2);
    }

    #[test]
    fn test_record_review() {
        let mut engine = ComplianceEngine::in_memory();

        engine
            .record_review(
                "FLAG-001",
                ReviewDecision::Approved,
                "OFFICER-001",
                "Verified source of funds",
            )
            .unwrap();
    }

    #[test]
    fn test_config_access() {
        let engine = ComplianceEngine::in_memory();
        assert_eq!(engine.config().large_tx_threshold, dec!(10000));
    }

    #[test]
    fn test_state_access() {
        let mut engine = ComplianceEngine::in_memory();

        let ctx = create_ctx(dec!(100));
        engine.check_transaction(&ctx).unwrap();

        assert!(engine.state().has_user("USER-001"));
    }
}

```

## File ./bibank\crates\compliance\src\error.rs:
```rust
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

```

## File ./bibank\crates\compliance\src\event.rs:
```rust
//! Compliance events (written to Compliance Ledger)
//!
//! These events form the Decision Truth - separate from financial truth.
//! All events are append-only and immutable.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::decision::{AmlDecision, ApprovalLevel, RiskScore};

/// Events appended to Compliance Ledger (append-only JSONL)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ComplianceEvent {
    /// Transaction was checked against rules
    CheckPerformed {
        id: String,
        correlation_id: String,
        user_id: String,
        decision: AmlDecision,
        rules_triggered: Vec<String>,
        risk_score: Option<RiskScore>,
        timestamp: DateTime<Utc>,
    },

    /// Transaction was flagged for review
    TransactionFlagged {
        id: String,
        correlation_id: String,
        user_id: String,
        reason: String,
        required_approval: ApprovalLevel,
        expires_at: DateTime<Utc>,
        timestamp: DateTime<Utc>,
    },

    /// Review decision made
    ReviewCompleted {
        id: String,
        flag_id: String,
        decision: ReviewDecision,
        reviewer_id: String,
        notes: String,
        timestamp: DateTime<Utc>,
    },

    /// Rule set activated/deactivated
    RuleSetChanged {
        id: String,
        rule_set_name: String,
        rule_set_version: String,
        rule_set_hash: String,
        action: RuleAction,
        performed_by: String,
        approved_by: Vec<String>,
        timestamp: DateTime<Utc>,
    },

    /// User added to internal watchlist
    WatchlistUpdated {
        id: String,
        user_id: String,
        action: WatchlistAction,
        reason: String,
        performed_by: String,
        timestamp: DateTime<Utc>,
    },
}

/// Review decision outcomes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    /// Transaction approved after review
    Approved,
    /// Transaction rejected after review
    Rejected,
    /// Review expired (auto-reject)
    Expired,
}

/// Rule set actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    /// Rule set activated
    Activated,
    /// Rule set deactivated
    Deactivated,
}

/// Watchlist actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchlistAction {
    /// User added to watchlist
    Added,
    /// User removed from watchlist
    Removed,
}

impl ComplianceEvent {
    /// Get the event ID
    pub fn id(&self) -> &str {
        match self {
            ComplianceEvent::CheckPerformed { id, .. } => id,
            ComplianceEvent::TransactionFlagged { id, .. } => id,
            ComplianceEvent::ReviewCompleted { id, .. } => id,
            ComplianceEvent::RuleSetChanged { id, .. } => id,
            ComplianceEvent::WatchlistUpdated { id, .. } => id,
        }
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            ComplianceEvent::CheckPerformed { timestamp, .. } => *timestamp,
            ComplianceEvent::TransactionFlagged { timestamp, .. } => *timestamp,
            ComplianceEvent::ReviewCompleted { timestamp, .. } => *timestamp,
            ComplianceEvent::RuleSetChanged { timestamp, .. } => *timestamp,
            ComplianceEvent::WatchlistUpdated { timestamp, .. } => *timestamp,
        }
    }

    /// Get the user ID if applicable
    pub fn user_id(&self) -> Option<&str> {
        match self {
            ComplianceEvent::CheckPerformed { user_id, .. } => Some(user_id),
            ComplianceEvent::TransactionFlagged { user_id, .. } => Some(user_id),
            ComplianceEvent::ReviewCompleted { .. } => None,
            ComplianceEvent::RuleSetChanged { .. } => None,
            ComplianceEvent::WatchlistUpdated { user_id, .. } => Some(user_id),
        }
    }

    /// Create a new CheckPerformed event
    pub fn check_performed(
        correlation_id: impl Into<String>,
        user_id: impl Into<String>,
        decision: AmlDecision,
        rules_triggered: Vec<String>,
    ) -> Self {
        ComplianceEvent::CheckPerformed {
            id: uuid::Uuid::new_v4().to_string(),
            correlation_id: correlation_id.into(),
            user_id: user_id.into(),
            decision,
            rules_triggered,
            risk_score: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a new TransactionFlagged event
    pub fn transaction_flagged(
        correlation_id: impl Into<String>,
        user_id: impl Into<String>,
        reason: impl Into<String>,
        required_approval: ApprovalLevel,
        expires_at: DateTime<Utc>,
    ) -> Self {
        ComplianceEvent::TransactionFlagged {
            id: uuid::Uuid::new_v4().to_string(),
            correlation_id: correlation_id.into(),
            user_id: user_id.into(),
            reason: reason.into(),
            required_approval,
            expires_at,
            timestamp: Utc::now(),
        }
    }

    /// Create a new ReviewCompleted event
    pub fn review_completed(
        flag_id: impl Into<String>,
        decision: ReviewDecision,
        reviewer_id: impl Into<String>,
        notes: impl Into<String>,
    ) -> Self {
        ComplianceEvent::ReviewCompleted {
            id: uuid::Uuid::new_v4().to_string(),
            flag_id: flag_id.into(),
            decision,
            reviewer_id: reviewer_id.into(),
            notes: notes.into(),
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_performed_serialization() {
        let event = ComplianceEvent::check_performed(
            "TX-123",
            "USER-001",
            AmlDecision::Approved,
            vec!["RULE_1".to_string()],
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("check_performed"));
        assert!(json.contains("TX-123"));
        assert!(json.contains("USER-001"));

        let parsed: ComplianceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), event.id());
    }

    #[test]
    fn test_transaction_flagged_serialization() {
        let expires_at = Utc::now() + chrono::Duration::hours(72);
        let event = ComplianceEvent::transaction_flagged(
            "TX-456",
            "USER-002",
            "Large transaction",
            ApprovalLevel::L2,
            expires_at,
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("transaction_flagged"));
        assert!(json.contains("Large transaction"));
        assert!(json.contains("l2"));
    }

    #[test]
    fn test_review_decision_serialization() {
        let approved = ReviewDecision::Approved;
        let rejected = ReviewDecision::Rejected;
        let expired = ReviewDecision::Expired;

        assert_eq!(
            serde_json::to_string(&approved).unwrap(),
            "\"approved\""
        );
        assert_eq!(
            serde_json::to_string(&rejected).unwrap(),
            "\"rejected\""
        );
        assert_eq!(
            serde_json::to_string(&expired).unwrap(),
            "\"expired\""
        );
    }

    #[test]
    fn test_event_accessors() {
        let event = ComplianceEvent::check_performed(
            "TX-789",
            "USER-003",
            AmlDecision::Approved,
            vec![],
        );

        assert!(!event.id().is_empty());
        assert_eq!(event.user_id(), Some("USER-003"));
        assert!(event.timestamp() <= Utc::now());
    }
}

```

## File ./bibank\crates\compliance\src\ledger.rs:
```rust
//! Compliance Ledger - Append-only JSONL storage
//!
//! This is the "Decision Truth" ledger, separate from the main financial ledger.
//! All writes are append-only and immutable.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::error::ComplianceResult;
use crate::event::ComplianceEvent;

/// Append-only JSONL ledger for compliance events
///
/// Each line is a JSON-serialized ComplianceEvent.
/// The file is append-only and should never be modified.
pub struct ComplianceLedger {
    path: PathBuf,
    file: Option<File>,
}

impl ComplianceLedger {
    /// Create a new ledger at the given path
    pub fn new(path: impl AsRef<Path>) -> ComplianceResult<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        Ok(Self {
            path,
            file: Some(file),
        })
    }

    /// Create an in-memory ledger (for testing)
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            file: None,
        }
    }

    /// Append an event to the ledger
    pub fn append(&mut self, event: &ComplianceEvent) -> ComplianceResult<()> {
        if let Some(ref mut file) = self.file {
            let json = serde_json::to_string(event)?;
            writeln!(file, "{}", json)?;
            file.flush()?;
            Ok(())
        } else {
            // In-memory mode - just validate serialization
            let _ = serde_json::to_string(event)?;
            Ok(())
        }
    }

    /// Read all events from the ledger
    pub fn read_all(&self) -> ComplianceResult<Vec<ComplianceEvent>> {
        if self.file.is_none() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: ComplianceEvent = serde_json::from_str(&line)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Read events from a specific point (by line number)
    pub fn read_from(&self, start_line: usize) -> ComplianceResult<Vec<ComplianceEvent>> {
        if self.file.is_none() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            if i < start_line {
                continue;
            }
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: ComplianceEvent = serde_json::from_str(&line)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get the current line count (for checkpointing)
    pub fn line_count(&self) -> ComplianceResult<usize> {
        if self.file.is_none() {
            return Ok(0);
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        Ok(reader.lines().count())
    }

    /// Get the path to the ledger file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if this is an in-memory ledger
    pub fn is_in_memory(&self) -> bool {
        self.file.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::AmlDecision;
    use tempfile::tempdir;

    #[test]
    fn test_in_memory_ledger() {
        let mut ledger = ComplianceLedger::in_memory();

        let event = ComplianceEvent::check_performed(
            "TX-001",
            "USER-001",
            AmlDecision::Approved,
            vec![],
        );

        ledger.append(&event).unwrap();

        assert!(ledger.is_in_memory());
        assert_eq!(ledger.read_all().unwrap().len(), 0); // In-memory doesn't store
    }

    #[test]
    fn test_file_ledger_write_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("compliance.jsonl");

        let event1 = ComplianceEvent::check_performed(
            "TX-001",
            "USER-001",
            AmlDecision::Approved,
            vec!["RULE_1".to_string()],
        );
        let event2 = ComplianceEvent::check_performed(
            "TX-002",
            "USER-002",
            AmlDecision::blocked("Sanctions", "OFAC-001"),
            vec!["WATCHLIST".to_string()],
        );

        // Write events
        {
            let mut ledger = ComplianceLedger::new(&path).unwrap();
            ledger.append(&event1).unwrap();
            ledger.append(&event2).unwrap();
        }

        // Read events
        {
            let ledger = ComplianceLedger::new(&path).unwrap();
            let events = ledger.read_all().unwrap();

            assert_eq!(events.len(), 2);
            assert_eq!(events[0].id(), event1.id());
            assert_eq!(events[1].id(), event2.id());
        }
    }

    #[test]
    fn test_line_count() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("compliance.jsonl");

        let mut ledger = ComplianceLedger::new(&path).unwrap();

        assert_eq!(ledger.line_count().unwrap(), 0);

        ledger
            .append(&ComplianceEvent::check_performed(
                "TX-001",
                "USER-001",
                AmlDecision::Approved,
                vec![],
            ))
            .unwrap();

        // Need to re-read to get updated count
        let ledger = ComplianceLedger::new(&path).unwrap();
        assert_eq!(ledger.line_count().unwrap(), 1);
    }

    #[test]
    fn test_read_from_offset() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("compliance.jsonl");

        // Write 5 events
        {
            let mut ledger = ComplianceLedger::new(&path).unwrap();
            for i in 0..5 {
                ledger
                    .append(&ComplianceEvent::check_performed(
                        format!("TX-{:03}", i),
                        "USER-001",
                        AmlDecision::Approved,
                        vec![],
                    ))
                    .unwrap();
            }
        }

        // Read from offset 3
        {
            let ledger = ComplianceLedger::new(&path).unwrap();
            let events = ledger.read_from(3).unwrap();

            assert_eq!(events.len(), 2);
        }
    }

    #[test]
    fn test_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("compliance.jsonl");

        let ledger = ComplianceLedger::new(&path).unwrap();
        assert!(!ledger.is_in_memory());
        assert!(path.parent().unwrap().exists());
    }
}

```

## File ./bibank\crates\compliance\src\lib.rs:
```rust
//! BiBank Compliance Engine
//!
//! Phase 4: AML Real-time Hooks, Rule DSL, KYC Integration
//!
//! ## Architecture (Dual Ledger)
//!
//! ```text
//! Main Journal Ledger (JSONL)    Compliance Ledger (JSONL)
//! ├── Financial truth            ├── Decision truth
//! ├── DepositConfirmed           ├── TransactionFlagged
//! ├── TradeExecuted              ├── ReviewApproved
//! └── LockApplied ◄──────────────┴── ComplianceIntent
//!         │                              │
//!         └──────────┬───────────────────┘
//!                    ▼
//!             SQLite (Projection)
//! ```
//!
//! ## Key Components
//!
//! - [`config::ComplianceConfig`] - Configurable thresholds (not hardcoded)
//! - [`state::ComplianceState`] - In-memory sliding window for O(1) velocity checks
//! - [`decision::AmlDecision`] - Formal lattice with `max()` aggregation
//! - [`ledger::ComplianceLedger`] - Append-only JSONL ledger
//! - [`engine::ComplianceEngine`] - Main orchestrator

pub mod config;
pub mod decision;
pub mod engine;
pub mod error;
pub mod event;
pub mod ledger;
pub mod state;

pub use config::{ComplianceConfig, FailPolicy};
pub use decision::{AmlDecision, ApprovalLevel, RiskScore};
pub use engine::{CheckResult, ComplianceEngine};
pub use error::ComplianceError;
pub use event::{ComplianceEvent, ReviewDecision, RuleAction};
pub use ledger::ComplianceLedger;
pub use state::ComplianceState;

```

## File ./bibank\crates\compliance\src\state.rs:
```rust
//! In-memory compliance state with sliding window
//!
//! Provides O(1) queries for velocity checks like:
//! - `user.transactions_in_last(1.hour)`
//! - `user.total_volume_in_last(1.hour)`
//!
//! Uses circular buffer with minute-granularity buckets.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Number of buckets (1 per minute for 60-minute window)
const BUCKET_COUNT: usize = 60;

/// In-memory state for fast compliance checks
///
/// Rebuilt from Compliance Ledger events on startup.
/// All queries are O(1).
#[derive(Debug, Default)]
pub struct ComplianceState {
    /// Sliding window aggregates per user
    windows: HashMap<String, TransactionWindow>,
}

/// Sliding window for a single user
#[derive(Debug)]
pub struct TransactionWindow {
    /// Circular buffer: each bucket = 1 minute
    buckets: [Bucket; BUCKET_COUNT],
    /// Last update timestamp (for bucket rotation)
    last_update: DateTime<Utc>,
}

/// Single time bucket (1 minute of data)
#[derive(Debug, Default, Clone)]
pub struct Bucket {
    /// Transaction count in this bucket
    pub tx_count: u32,
    /// Volume per asset in this bucket
    pub volume: HashMap<String, Decimal>,
}

impl Default for TransactionWindow {
    fn default() -> Self {
        Self {
            buckets: std::array::from_fn(|_| Bucket::default()),
            last_update: Utc::now(),
        }
    }
}

impl TransactionWindow {
    /// Get the bucket index for a given timestamp
    fn bucket_index(timestamp: DateTime<Utc>) -> usize {
        let minutes = timestamp.timestamp() / 60;
        (minutes as usize) % BUCKET_COUNT
    }

    /// Rotate buckets to current time, clearing expired ones
    fn rotate_to_now(&mut self, now: DateTime<Utc>) {
        let last_idx = Self::bucket_index(self.last_update);
        let current_idx = Self::bucket_index(now);

        // Calculate how many minutes have passed
        let elapsed_minutes = (now - self.last_update).num_minutes();

        if elapsed_minutes >= BUCKET_COUNT as i64 {
            // All buckets expired, clear everything
            for bucket in &mut self.buckets {
                *bucket = Bucket::default();
            }
        } else if elapsed_minutes > 0 {
            // Clear buckets between last update and now
            let mut idx = (last_idx + 1) % BUCKET_COUNT;
            let count = elapsed_minutes.min(BUCKET_COUNT as i64) as usize;

            for _ in 0..count {
                self.buckets[idx] = Bucket::default();
                idx = (idx + 1) % BUCKET_COUNT;
            }
        }

        self.last_update = now;
        // Clear current bucket if it's a new minute
        if last_idx != current_idx {
            self.buckets[current_idx] = Bucket::default();
        }
    }

    /// Record a transaction
    pub fn record(&mut self, asset: &str, amount: Decimal, timestamp: DateTime<Utc>) {
        self.rotate_to_now(timestamp);
        let idx = Self::bucket_index(timestamp);

        self.buckets[idx].tx_count += 1;
        *self.buckets[idx]
            .volume
            .entry(asset.to_string())
            .or_insert(Decimal::ZERO) += amount;
    }

    /// Get transaction count in last N minutes
    pub fn tx_count_in_last(&self, minutes: u32, now: DateTime<Utc>) -> u32 {
        let minutes = minutes.min(BUCKET_COUNT as u32) as usize;
        let current_idx = Self::bucket_index(now);
        let cutoff = now - Duration::minutes(minutes as i64);

        let mut count = 0;
        for i in 0..minutes {
            let idx = (current_idx + BUCKET_COUNT - i) % BUCKET_COUNT;
            let bucket_time = now - Duration::minutes(i as i64);

            // Only count if bucket is within our window
            if bucket_time >= cutoff && bucket_time <= now {
                // Check if bucket is not stale
                let bucket_age = (now - self.last_update).num_minutes();
                if bucket_age < BUCKET_COUNT as i64 {
                    count += self.buckets[idx].tx_count;
                }
            }
        }
        count
    }

    /// Get total volume for an asset in last N minutes
    pub fn volume_in_last(&self, asset: &str, minutes: u32, now: DateTime<Utc>) -> Decimal {
        let minutes = minutes.min(BUCKET_COUNT as u32) as usize;
        let current_idx = Self::bucket_index(now);
        let cutoff = now - Duration::minutes(minutes as i64);

        let mut total = Decimal::ZERO;
        for i in 0..minutes {
            let idx = (current_idx + BUCKET_COUNT - i) % BUCKET_COUNT;
            let bucket_time = now - Duration::minutes(i as i64);

            if bucket_time >= cutoff && bucket_time <= now {
                let bucket_age = (now - self.last_update).num_minutes();
                if bucket_age < BUCKET_COUNT as i64 {
                    if let Some(vol) = self.buckets[idx].volume.get(asset) {
                        total += vol;
                    }
                }
            }
        }
        total
    }
}

impl ComplianceState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a transaction for a user
    pub fn record_transaction(
        &mut self,
        user_id: &str,
        asset: &str,
        amount: Decimal,
    ) {
        self.record_transaction_at(user_id, asset, amount, Utc::now());
    }

    /// Record a transaction at a specific time (for replay)
    pub fn record_transaction_at(
        &mut self,
        user_id: &str,
        asset: &str,
        amount: Decimal,
        timestamp: DateTime<Utc>,
    ) {
        self.windows
            .entry(user_id.to_string())
            .or_default()
            .record(asset, amount, timestamp);
    }

    /// Get transaction count in last N minutes for a user
    pub fn tx_count_in_last(&self, user_id: &str, minutes: u32) -> u32 {
        self.windows
            .get(user_id)
            .map(|w| w.tx_count_in_last(minutes, Utc::now()))
            .unwrap_or(0)
    }

    /// Get total volume in last N minutes for a user and asset
    pub fn volume_in_last(&self, user_id: &str, asset: &str, minutes: u32) -> Decimal {
        self.windows
            .get(user_id)
            .map(|w| w.volume_in_last(asset, minutes, Utc::now()))
            .unwrap_or(Decimal::ZERO)
    }

    /// Check if user has any recorded transactions
    pub fn has_user(&self, user_id: &str) -> bool {
        self.windows.contains_key(user_id)
    }

    /// Get number of tracked users
    pub fn user_count(&self) -> usize {
        self.windows.len()
    }

    /// Clear all state (for testing)
    pub fn clear(&mut self) {
        self.windows.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_empty_state() {
        let state = ComplianceState::new();
        assert_eq!(state.tx_count_in_last("USER-001", 60), 0);
        assert_eq!(state.volume_in_last("USER-001", "USDT", 60), Decimal::ZERO);
    }

    #[test]
    fn test_record_single_transaction() {
        let mut state = ComplianceState::new();

        state.record_transaction("USER-001", "USDT", dec!(1000));

        assert_eq!(state.tx_count_in_last("USER-001", 60), 1);
        assert_eq!(state.volume_in_last("USER-001", "USDT", 60), dec!(1000));
        assert_eq!(state.volume_in_last("USER-001", "BTC", 60), Decimal::ZERO);
    }

    #[test]
    fn test_record_multiple_transactions() {
        let mut state = ComplianceState::new();

        state.record_transaction("USER-001", "USDT", dec!(1000));
        state.record_transaction("USER-001", "USDT", dec!(2000));
        state.record_transaction("USER-001", "BTC", dec!(0.5));

        assert_eq!(state.tx_count_in_last("USER-001", 60), 3);
        assert_eq!(state.volume_in_last("USER-001", "USDT", 60), dec!(3000));
        assert_eq!(state.volume_in_last("USER-001", "BTC", 60), dec!(0.5));
    }

    #[test]
    fn test_multiple_users() {
        let mut state = ComplianceState::new();

        state.record_transaction("USER-001", "USDT", dec!(1000));
        state.record_transaction("USER-002", "USDT", dec!(5000));

        assert_eq!(state.tx_count_in_last("USER-001", 60), 1);
        assert_eq!(state.tx_count_in_last("USER-002", 60), 1);
        assert_eq!(state.volume_in_last("USER-001", "USDT", 60), dec!(1000));
        assert_eq!(state.volume_in_last("USER-002", "USDT", 60), dec!(5000));
    }

    #[test]
    fn test_bucket_index() {
        let t1 = Utc::now();
        let t2 = t1 + Duration::minutes(1);

        let idx1 = TransactionWindow::bucket_index(t1);
        let idx2 = TransactionWindow::bucket_index(t2);

        // Different minutes should have different indices
        assert_ne!(idx1, idx2);
        // Index should wrap around
        assert!(idx1 < BUCKET_COUNT);
        assert!(idx2 < BUCKET_COUNT);
    }

    #[test]
    fn test_has_user() {
        let mut state = ComplianceState::new();

        assert!(!state.has_user("USER-001"));

        state.record_transaction("USER-001", "USDT", dec!(100));

        assert!(state.has_user("USER-001"));
        assert!(!state.has_user("USER-002"));
    }

    #[test]
    fn test_user_count() {
        let mut state = ComplianceState::new();

        assert_eq!(state.user_count(), 0);

        state.record_transaction("USER-001", "USDT", dec!(100));
        assert_eq!(state.user_count(), 1);

        state.record_transaction("USER-002", "USDT", dec!(200));
        assert_eq!(state.user_count(), 2);

        state.record_transaction("USER-001", "USDT", dec!(300));
        assert_eq!(state.user_count(), 2); // Still 2, not 3
    }

    #[test]
    fn test_clear() {
        let mut state = ComplianceState::new();

        state.record_transaction("USER-001", "USDT", dec!(100));
        state.record_transaction("USER-002", "USDT", dec!(200));

        state.clear();

        assert_eq!(state.user_count(), 0);
        assert_eq!(state.tx_count_in_last("USER-001", 60), 0);
    }

    #[test]
    fn test_window_time_based_query() {
        let mut window = TransactionWindow::default();
        let now = Utc::now();

        // Record at current time
        window.record("USDT", dec!(100), now);

        // Should find it in last 60 minutes
        assert_eq!(window.tx_count_in_last(60, now), 1);
        assert_eq!(window.volume_in_last("USDT", 60, now), dec!(100));

        // Should find it in last 5 minutes
        assert_eq!(window.tx_count_in_last(5, now), 1);
    }
}

```

## File ./bibank\crates\core\src\amount.rs:
```rust
//! Amount - Non-negative decimal wrapper for financial amounts
//!
//! All financial amounts in BiBank MUST be non-negative.
//! This is enforced at the type level.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Errors that can occur when working with amounts
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum AmountError {
    #[error("Amount cannot be negative: {0}")]
    NegativeAmount(Decimal),
}

/// A non-negative decimal amount for financial operations.
///
/// # Invariant
/// The inner value is always >= 0. This is enforced by the constructor.
///
/// # Example
/// ```
/// use bibank_core::Amount;
/// use rust_decimal::Decimal;
///
/// let amount = Amount::new(Decimal::new(100, 0)).unwrap();
/// assert_eq!(amount.value(), Decimal::new(100, 0));
///
/// // Negative amounts are rejected
/// let negative = Amount::new(Decimal::new(-100, 0));
/// assert!(negative.is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "Decimal", into = "Decimal")]
pub struct Amount(Decimal);

impl Amount {
    /// Zero amount constant
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Create a new Amount from a Decimal.
    ///
    /// Returns an error if the value is negative.
    pub fn new(value: Decimal) -> Result<Self, AmountError> {
        if value < Decimal::ZERO {
            Err(AmountError::NegativeAmount(value))
        } else {
            Ok(Self(value))
        }
    }

    /// Create an Amount without validation.
    ///
    /// # Safety
    /// The caller MUST ensure the value is non-negative.
    /// Use only for trusted sources (e.g., deserialization from validated storage).
    #[inline]
    pub const fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    /// Get the inner Decimal value
    #[inline]
    pub const fn value(&self) -> Decimal {
        self.0
    }

    /// Check if the amount is zero
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Saturating addition - returns the sum or panics on overflow
    pub fn checked_add(&self, other: &Amount) -> Option<Amount> {
        self.0.checked_add(other.0).map(Amount)
    }

    /// Saturating subtraction - returns None if result would be negative
    pub fn checked_sub(&self, other: &Amount) -> Option<Amount> {
        let result = self.0.checked_sub(other.0)?;
        if result < Decimal::ZERO {
            None
        } else {
            Some(Amount(result))
        }
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<Decimal> for Amount {
    type Error = AmountError;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Amount> for Decimal {
    fn from(amount: Amount) -> Self {
        amount.0
    }
}

impl Default for Amount {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_positive() {
        let amount = Amount::new(Decimal::new(100, 0)).unwrap();
        assert_eq!(amount.value(), Decimal::new(100, 0));
    }

    #[test]
    fn test_amount_zero() {
        let amount = Amount::new(Decimal::ZERO).unwrap();
        assert!(amount.is_zero());
    }

    #[test]
    fn test_amount_negative_rejected() {
        let result = Amount::new(Decimal::new(-100, 0));
        assert!(matches!(result, Err(AmountError::NegativeAmount(_))));
    }

    #[test]
    fn test_checked_sub_prevents_negative() {
        let a = Amount::new(Decimal::new(50, 0)).unwrap();
        let b = Amount::new(Decimal::new(100, 0)).unwrap();
        assert!(a.checked_sub(&b).is_none());
    }

    #[test]
    fn test_checked_sub_success() {
        let a = Amount::new(Decimal::new(100, 0)).unwrap();
        let b = Amount::new(Decimal::new(30, 0)).unwrap();
        let result = a.checked_sub(&b).unwrap();
        assert_eq!(result.value(), Decimal::new(70, 0));
    }

    #[test]
    fn test_serde_roundtrip() {
        let amount = Amount::new(Decimal::new(12345, 2)).unwrap(); // 123.45
        let json = serde_json::to_string(&amount).unwrap();
        let parsed: Amount = serde_json::from_str(&json).unwrap();
        assert_eq!(amount, parsed);
    }
}

```

## File ./bibank\crates\core\src\currency.rs:
```rust
//! Currency - Type-safe currency/asset codes
//!
//! Instead of raw strings, we use an enum for common currencies
//! and a fallback for custom tokens.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur when parsing currencies
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CurrencyError {
    #[error("Empty currency code")]
    EmptyCode,

    #[error("Currency code too long (max 10 chars): {0}")]
    TooLong(String),

    #[error("Invalid currency code format: {0}")]
    InvalidFormat(String),
}

/// Currency/Asset codes
///
/// Common currencies are pre-defined for type safety and performance.
/// Custom tokens use the `Other` variant.
///
/// # Examples
/// ```
/// use bibank_core::Currency;
///
/// let usdt: Currency = "USDT".parse().unwrap();
/// assert_eq!(usdt, Currency::Usdt);
///
/// let btc = Currency::Btc;
/// assert_eq!(btc.to_string(), "BTC");
///
/// // Custom token
/// let custom: Currency = "MYTOKEN".parse().unwrap();
/// assert!(matches!(custom, Currency::Other(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Currency {
    // === Stablecoins ===
    /// Tether USD
    Usdt,
    /// USD Coin
    Usdc,
    /// Binance USD
    Busd,
    /// Dai
    Dai,

    // === Major Crypto ===
    /// Bitcoin
    Btc,
    /// Ethereum
    Eth,
    /// Binance Coin
    Bnb,
    /// Solana
    Sol,
    /// XRP
    Xrp,
    /// Cardano
    Ada,
    /// Dogecoin
    Doge,
    /// Polygon
    Matic,
    /// Litecoin
    Ltc,

    // === Fiat ===
    /// US Dollar
    Usd,
    /// Euro
    Eur,
    /// British Pound
    Gbp,
    /// Japanese Yen
    Jpy,
    /// Vietnamese Dong
    Vnd,

    // === Custom tokens ===
    /// Any other token/currency
    Other(String),
}

impl Currency {
    /// Returns the currency code as a string slice
    pub fn code(&self) -> &str {
        match self {
            // Stablecoins
            Currency::Usdt => "USDT",
            Currency::Usdc => "USDC",
            Currency::Busd => "BUSD",
            Currency::Dai => "DAI",

            // Crypto
            Currency::Btc => "BTC",
            Currency::Eth => "ETH",
            Currency::Bnb => "BNB",
            Currency::Sol => "SOL",
            Currency::Xrp => "XRP",
            Currency::Ada => "ADA",
            Currency::Doge => "DOGE",
            Currency::Matic => "MATIC",
            Currency::Ltc => "LTC",

            // Fiat
            Currency::Usd => "USD",
            Currency::Eur => "EUR",
            Currency::Gbp => "GBP",
            Currency::Jpy => "JPY",
            Currency::Vnd => "VND",

            // Other
            Currency::Other(s) => s.as_str(),
        }
    }

    /// Returns true if this is a stablecoin
    pub fn is_stablecoin(&self) -> bool {
        matches!(
            self,
            Currency::Usdt | Currency::Usdc | Currency::Busd | Currency::Dai
        )
    }

    /// Returns true if this is fiat currency
    pub fn is_fiat(&self) -> bool {
        matches!(
            self,
            Currency::Usd | Currency::Eur | Currency::Gbp | Currency::Jpy | Currency::Vnd
        )
    }

    /// Returns true if this is a major cryptocurrency
    pub fn is_crypto(&self) -> bool {
        matches!(
            self,
            Currency::Btc
                | Currency::Eth
                | Currency::Bnb
                | Currency::Sol
                | Currency::Xrp
                | Currency::Ada
                | Currency::Doge
                | Currency::Matic
                | Currency::Ltc
        )
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl FromStr for Currency {
    type Err = CurrencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_uppercase();

        if s.is_empty() {
            return Err(CurrencyError::EmptyCode);
        }

        if s.len() > 10 {
            return Err(CurrencyError::TooLong(s));
        }

        // Validate: only alphanumeric
        if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(CurrencyError::InvalidFormat(s));
        }

        Ok(match s.as_str() {
            // Stablecoins
            "USDT" => Currency::Usdt,
            "USDC" => Currency::Usdc,
            "BUSD" => Currency::Busd,
            "DAI" => Currency::Dai,

            // Crypto
            "BTC" => Currency::Btc,
            "ETH" => Currency::Eth,
            "BNB" => Currency::Bnb,
            "SOL" => Currency::Sol,
            "XRP" => Currency::Xrp,
            "ADA" => Currency::Ada,
            "DOGE" => Currency::Doge,
            "MATIC" => Currency::Matic,
            "LTC" => Currency::Ltc,

            // Fiat
            "USD" => Currency::Usd,
            "EUR" => Currency::Eur,
            "GBP" => Currency::Gbp,
            "JPY" => Currency::Jpy,
            "VND" => Currency::Vnd,

            // Other
            _ => Currency::Other(s),
        })
    }
}

impl TryFrom<String> for Currency {
    type Error = CurrencyError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Currency> for String {
    fn from(c: Currency) -> Self {
        c.code().to_string()
    }
}

impl From<&str> for Currency {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or_else(|_| Currency::Other(s.to_uppercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_known_currencies() {
        assert_eq!("USDT".parse::<Currency>().unwrap(), Currency::Usdt);
        assert_eq!("btc".parse::<Currency>().unwrap(), Currency::Btc);
        assert_eq!("ETH".parse::<Currency>().unwrap(), Currency::Eth);
        assert_eq!("usd".parse::<Currency>().unwrap(), Currency::Usd);
    }

    #[test]
    fn test_parse_custom_token() {
        let custom: Currency = "MYTOKEN".parse().unwrap();
        assert_eq!(custom, Currency::Other("MYTOKEN".to_string()));
        assert_eq!(custom.to_string(), "MYTOKEN");
    }

    #[test]
    fn test_display() {
        assert_eq!(Currency::Usdt.to_string(), "USDT");
        assert_eq!(Currency::Btc.to_string(), "BTC");
        assert_eq!(Currency::Other("XYZ".to_string()).to_string(), "XYZ");
    }

    #[test]
    fn test_is_stablecoin() {
        assert!(Currency::Usdt.is_stablecoin());
        assert!(Currency::Usdc.is_stablecoin());
        assert!(!Currency::Btc.is_stablecoin());
        assert!(!Currency::Usd.is_stablecoin());
    }

    #[test]
    fn test_is_fiat() {
        assert!(Currency::Usd.is_fiat());
        assert!(Currency::Vnd.is_fiat());
        assert!(!Currency::Usdt.is_fiat());
        assert!(!Currency::Btc.is_fiat());
    }

    #[test]
    fn test_is_crypto() {
        assert!(Currency::Btc.is_crypto());
        assert!(Currency::Eth.is_crypto());
        assert!(!Currency::Usdt.is_crypto());
        assert!(!Currency::Usd.is_crypto());
    }

    #[test]
    fn test_empty_code_error() {
        let result: Result<Currency, _> = "".parse();
        assert!(matches!(result, Err(CurrencyError::EmptyCode)));
    }

    #[test]
    fn test_too_long_error() {
        let result: Result<Currency, _> = "VERYLONGCURRENCYNAME".parse();
        assert!(matches!(result, Err(CurrencyError::TooLong(_))));
    }

    #[test]
    fn test_invalid_format_error() {
        let result: Result<Currency, _> = "BTC-USD".parse();
        assert!(matches!(result, Err(CurrencyError::InvalidFormat(_))));
    }

    #[test]
    fn test_serde_roundtrip() {
        let currencies = vec![
            Currency::Usdt,
            Currency::Btc,
            Currency::Usd,
            Currency::Other("XYZ".to_string()),
        ];

        for currency in currencies {
            let json = serde_json::to_string(&currency).unwrap();
            let parsed: Currency = serde_json::from_str(&json).unwrap();
            assert_eq!(currency, parsed);
        }
    }

    #[test]
    fn test_from_str_trait() {
        let currency: Currency = "ETH".into();
        assert_eq!(currency, Currency::Eth);
    }
}

```

## File ./bibank\crates\core\src\lib.rs:
```rust
//! BiBank Core - Domain types
//!
//! This crate contains the fundamental types used across BiBank:
//! - `Amount`: Non-negative decimal wrapper for financial amounts
//! - `Currency`: Type-safe currency/asset codes

pub mod amount;
pub mod currency;

pub use amount::Amount;
pub use currency::Currency;

```

## File ./bibank\crates\dsl\src\evaluator.rs:
```rust
//! Rule evaluator - evaluates rules against transaction context

use rust_decimal::Decimal;

use bibank_compliance::{AmlDecision, CheckResult};
use bibank_hooks::HookContext;

use crate::types::{Condition, RuleAction, RuleDefinition, RuleSet, RuleType};

/// Result of evaluating a single rule
#[derive(Debug, Clone)]
pub struct RuleEvalResult {
    /// Rule ID
    pub rule_id: String,
    /// Whether the rule triggered
    pub triggered: bool,
    /// The action (if triggered)
    pub action: Option<RuleAction>,
}

/// Result of evaluating a rule set
#[derive(Debug, Clone)]
pub struct RuleSetEvalResult {
    /// Individual rule results
    pub results: Vec<RuleEvalResult>,
    /// Rules that triggered
    pub triggered_rules: Vec<String>,
    /// Final aggregated decision
    pub decision: AmlDecision,
}

/// Rule evaluator
pub struct RuleEvaluator;

impl RuleEvaluator {
    /// Evaluate a single condition against a context
    pub fn eval_condition(condition: &Condition, ctx: &HookContext) -> bool {
        match condition {
            Condition::AmountGte { threshold } => ctx.amount >= *threshold,
            Condition::AmountLt { threshold } => ctx.amount < *threshold,
            Condition::AmountInRange { min, max } => ctx.amount >= *min && ctx.amount < *max,
            Condition::AccountAgeLt { days } => {
                ctx.metadata.account_age_days.unwrap_or(365) < *days
            }
            Condition::AccountAgeGte { days } => {
                ctx.metadata.account_age_days.unwrap_or(365) >= *days
            }
            Condition::IsWatchlisted => ctx.metadata.is_watchlisted,
            Condition::IsPep => ctx.metadata.is_pep,
            Condition::TxCountGte { count, .. } => {
                // Would need velocity state - simplified for now
                // In real implementation, would query ComplianceState
                false
            }
            Condition::VolumeGte { threshold, .. } => {
                // Would need velocity state - simplified for now
                false
            }
            Condition::Custom { .. } => {
                // Custom conditions need external handler
                false
            }
            Condition::All { conditions } => {
                conditions.iter().all(|c| Self::eval_condition(c, ctx))
            }
            Condition::Any { conditions } => {
                conditions.iter().any(|c| Self::eval_condition(c, ctx))
            }
        }
    }

    /// Evaluate a single rule
    pub fn eval_rule(rule: &RuleDefinition, ctx: &HookContext) -> RuleEvalResult {
        if !rule.enabled {
            return RuleEvalResult {
                rule_id: rule.id.clone(),
                triggered: false,
                action: None,
            };
        }

        let triggered = Self::eval_condition(&rule.condition, ctx);

        RuleEvalResult {
            rule_id: rule.id.clone(),
            triggered,
            action: if triggered { Some(rule.action.clone()) } else { None },
        }
    }

    /// Evaluate all rules in a rule set
    pub fn eval_ruleset(ruleset: &RuleSet, ctx: &HookContext) -> RuleSetEvalResult {
        let mut results = Vec::new();
        let mut triggered_rules = Vec::new();
        let mut decisions = Vec::new();

        // Evaluate all rules
        for rule in &ruleset.rules {
            let result = Self::eval_rule(rule, ctx);
            if result.triggered {
                triggered_rules.push(rule.id.clone());
                if let Some(action) = &result.action {
                    decisions.push(action.to_decision());
                }
            }
            results.push(result);
        }

        // Aggregate decisions using max()
        let final_decision = decisions
            .into_iter()
            .max()
            .unwrap_or(AmlDecision::Approved);

        RuleSetEvalResult {
            results,
            triggered_rules,
            decision: final_decision,
        }
    }

    /// Evaluate only BLOCK rules (for pre-validation)
    pub fn eval_block_rules(ruleset: &RuleSet, ctx: &HookContext) -> Option<RuleEvalResult> {
        for rule in ruleset.block_rules() {
            let result = Self::eval_rule(rule, ctx);
            if result.triggered {
                return Some(result);
            }
        }
        None
    }

    /// Evaluate only FLAG rules (for post-commit)
    pub fn eval_flag_rules(ruleset: &RuleSet, ctx: &HookContext) -> RuleSetEvalResult {
        let mut results = Vec::new();
        let mut triggered_rules = Vec::new();
        let mut decisions = Vec::new();

        for rule in ruleset.flag_rules() {
            let result = Self::eval_rule(rule, ctx);
            if result.triggered {
                triggered_rules.push(rule.id.clone());
                if let Some(action) = &result.action {
                    decisions.push(action.to_decision());
                }
            }
            results.push(result);
        }

        let final_decision = decisions
            .into_iter()
            .max()
            .unwrap_or(AmlDecision::Approved);

        RuleSetEvalResult {
            results,
            triggered_rules,
            decision: final_decision,
        }
    }

    /// Convert rule set evaluation to CheckResult
    pub fn to_check_result(eval_result: &RuleSetEvalResult) -> CheckResult {
        CheckResult {
            decision: eval_result.decision.clone(),
            rules_triggered: eval_result.triggered_rules.clone(),
            risk_score: None, // Would extract from flagged decision
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RuleAction;
    use crate::{account_age_lt, all_of, amount_gte, is_watchlisted, rule, rule_set};
    use bibank_compliance::{ApprovalLevel, RiskScore};
    use rust_decimal_macros::dec;

    fn create_test_context(amount: Decimal, watchlisted: bool, age_days: i64) -> HookContext {
        let mut ctx = HookContext::new("corr-1", "user-1", "DEPOSIT", amount, "USDT")
            .with_account_age(age_days);
        if watchlisted {
            ctx.metadata.is_watchlisted = true;
        }
        ctx
    }

    #[test]
    fn test_eval_amount_gte() {
        let ctx = create_test_context(dec!(15000), false, 30);
        assert!(RuleEvaluator::eval_condition(
            &Condition::amount_gte(dec!(10000)),
            &ctx
        ));
        assert!(!RuleEvaluator::eval_condition(
            &Condition::amount_gte(dec!(20000)),
            &ctx
        ));
    }

    #[test]
    fn test_eval_amount_lt() {
        let ctx = create_test_context(dec!(5000), false, 30);
        assert!(RuleEvaluator::eval_condition(
            &Condition::amount_lt(dec!(10000)),
            &ctx
        ));
        assert!(!RuleEvaluator::eval_condition(
            &Condition::amount_lt(dec!(5000)),
            &ctx
        ));
    }

    #[test]
    fn test_eval_watchlisted() {
        let ctx_clean = create_test_context(dec!(1000), false, 30);
        let ctx_bad = create_test_context(dec!(1000), true, 30);

        assert!(!RuleEvaluator::eval_condition(
            &Condition::is_watchlisted(),
            &ctx_clean
        ));
        assert!(RuleEvaluator::eval_condition(
            &Condition::is_watchlisted(),
            &ctx_bad
        ));
    }

    #[test]
    fn test_eval_account_age() {
        let ctx_new = create_test_context(dec!(1000), false, 3);
        let ctx_old = create_test_context(dec!(1000), false, 30);

        assert!(RuleEvaluator::eval_condition(
            &Condition::account_age_lt(7),
            &ctx_new
        ));
        assert!(!RuleEvaluator::eval_condition(
            &Condition::account_age_lt(7),
            &ctx_old
        ));
    }

    #[test]
    fn test_eval_all_condition() {
        let cond = Condition::all(vec![
            Condition::amount_gte(dec!(5000)),
            Condition::account_age_lt(7),
        ]);

        let ctx_match = create_test_context(dec!(10000), false, 3);
        let ctx_no_match = create_test_context(dec!(10000), false, 30);

        assert!(RuleEvaluator::eval_condition(&cond, &ctx_match));
        assert!(!RuleEvaluator::eval_condition(&cond, &ctx_no_match));
    }

    #[test]
    fn test_eval_any_condition() {
        let cond = Condition::any(vec![
            Condition::is_watchlisted(),
            Condition::amount_gte(dec!(50000)),
        ]);

        let ctx_watchlisted = create_test_context(dec!(100), true, 30);
        let ctx_large = create_test_context(dec!(60000), false, 30);
        let ctx_neither = create_test_context(dec!(100), false, 30);

        assert!(RuleEvaluator::eval_condition(&cond, &ctx_watchlisted));
        assert!(RuleEvaluator::eval_condition(&cond, &ctx_large));
        assert!(!RuleEvaluator::eval_condition(&cond, &ctx_neither));
    }

    #[test]
    fn test_eval_rule() {
        let rule = rule! {
            id: "LARGE_TX",
            type: flag,
            when: amount_gte!(dec!(10000)),
            then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large"),
        };

        let ctx_small = create_test_context(dec!(5000), false, 30);
        let ctx_large = create_test_context(dec!(15000), false, 30);

        let result_small = RuleEvaluator::eval_rule(&rule, &ctx_small);
        assert!(!result_small.triggered);

        let result_large = RuleEvaluator::eval_rule(&rule, &ctx_large);
        assert!(result_large.triggered);
        assert!(result_large.action.is_some());
    }

    #[test]
    fn test_eval_ruleset() {
        let ruleset = rule_set! {
            name: "TEST",
            rules: [
                rule! {
                    id: "SANCTIONS",
                    type: block,
                    when: is_watchlisted!(),
                    then: RuleAction::block("SANCTIONS", "Watchlisted"),
                    priority: 10,
                },
                rule! {
                    id: "LARGE_TX",
                    type: flag,
                    when: amount_gte!(dec!(10000)),
                    then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large"),
                    priority: 50,
                },
            ],
        };

        // Clean small transaction
        let ctx1 = create_test_context(dec!(1000), false, 30);
        let result1 = RuleEvaluator::eval_ruleset(&ruleset, &ctx1);
        assert!(result1.triggered_rules.is_empty());
        assert!(result1.decision.is_approved());

        // Large transaction
        let ctx2 = create_test_context(dec!(15000), false, 30);
        let result2 = RuleEvaluator::eval_ruleset(&ruleset, &ctx2);
        assert!(result2.triggered_rules.contains(&"LARGE_TX".to_string()));
        assert!(result2.decision.is_flagged());

        // Watchlisted user
        let ctx3 = create_test_context(dec!(1000), true, 30);
        let result3 = RuleEvaluator::eval_ruleset(&ruleset, &ctx3);
        assert!(result3.triggered_rules.contains(&"SANCTIONS".to_string()));
        assert!(result3.decision.is_blocked());
    }

    #[test]
    fn test_eval_block_rules_only() {
        let ruleset = rule_set! {
            name: "TEST",
            rules: [
                rule! {
                    id: "SANCTIONS",
                    type: block,
                    when: is_watchlisted!(),
                    then: RuleAction::block("SANCTIONS", "Watchlisted"),
                },
                rule! {
                    id: "LARGE_TX",
                    type: flag,
                    when: amount_gte!(dec!(10000)),
                    then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large"),
                },
            ],
        };

        let ctx_clean = create_test_context(dec!(15000), false, 30);
        let result = RuleEvaluator::eval_block_rules(&ruleset, &ctx_clean);
        assert!(result.is_none()); // No BLOCK rules triggered

        let ctx_bad = create_test_context(dec!(100), true, 30);
        let result2 = RuleEvaluator::eval_block_rules(&ruleset, &ctx_bad);
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().rule_id, "SANCTIONS");
    }

    #[test]
    fn test_decision_aggregation() {
        let ruleset = rule_set! {
            name: "TEST",
            rules: [
                rule! {
                    id: "RULE_1",
                    type: flag,
                    when: amount_gte!(dec!(5000)),
                    then: RuleAction::flag(RiskScore::Low, ApprovalLevel::L1, "Flag 1"),
                },
                rule! {
                    id: "RULE_2",
                    type: flag,
                    when: amount_gte!(dec!(10000)),
                    then: RuleAction::flag(RiskScore::High, ApprovalLevel::L3, "Flag 2"),
                },
            ],
        };

        // Both rules trigger, should get max (Higher risk/approval)
        let ctx = create_test_context(dec!(15000), false, 30);
        let result = RuleEvaluator::eval_ruleset(&ruleset, &ctx);

        assert_eq!(result.triggered_rules.len(), 2);
        assert!(result.decision.is_flagged());
        // Max should be L3 with High risk
        if let AmlDecision::Flagged {
            required_approval,
            risk_score,
            ..
        } = &result.decision
        {
            assert_eq!(*required_approval, ApprovalLevel::L3);
            assert_eq!(*risk_score, RiskScore::High);
        }
    }
}

```

## File ./bibank\crates\dsl\src\lib.rs:
```rust
//! BiBank DSL - Domain Specific Language for Compliance Rules
//!
//! Phase 4: Declarative rule definition using macros
//!
//! # Overview
//!
//! This crate provides a DSL for defining AML/Compliance rules declaratively:
//!
//! ```ignore
//! use bibank_dsl::*;
//!
//! // Define individual rules
//! let sanctions_rule = rule! {
//!     id: "SANCTIONS_CHECK",
//!     name: "Sanctions Watchlist Check",
//!     type: block,
//!     when: is_watchlisted!(),
//!     then: RuleAction::block("SANCTIONS", "User on sanctions watchlist"),
//!     priority: 10,
//! };
//!
//! let large_tx_rule = rule! {
//!     id: "LARGE_TX_ALERT",
//!     type: flag,
//!     when: amount_gte!(dec!(10000)),
//!     then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large transaction"),
//! };
//!
//! // Group rules into a rule set
//! let ruleset = rule_set! {
//!     name: "AML_BASIC",
//!     description: "Basic AML compliance rules",
//!     rules: [sanctions_rule, large_tx_rule],
//! };
//!
//! // Evaluate against transaction context
//! let result = RuleEvaluator::eval_ruleset(&ruleset, &ctx);
//! ```
//!
//! # Rule Types
//!
//! - **BLOCK rules**: Pre-validation hooks that reject transactions immediately
//! - **FLAG rules**: Post-commit hooks that flag transactions for review
//!
//! # Conditions
//!
//! Built-in conditions:
//! - `amount_gte!(value)` - Amount >= threshold
//! - `amount_lt!(value)` - Amount < threshold
//! - `account_age_lt!(days)` - Account younger than N days
//! - `is_watchlisted!()` - User on sanctions watchlist
//! - `is_pep!()` - User is Politically Exposed Person
//! - `all_of!(cond1, cond2)` - All conditions must match (AND)
//! - `any_of!(cond1, cond2)` - Any condition must match (OR)
//!
//! # Actions
//!
//! - `RuleAction::block(code, reason)` - Block transaction
//! - `RuleAction::flag(risk, level, reason)` - Flag for review
//! - `RuleAction::Approve` - Approve (no action)

pub mod evaluator;
pub mod macros;
pub mod types;

// Re-export commonly used types
pub use bibank_compliance::{AmlDecision, ApprovalLevel, RiskScore};
pub use evaluator::{RuleEvalResult, RuleEvaluator, RuleSetEvalResult};
pub use types::{Condition, RuleAction, RuleBuilder, RuleDefinition, RuleSet, RuleType};

```

## File ./bibank\crates\dsl\src\macros.rs:
```rust
//! Rule and RuleSet macros for declarative compliance rule definition
//!
//! # Example
//!
//! ```ignore
//! use bibank_dsl::{rule, rule_set};
//!
//! let sanctions_rule = rule! {
//!     id: "SANCTIONS_CHECK",
//!     name: "Sanctions Watchlist Check",
//!     type: block,
//!     when: is_watchlisted,
//!     then: block("SANCTIONS", "User on sanctions watchlist"),
//!     priority: 10,
//! };
//!
//! let large_tx_rule = rule! {
//!     id: "LARGE_TX_ALERT",
//!     type: flag,
//!     when: amount >= 10_000,
//!     then: flag(Medium, L1, "Large transaction detected"),
//! };
//!
//! let ruleset = rule_set! {
//!     name: "AML_BASIC",
//!     description: "Basic AML compliance rules",
//!     rules: [sanctions_rule, large_tx_rule],
//! };
//! ```

/// Create a compliance rule definition
///
/// # Syntax
///
/// ```ignore
/// rule! {
///     id: "RULE_ID",                          // Required: unique identifier
///     name: "Human readable name",            // Optional: defaults to id
///     description: "What this rule does",     // Optional
///     type: block | flag,                     // Required: rule type
///     when: <condition>,                      // Required: trigger condition
///     then: <action>,                         // Required: action to take
///     priority: 100,                          // Optional: lower = runs first
///     enabled: true,                          // Optional: defaults to true
/// }
/// ```
///
/// # Conditions
///
/// - `amount >= <value>` - Amount greater than or equal
/// - `amount < <value>` - Amount less than
/// - `amount in <min>..<max>` - Amount in range
/// - `account_age < <days>` - Account younger than days
/// - `account_age >= <days>` - Account older than days
/// - `is_watchlisted` - User on watchlist
/// - `is_pep` - User is PEP
/// - `tx_count >= <n> in <minutes>m` - Transaction count in window
/// - `volume >= <amount> in <minutes>m` - Volume in window
/// - `all(<cond1>, <cond2>, ...)` - All conditions must match
/// - `any(<cond1>, <cond2>, ...)` - Any condition must match
///
/// # Actions
///
/// - `block("<code>", "<reason>")` - Block the transaction
/// - `flag(<RiskScore>, <ApprovalLevel>, "<reason>")` - Flag for review
/// - `approve` - Approve (no action)
#[macro_export]
macro_rules! rule {
    // Full syntax with all fields
    (
        id: $id:expr,
        $(name: $name:expr,)?
        $(description: $desc:expr,)?
        type: block,
        when: $cond:expr,
        then: $action:expr
        $(, priority: $priority:expr)?
        $(, enabled: $enabled:expr)?
        $(,)?
    ) => {{
        $crate::types::RuleDefinition::builder($id)
            $(.name($name))?
            $(.description($desc))?
            .block_rule()
            .when($cond)
            .then($action)
            $(.priority($priority))?
            $(.enabled($enabled))?
            .build()
    }};

    // Flag rule
    (
        id: $id:expr,
        $(name: $name:expr,)?
        $(description: $desc:expr,)?
        type: flag,
        when: $cond:expr,
        then: $action:expr
        $(, priority: $priority:expr)?
        $(, enabled: $enabled:expr)?
        $(,)?
    ) => {{
        $crate::types::RuleDefinition::builder($id)
            $(.name($name))?
            $(.description($desc))?
            .flag_rule()
            .when($cond)
            .then($action)
            $(.priority($priority))?
            $(.enabled($enabled))?
            .build()
    }};
}

/// Create a rule set containing multiple rules
///
/// # Syntax
///
/// ```ignore
/// rule_set! {
///     name: "RULESET_NAME",
///     description: "What this ruleset does",  // Optional
///     rules: [rule1, rule2, ...],
/// }
/// ```
#[macro_export]
macro_rules! rule_set {
    (
        name: $name:expr,
        $(description: $desc:expr,)?
        rules: [$($rule:expr),* $(,)?]
        $(,)?
    ) => {{
        let mut ruleset = $crate::types::RuleSet::new($name);
        $(ruleset = ruleset.with_description($desc);)?
        $(ruleset = ruleset.add_rule($rule);)*
        ruleset
    }};
}

/// Shorthand for creating a block action
#[macro_export]
macro_rules! block_action {
    ($code:expr, $reason:expr) => {
        $crate::types::RuleAction::block($code, $reason)
    };
}

/// Shorthand for creating a flag action
#[macro_export]
macro_rules! flag_action {
    ($risk:ident, $level:ident, $reason:expr) => {
        $crate::types::RuleAction::flag(
            $crate::RiskScore::$risk,
            $crate::ApprovalLevel::$level,
            $reason,
        )
    };
}

/// Shorthand for amount >= condition
#[macro_export]
macro_rules! amount_gte {
    ($threshold:expr) => {
        $crate::types::Condition::amount_gte($threshold)
    };
}

/// Shorthand for amount < condition
#[macro_export]
macro_rules! amount_lt {
    ($threshold:expr) => {
        $crate::types::Condition::amount_lt($threshold)
    };
}

/// Shorthand for account_age < days condition
#[macro_export]
macro_rules! account_age_lt {
    ($days:expr) => {
        $crate::types::Condition::account_age_lt($days)
    };
}

/// Shorthand for is_watchlisted condition
#[macro_export]
macro_rules! is_watchlisted {
    () => {
        $crate::types::Condition::is_watchlisted()
    };
}

/// Shorthand for is_pep condition
#[macro_export]
macro_rules! is_pep {
    () => {
        $crate::types::Condition::is_pep()
    };
}

/// Shorthand for all conditions (AND)
#[macro_export]
macro_rules! all_of {
    ($($cond:expr),+ $(,)?) => {
        $crate::types::Condition::all(vec![$($cond),+])
    };
}

/// Shorthand for any conditions (OR)
#[macro_export]
macro_rules! any_of {
    ($($cond:expr),+ $(,)?) => {
        $crate::types::Condition::any(vec![$($cond),+])
    };
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Condition, RuleAction, RuleType};
    use bibank_compliance::{ApprovalLevel, RiskScore};
    use rust_decimal_macros::dec;

    #[test]
    fn test_rule_macro_block() {
        let rule = rule! {
            id: "SANCTIONS",
            name: "Sanctions Check",
            type: block,
            when: Condition::is_watchlisted(),
            then: RuleAction::block("SANCTIONS", "User on watchlist"),
            priority: 10,
        };

        assert_eq!(rule.id, "SANCTIONS");
        assert_eq!(rule.name, "Sanctions Check");
        assert_eq!(rule.rule_type, RuleType::Block);
        assert_eq!(rule.priority, 10);
    }

    #[test]
    fn test_rule_macro_flag() {
        let rule = rule! {
            id: "LARGE_TX",
            type: flag,
            when: Condition::amount_gte(dec!(10000)),
            then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large tx"),
        };

        assert_eq!(rule.id, "LARGE_TX");
        assert_eq!(rule.rule_type, RuleType::Flag);
        assert!(rule.enabled);
    }

    #[test]
    fn test_rule_set_macro() {
        let sanctions = rule! {
            id: "SANCTIONS",
            type: block,
            when: Condition::is_watchlisted(),
            then: RuleAction::block("SANCTIONS", "Watchlisted"),
        };

        let large_tx = rule! {
            id: "LARGE_TX",
            type: flag,
            when: Condition::amount_gte(dec!(10000)),
            then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large"),
        };

        let ruleset = rule_set! {
            name: "AML_BASIC",
            description: "Basic AML rules",
            rules: [sanctions, large_tx],
        };

        assert_eq!(ruleset.name, "AML_BASIC");
        assert_eq!(ruleset.rules.len(), 2);
    }

    #[test]
    fn test_shorthand_macros() {
        let cond = amount_gte!(dec!(5000));
        assert!(matches!(cond, Condition::AmountGte { .. }));

        let cond2 = account_age_lt!(7);
        assert!(matches!(cond2, Condition::AccountAgeLt { days: 7 }));

        let cond3 = is_watchlisted!();
        assert!(matches!(cond3, Condition::IsWatchlisted));
    }

    #[test]
    fn test_all_of_macro() {
        let cond = all_of!(
            amount_gte!(dec!(5000)),
            account_age_lt!(7),
        );

        if let Condition::All { conditions } = cond {
            assert_eq!(conditions.len(), 2);
        } else {
            panic!("Expected All condition");
        }
    }

    #[test]
    fn test_any_of_macro() {
        let cond = any_of!(
            is_watchlisted!(),
            is_pep!(),
        );

        if let Condition::Any { conditions } = cond {
            assert_eq!(conditions.len(), 2);
        } else {
            panic!("Expected Any condition");
        }
    }

    #[test]
    fn test_complex_rule_with_macros() {
        let rule = rule! {
            id: "NEW_ACCOUNT_LARGE_TX",
            name: "New Account Large Transaction",
            description: "Flag large transactions from new accounts",
            type: flag,
            when: all_of!(
                amount_gte!(dec!(5000)),
                account_age_lt!(7)
            ),
            then: RuleAction::flag(
                RiskScore::High,
                ApprovalLevel::L2,
                "New account with large transaction"
            ),
            priority: 30,
        };

        assert_eq!(rule.id, "NEW_ACCOUNT_LARGE_TX");
        assert_eq!(rule.priority, 30);

        if let Condition::All { conditions } = &rule.condition {
            assert_eq!(conditions.len(), 2);
        }
    }
}

```

## File ./bibank\crates\dsl\src\types.rs:
```rust
//! Rule types for the DSL
//!
//! Defines the core types used by the rule! and rule_set! macros.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use bibank_compliance::{AmlDecision, ApprovalLevel, RiskScore};

/// Action to take when a rule triggers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleAction {
    /// Block the transaction (pre-validation)
    Block {
        /// Compliance code
        code: String,
        /// Human-readable reason
        reason: String,
    },
    /// Flag the transaction for review (post-commit)
    Flag {
        /// Risk score
        risk_score: RiskScore,
        /// Required approval level
        approval_level: ApprovalLevel,
        /// Reason for flagging
        reason: String,
    },
    /// Approve (no action needed)
    Approve,
}

impl RuleAction {
    /// Create a block action
    pub fn block(code: impl Into<String>, reason: impl Into<String>) -> Self {
        RuleAction::Block {
            code: code.into(),
            reason: reason.into(),
        }
    }

    /// Create a flag action
    pub fn flag(
        risk_score: RiskScore,
        approval_level: ApprovalLevel,
        reason: impl Into<String>,
    ) -> Self {
        RuleAction::Flag {
            risk_score,
            approval_level,
            reason: reason.into(),
        }
    }

    /// Check if this is a block action
    pub fn is_block(&self) -> bool {
        matches!(self, RuleAction::Block { .. })
    }

    /// Check if this is a flag action
    pub fn is_flag(&self) -> bool {
        matches!(self, RuleAction::Flag { .. })
    }

    /// Convert to AmlDecision
    pub fn to_decision(&self) -> AmlDecision {
        match self {
            RuleAction::Block { code, reason } => AmlDecision::blocked(reason, code),
            RuleAction::Flag {
                risk_score,
                approval_level,
                reason,
            } => AmlDecision::flagged(reason, *risk_score, *approval_level),
            RuleAction::Approve => AmlDecision::Approved,
        }
    }
}

/// Condition operators for rule matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Condition {
    /// Amount >= threshold
    AmountGte { threshold: Decimal },
    /// Amount < threshold
    AmountLt { threshold: Decimal },
    /// Amount in range [min, max)
    AmountInRange { min: Decimal, max: Decimal },
    /// Account age < days
    AccountAgeLt { days: i64 },
    /// Account age >= days
    AccountAgeGte { days: i64 },
    /// User is on watchlist
    IsWatchlisted,
    /// User is a PEP
    IsPep,
    /// Transaction count in window >= count
    TxCountGte { count: u32, window_minutes: u32 },
    /// Total volume in window >= threshold
    VolumeGte { threshold: Decimal, window_minutes: u32 },
    /// Custom condition with name
    Custom { name: String },
    /// All conditions must match (AND)
    All { conditions: Vec<Condition> },
    /// Any condition must match (OR)
    Any { conditions: Vec<Condition> },
}

impl Condition {
    /// Amount >= threshold
    pub fn amount_gte(threshold: Decimal) -> Self {
        Condition::AmountGte { threshold }
    }

    /// Amount < threshold
    pub fn amount_lt(threshold: Decimal) -> Self {
        Condition::AmountLt { threshold }
    }

    /// Amount in range [min, max)
    pub fn amount_in_range(min: Decimal, max: Decimal) -> Self {
        Condition::AmountInRange { min, max }
    }

    /// Account age < days
    pub fn account_age_lt(days: i64) -> Self {
        Condition::AccountAgeLt { days }
    }

    /// User is on watchlist
    pub fn is_watchlisted() -> Self {
        Condition::IsWatchlisted
    }

    /// User is a PEP
    pub fn is_pep() -> Self {
        Condition::IsPep
    }

    /// Transaction count >= count in window
    pub fn tx_count_gte(count: u32, window_minutes: u32) -> Self {
        Condition::TxCountGte {
            count,
            window_minutes,
        }
    }

    /// Volume >= threshold in window
    pub fn volume_gte(threshold: Decimal, window_minutes: u32) -> Self {
        Condition::VolumeGte {
            threshold,
            window_minutes,
        }
    }

    /// All conditions (AND)
    pub fn all(conditions: Vec<Condition>) -> Self {
        Condition::All { conditions }
    }

    /// Any condition (OR)
    pub fn any(conditions: Vec<Condition>) -> Self {
        Condition::Any { conditions }
    }
}

/// A single compliance rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDefinition {
    /// Unique rule ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Whether this is a BLOCK (pre-validation) or FLAG (post-commit) rule
    pub rule_type: RuleType,
    /// Condition that triggers this rule
    pub condition: Condition,
    /// Action to take when triggered
    pub action: RuleAction,
    /// Priority (lower = runs first)
    pub priority: u32,
    /// Whether this rule is enabled
    pub enabled: bool,
}

/// Type of rule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    /// Pre-validation rule (can BLOCK)
    Block,
    /// Post-commit rule (can FLAG)
    Flag,
}

impl RuleDefinition {
    /// Create a new rule definition builder
    pub fn builder(id: impl Into<String>) -> RuleBuilder {
        RuleBuilder::new(id)
    }
}

/// Builder for RuleDefinition
pub struct RuleBuilder {
    id: String,
    name: Option<String>,
    description: String,
    rule_type: RuleType,
    condition: Option<Condition>,
    action: Option<RuleAction>,
    priority: u32,
    enabled: bool,
}

impl RuleBuilder {
    /// Create a new builder
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            description: String::new(),
            rule_type: RuleType::Flag,
            condition: None,
            action: None,
            priority: 100,
            enabled: true,
        }
    }

    /// Set the rule name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set as a BLOCK rule
    pub fn block_rule(mut self) -> Self {
        self.rule_type = RuleType::Block;
        self
    }

    /// Set as a FLAG rule
    pub fn flag_rule(mut self) -> Self {
        self.rule_type = RuleType::Flag;
        self
    }

    /// Set the condition
    pub fn when(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Set the action
    pub fn then(mut self, action: RuleAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Set priority
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Build the rule definition
    pub fn build(self) -> RuleDefinition {
        RuleDefinition {
            id: self.id.clone(),
            name: self.name.unwrap_or_else(|| self.id.clone()),
            description: self.description,
            rule_type: self.rule_type,
            condition: self.condition.expect("condition is required"),
            action: self.action.expect("action is required"),
            priority: self.priority,
            enabled: self.enabled,
        }
    }
}

/// A set of rules
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    /// Name of this rule set
    pub name: String,
    /// Description
    pub description: String,
    /// Rules in this set
    pub rules: Vec<RuleDefinition>,
}

impl RuleSet {
    /// Create a new empty rule set
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            rules: Vec::new(),
        }
    }

    /// Add a description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add a rule
    pub fn add_rule(mut self, rule: RuleDefinition) -> Self {
        self.rules.push(rule);
        self
    }

    /// Add multiple rules
    pub fn add_rules(mut self, rules: Vec<RuleDefinition>) -> Self {
        self.rules.extend(rules);
        self
    }

    /// Get all BLOCK rules sorted by priority
    pub fn block_rules(&self) -> Vec<&RuleDefinition> {
        let mut rules: Vec<_> = self
            .rules
            .iter()
            .filter(|r| r.rule_type == RuleType::Block && r.enabled)
            .collect();
        rules.sort_by_key(|r| r.priority);
        rules
    }

    /// Get all FLAG rules sorted by priority
    pub fn flag_rules(&self) -> Vec<&RuleDefinition> {
        let mut rules: Vec<_> = self
            .rules
            .iter()
            .filter(|r| r.rule_type == RuleType::Flag && r.enabled)
            .collect();
        rules.sort_by_key(|r| r.priority);
        rules
    }

    /// Get rule by ID
    pub fn get_rule(&self, id: &str) -> Option<&RuleDefinition> {
        self.rules.iter().find(|r| r.id == id)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_rule_action_block() {
        let action = RuleAction::block("SANCTIONS", "User on watchlist");
        assert!(action.is_block());
        assert!(!action.is_flag());

        let decision = action.to_decision();
        assert!(decision.is_blocked());
    }

    #[test]
    fn test_rule_action_flag() {
        let action = RuleAction::flag(RiskScore::High, ApprovalLevel::L2, "Large transaction");
        assert!(action.is_flag());
        assert!(!action.is_block());

        let decision = action.to_decision();
        assert!(decision.is_flagged());
    }

    #[test]
    fn test_condition_amount() {
        let cond = Condition::amount_gte(dec!(10000));
        assert!(matches!(cond, Condition::AmountGte { threshold } if threshold == dec!(10000)));
    }

    #[test]
    fn test_condition_all() {
        let cond = Condition::all(vec![
            Condition::amount_gte(dec!(5000)),
            Condition::account_age_lt(7),
        ]);

        if let Condition::All { conditions } = cond {
            assert_eq!(conditions.len(), 2);
        } else {
            panic!("Expected All condition");
        }
    }

    #[test]
    fn test_rule_builder() {
        let rule = RuleDefinition::builder("LARGE_TX")
            .name("Large Transaction Alert")
            .description("Flag transactions >= 10,000")
            .flag_rule()
            .when(Condition::amount_gte(dec!(10000)))
            .then(RuleAction::flag(
                RiskScore::Medium,
                ApprovalLevel::L1,
                "Large transaction detected",
            ))
            .priority(50)
            .build();

        assert_eq!(rule.id, "LARGE_TX");
        assert_eq!(rule.name, "Large Transaction Alert");
        assert_eq!(rule.rule_type, RuleType::Flag);
        assert_eq!(rule.priority, 50);
        assert!(rule.enabled);
    }

    #[test]
    fn test_rule_set() {
        let ruleset = RuleSet::new("AML_BASIC")
            .with_description("Basic AML rules")
            .add_rule(
                RuleDefinition::builder("SANCTIONS")
                    .block_rule()
                    .when(Condition::is_watchlisted())
                    .then(RuleAction::block("SANCTIONS", "User on watchlist"))
                    .priority(10)
                    .build(),
            )
            .add_rule(
                RuleDefinition::builder("LARGE_TX")
                    .flag_rule()
                    .when(Condition::amount_gte(dec!(10000)))
                    .then(RuleAction::flag(
                        RiskScore::Medium,
                        ApprovalLevel::L1,
                        "Large transaction",
                    ))
                    .priority(50)
                    .build(),
            );

        assert_eq!(ruleset.name, "AML_BASIC");
        assert_eq!(ruleset.rules.len(), 2);
        assert_eq!(ruleset.block_rules().len(), 1);
        assert_eq!(ruleset.flag_rules().len(), 1);
    }

    #[test]
    fn test_rule_set_get_by_id() {
        let ruleset = RuleSet::new("TEST")
            .add_rule(
                RuleDefinition::builder("RULE_1")
                    .flag_rule()
                    .when(Condition::amount_gte(dec!(100)))
                    .then(RuleAction::Approve)
                    .build(),
            );

        assert!(ruleset.get_rule("RULE_1").is_some());
        assert!(ruleset.get_rule("RULE_2").is_none());
    }

    #[test]
    fn test_rule_serialization() {
        let rule = RuleDefinition::builder("TEST")
            .when(Condition::amount_gte(dec!(1000)))
            .then(RuleAction::Approve)
            .build();

        let json = serde_json::to_string(&rule).unwrap();
        let parsed: RuleDefinition = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, rule.id);
    }
}

```

## File ./bibank\crates\events\src\error.rs:
```rust
//! Event store errors

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EventError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Event store not initialized")]
    NotInitialized,

    #[error("Invalid event file: {0}")]
    InvalidFile(String),
}

```

## File ./bibank\crates\events\src\lib.rs:
```rust
//! BiBank Events - JSONL event store
//!
//! This crate handles persistence of journal entries to JSONL files.
//! JSONL is the Source of Truth - SQLite projections are disposable.

pub mod error;
pub mod reader;
pub mod store;

pub use error::EventError;
pub use reader::EventReader;
pub use store::EventStore;

```

## File ./bibank\crates\events\src\reader.rs:
```rust
//! JSONL event reader - sequential reader for replay

use crate::error::EventError;
use bibank_ledger::JournalEntry;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Sequential event reader for replay
pub struct EventReader {
    files: Vec<std::path::PathBuf>,
}

impl EventReader {
    /// Create a new reader from a directory
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, EventError> {
        let path = path.as_ref();
        let mut files = Vec::new();

        if path.exists() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let file_path = entry.path();
                if file_path.extension().map_or(false, |ext| ext == "jsonl") {
                    files.push(file_path);
                }
            }
        }

        files.sort();

        Ok(Self { files })
    }

    /// Read all entries from all files in order
    pub fn read_all(&self) -> Result<Vec<JournalEntry>, EventError> {
        let mut entries = Vec::new();

        for file_path in &self.files {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let entry: JournalEntry = serde_json::from_str(&line)?;
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Get the last sequence number from all files
    pub fn last_sequence(&self) -> Result<Option<u64>, EventError> {
        if self.files.is_empty() {
            return Ok(None);
        }

        // Read from the last file
        let last_file = &self.files[self.files.len() - 1];
        let file = File::open(last_file)?;
        let reader = BufReader::new(file);

        let mut last_seq = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: JournalEntry = serde_json::from_str(&line)?;
            last_seq = Some(entry.sequence);
        }

        Ok(last_seq)
    }

    /// Get the last entry (for prev_hash)
    pub fn last_entry(&self) -> Result<Option<JournalEntry>, EventError> {
        let entries = self.read_all()?;
        Ok(entries.into_iter().last())
    }

    /// Count total entries across all files
    pub fn count(&self) -> Result<usize, EventError> {
        let mut count = 0;

        for file_path in &self.files {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if !line.trim().is_empty() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

```

## File ./bibank\crates\events\src\store.rs:
```rust
//! JSONL event store - append-only writer

use crate::error::EventError;
use bibank_ledger::JournalEntry;
use chrono::Utc;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Append-only JSONL event store
pub struct EventStore {
    base_path: PathBuf,
    current_file: Option<BufWriter<File>>,
    current_date: Option<String>,
}

impl EventStore {
    /// Create a new event store at the given path
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self, EventError> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;

        Ok(Self {
            base_path,
            current_file: None,
            current_date: None,
        })
    }

    /// Append a journal entry to the store
    pub fn append(&mut self, entry: &JournalEntry) -> Result<(), EventError> {
        let date = entry.timestamp.format("%Y-%m-%d").to_string();

        // Rotate file if date changed
        if self.current_date.as_ref() != Some(&date) {
            self.rotate_file(&date)?;
        }

        // Write entry as JSON line
        if let Some(ref mut writer) = self.current_file {
            let json = serde_json::to_string(entry)?;
            writeln!(writer, "{}", json)?;
            writer.flush()?;
        }

        Ok(())
    }

    /// Rotate to a new file for the given date
    fn rotate_file(&mut self, date: &str) -> Result<(), EventError> {
        // Flush current file
        if let Some(ref mut writer) = self.current_file {
            writer.flush()?;
        }

        // Open new file
        let file_path = self.base_path.join(format!("{}.jsonl", date));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        self.current_file = Some(BufWriter::new(file));
        self.current_date = Some(date.to_string());

        Ok(())
    }

    /// Get the path to today's file
    pub fn today_file_path(&self) -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        self.base_path.join(format!("{}.jsonl", date))
    }

    /// List all JSONL files in the store
    pub fn list_files(&self) -> Result<Vec<PathBuf>, EventError> {
        let mut files = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "jsonl") {
                files.push(path);
            }
        }

        files.sort();
        Ok(files)
    }

    /// Flush and close the current file
    pub fn close(&mut self) -> Result<(), EventError> {
        if let Some(ref mut writer) = self.current_file {
            writer.flush()?;
        }
        self.current_file = None;
        self.current_date = None;
        Ok(())
    }
}

impl Drop for EventStore {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

```

## File ./bibank\crates\hooks\src\aml.rs:
```rust
//! AML-specific hook implementations
//!
//! Built-in hooks for common AML checks:
//! - [`SanctionsHook`] - Pre-validation check against watchlist (BLOCK)
//! - [`PepCheckHook`] - Pre-validation check for PEPs (BLOCK)
//! - [`NewAccountHook`] - Post-commit check for new accounts (FLAG)

use std::collections::HashSet;
use std::sync::RwLock;

use async_trait::async_trait;
use rust_decimal::Decimal;

use bibank_compliance::{AmlDecision, ApprovalLevel, CheckResult, RiskScore};

use crate::context::HookContext;
use crate::error::HookResult;
use crate::traits::{HookDecision, PostCommitHook, PreValidationHook};

// =============================================================================
// SanctionsHook - Pre-validation BLOCK hook
// =============================================================================

/// Sanctions/Watchlist check hook
///
/// This pre-validation hook blocks transactions from users on the watchlist.
/// It runs BEFORE ledger commit and can reject transactions immediately.
pub struct SanctionsHook {
    /// Priority (lower = runs first)
    priority: u32,
    /// Watchlist (user IDs that are sanctioned)
    watchlist: RwLock<HashSet<String>>,
}

impl SanctionsHook {
    /// Create a new sanctions hook with empty watchlist
    pub fn new(priority: u32) -> Self {
        Self {
            priority,
            watchlist: RwLock::new(HashSet::new()),
        }
    }

    /// Add a user to the watchlist
    pub fn add_to_watchlist(&self, user_id: &str) {
        let mut watchlist = self.watchlist.write().unwrap();
        watchlist.insert(user_id.to_string());
    }

    /// Remove a user from the watchlist
    pub fn remove_from_watchlist(&self, user_id: &str) {
        let mut watchlist = self.watchlist.write().unwrap();
        watchlist.remove(user_id);
    }

    /// Check if a user is on the watchlist
    pub fn is_on_watchlist(&self, user_id: &str) -> bool {
        let watchlist = self.watchlist.read().unwrap();
        watchlist.contains(user_id)
    }
}

#[async_trait]
impl PreValidationHook for SanctionsHook {
    fn name(&self) -> &str {
        "sanctions_hook"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn on_pre_validation(&self, ctx: &HookContext) -> HookResult<HookDecision> {
        // Check if user is on watchlist
        if self.is_on_watchlist(&ctx.user_id) {
            return Ok(HookDecision::block(
                format!("User {} is on sanctions watchlist", ctx.user_id),
                "SANCTIONS_BLOCKED",
            ));
        }

        // Also check if flagged in metadata
        if ctx.metadata.is_watchlisted {
            return Ok(HookDecision::block(
                "User is flagged as watchlisted",
                "WATCHLIST_BLOCKED",
            ));
        }

        Ok(HookDecision::Allow)
    }
}

// =============================================================================
// PepCheckHook - Pre-validation BLOCK hook for PEPs
// =============================================================================

/// Politically Exposed Person (PEP) check hook
///
/// Blocks transactions from PEPs above certain thresholds or
/// requires enhanced due diligence.
pub struct PepCheckHook {
    priority: u32,
    /// Threshold above which PEP transactions are blocked
    threshold: Decimal,
}

impl PepCheckHook {
    /// Create a new PEP check hook
    pub fn new(priority: u32, threshold: Decimal) -> Self {
        Self { priority, threshold }
    }
}

#[async_trait]
impl PreValidationHook for PepCheckHook {
    fn name(&self) -> &str {
        "pep_check_hook"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn on_pre_validation(&self, ctx: &HookContext) -> HookResult<HookDecision> {
        // Only check if user is PEP
        if !ctx.metadata.is_pep {
            return Ok(HookDecision::Allow);
        }

        // PEP with transaction above threshold is blocked
        if ctx.amount >= self.threshold {
            return Ok(HookDecision::block(
                format!(
                    "PEP transaction {} {} exceeds threshold {}",
                    ctx.amount, ctx.asset, self.threshold
                ),
                "PEP_THRESHOLD_EXCEEDED",
            ));
        }

        Ok(HookDecision::Allow)
    }
}

// =============================================================================
// NewAccountHook - Post-commit FLAG hook for new accounts
// =============================================================================

/// New account large transaction hook
///
/// Flags large transactions from accounts less than N days old.
pub struct NewAccountHook {
    priority: u32,
    /// Account age threshold in days
    age_threshold_days: i64,
    /// Amount threshold for flagging
    amount_threshold: Decimal,
}

impl NewAccountHook {
    /// Create a new account hook
    pub fn new(priority: u32, age_threshold_days: i64, amount_threshold: Decimal) -> Self {
        Self {
            priority,
            age_threshold_days,
            amount_threshold,
        }
    }
}

#[async_trait]
impl PostCommitHook for NewAccountHook {
    fn name(&self) -> &str {
        "new_account_hook"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn on_post_commit(&self, ctx: &HookContext) -> HookResult<CheckResult> {
        // Check if account is new and transaction is large
        let account_age = ctx.metadata.account_age_days.unwrap_or(365);
        let is_new_account = account_age < self.age_threshold_days;
        let is_large_amount = ctx.amount >= self.amount_threshold;

        if is_new_account && is_large_amount {
            return Ok(CheckResult {
                decision: AmlDecision::flagged(
                    format!(
                        "Account {} days old, transaction {} {} >= {}",
                        account_age, ctx.amount, ctx.asset, self.amount_threshold
                    ),
                    RiskScore::Medium,
                    ApprovalLevel::L1,
                ),
                rules_triggered: vec!["NEW_ACCOUNT_LARGE_TX".to_string()],
                risk_score: Some(RiskScore::Medium),
            });
        }

        Ok(CheckResult {
            decision: AmlDecision::Approved,
            rules_triggered: vec![],
            risk_score: None,
        })
    }
}

// =============================================================================
// LargeTxHook - Post-commit FLAG hook for large transactions
// =============================================================================

/// Large transaction monitoring hook
///
/// Flags transactions above a configurable threshold.
pub struct LargeTxHook {
    priority: u32,
    /// Threshold for flagging
    threshold: Decimal,
}

impl LargeTxHook {
    /// Create a new large transaction hook
    pub fn new(priority: u32, threshold: Decimal) -> Self {
        Self { priority, threshold }
    }
}

#[async_trait]
impl PostCommitHook for LargeTxHook {
    fn name(&self) -> &str {
        "large_tx_hook"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn on_post_commit(&self, ctx: &HookContext) -> HookResult<CheckResult> {
        if ctx.amount >= self.threshold {
            return Ok(CheckResult {
                decision: AmlDecision::flagged(
                    format!("Large transaction: {} {} >= threshold {}", ctx.amount, ctx.asset, self.threshold),
                    RiskScore::Medium,
                    ApprovalLevel::L1,
                ),
                rules_triggered: vec!["LARGE_TX_ALERT".to_string()],
                risk_score: Some(RiskScore::Medium),
            });
        }

        Ok(CheckResult {
            decision: AmlDecision::Approved,
            rules_triggered: vec![],
            risk_score: None,
        })
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::HookMetadata;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_sanctions_hook_allow() {
        let hook = SanctionsHook::new(10);
        let ctx = HookContext::new(
            "corr-1",
            "user-clean",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        );

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_sanctions_hook_block() {
        let hook = SanctionsHook::new(10);
        hook.add_to_watchlist("user-bad");

        let ctx = HookContext::new(
            "corr-1",
            "user-bad",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        );

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_blocked());
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "SANCTIONS_BLOCKED");
        }
    }

    #[tokio::test]
    async fn test_sanctions_hook_remove_from_watchlist() {
        let hook = SanctionsHook::new(10);
        hook.add_to_watchlist("user-temp");
        assert!(hook.is_on_watchlist("user-temp"));

        hook.remove_from_watchlist("user-temp");
        assert!(!hook.is_on_watchlist("user-temp"));
    }

    #[tokio::test]
    async fn test_sanctions_hook_metadata_watchlist() {
        let hook = SanctionsHook::new(10);
        let mut metadata = HookMetadata::default();
        metadata.is_watchlisted = true;

        let ctx = HookContext::new(
            "corr-1",
            "user-x",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        ).with_metadata(metadata);

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_blocked());
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "WATCHLIST_BLOCKED");
        }
    }

    #[tokio::test]
    async fn test_pep_hook_non_pep() {
        let hook = PepCheckHook::new(20, dec!(10000));
        let ctx = HookContext::new(
            "corr-1",
            "user-normal",
            "DEPOSIT",
            dec!(50000),
            "USDT",
        );

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_pep_hook_pep_below_threshold() {
        let hook = PepCheckHook::new(20, dec!(10000));
        let mut metadata = HookMetadata::default();
        metadata.is_pep = true;

        let ctx = HookContext::new(
            "corr-1",
            "user-pep",
            "DEPOSIT",
            dec!(5000),
            "USDT",
        ).with_metadata(metadata);

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_pep_hook_pep_above_threshold() {
        let hook = PepCheckHook::new(20, dec!(10000));
        let mut metadata = HookMetadata::default();
        metadata.is_pep = true;

        let ctx = HookContext::new(
            "corr-1",
            "user-pep",
            "DEPOSIT",
            dec!(15000),
            "USDT",
        ).with_metadata(metadata);

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_blocked());
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "PEP_THRESHOLD_EXCEEDED");
        }
    }

    #[tokio::test]
    async fn test_new_account_hook_old_account() {
        let hook = NewAccountHook::new(30, 7, dec!(5000));
        let ctx = HookContext::new(
            "corr-1",
            "user-old",
            "DEPOSIT",
            dec!(10000),
            "USDT",
        ).with_account_age(30);

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[tokio::test]
    async fn test_new_account_hook_new_small() {
        let hook = NewAccountHook::new(30, 7, dec!(5000));
        let ctx = HookContext::new(
            "corr-1",
            "user-new",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        ).with_account_age(2);

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[tokio::test]
    async fn test_new_account_hook_new_large() {
        let hook = NewAccountHook::new(30, 7, dec!(5000));
        let ctx = HookContext::new(
            "corr-1",
            "user-new",
            "DEPOSIT",
            dec!(10000),
            "USDT",
        ).with_account_age(2);

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_flagged());
        assert!(result.rules_triggered.contains(&"NEW_ACCOUNT_LARGE_TX".to_string()));
    }

    #[tokio::test]
    async fn test_large_tx_hook_below_threshold() {
        let hook = LargeTxHook::new(40, dec!(10000));
        let ctx = HookContext::new(
            "corr-1",
            "user-1",
            "DEPOSIT",
            dec!(5000),
            "USDT",
        );

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[tokio::test]
    async fn test_large_tx_hook_above_threshold() {
        let hook = LargeTxHook::new(40, dec!(10000));
        let ctx = HookContext::new(
            "corr-1",
            "user-1",
            "DEPOSIT",
            dec!(15000),
            "USDT",
        );

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_flagged());
        assert!(result.rules_triggered.contains(&"LARGE_TX_ALERT".to_string()));
    }
}

```

## File ./bibank\crates\hooks\src\context.rs:
```rust
//! Hook context - data passed to hooks

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Context passed to all hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// Unique correlation ID for this transaction
    pub correlation_id: String,

    /// User initiating the transaction
    pub user_id: String,

    /// Transaction intent type
    pub intent: String,

    /// Transaction amount
    pub amount: Decimal,

    /// Asset code (e.g., "USDT", "BTC")
    pub asset: String,

    /// Destination account (if applicable)
    pub destination: Option<String>,

    /// Timestamp of the transaction
    pub timestamp: DateTime<Utc>,

    /// Additional metadata
    pub metadata: HookMetadata,
}

/// Additional metadata for hooks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookMetadata {
    /// User's account age in days
    pub account_age_days: Option<i64>,

    /// User's KYC level (0-4)
    pub kyc_level: Option<u8>,

    /// Whether user is on internal watchlist
    pub is_watchlisted: bool,

    /// Whether user is a PEP (Politically Exposed Person)
    pub is_pep: bool,

    /// Source IP address
    pub source_ip: Option<String>,

    /// Device fingerprint
    pub device_id: Option<String>,
}

impl HookContext {
    /// Create a new hook context
    pub fn new(
        correlation_id: impl Into<String>,
        user_id: impl Into<String>,
        intent: impl Into<String>,
        amount: Decimal,
        asset: impl Into<String>,
    ) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            user_id: user_id.into(),
            intent: intent.into(),
            amount,
            asset: asset.into(),
            destination: None,
            timestamp: Utc::now(),
            metadata: HookMetadata::default(),
        }
    }

    /// Set destination account
    pub fn with_destination(mut self, dest: impl Into<String>) -> Self {
        self.destination = Some(dest.into());
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: HookMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set account age
    pub fn with_account_age(mut self, days: i64) -> Self {
        self.metadata.account_age_days = Some(days);
        self
    }

    /// Set KYC level
    pub fn with_kyc_level(mut self, level: u8) -> Self {
        self.metadata.kyc_level = Some(level);
        self
    }

    /// Mark as watchlisted
    pub fn with_watchlist(mut self, watchlisted: bool) -> Self {
        self.metadata.is_watchlisted = watchlisted;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_context_creation() {
        let ctx = HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT");

        assert_eq!(ctx.correlation_id, "TX-001");
        assert_eq!(ctx.user_id, "USER-001");
        assert_eq!(ctx.intent, "Deposit");
        assert_eq!(ctx.amount, dec!(1000));
        assert_eq!(ctx.asset, "USDT");
        assert!(ctx.destination.is_none());
    }

    #[test]
    fn test_context_builder() {
        let ctx = HookContext::new("TX-001", "USER-001", "Withdrawal", dec!(5000), "USDT")
            .with_destination("external_address")
            .with_account_age(30)
            .with_kyc_level(2)
            .with_watchlist(false);

        assert_eq!(ctx.destination, Some("external_address".to_string()));
        assert_eq!(ctx.metadata.account_age_days, Some(30));
        assert_eq!(ctx.metadata.kyc_level, Some(2));
        assert!(!ctx.metadata.is_watchlisted);
    }

    #[test]
    fn test_context_serialization() {
        let ctx = HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT");
        let json = serde_json::to_string(&ctx).unwrap();

        assert!(json.contains("TX-001"));
        assert!(json.contains("USER-001"));

        let parsed: HookContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.correlation_id, ctx.correlation_id);
    }
}

```

## File ./bibank\crates\hooks\src\error.rs:
```rust
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

```

## File ./bibank\crates\hooks\src\executor.rs:
```rust
//! Transaction Executor - Orchestrates the full transaction lifecycle
//!
//! This module provides the main entry point for processing transactions
//! with compliance hooks:
//!
//! ```text
//! TransactionIntent
//!        │
//!        ▼
//! ┌─────────────────┐
//! │ Pre-validation  │──► Block? Return error
//! │ Hooks           │
//! └────────┬────────┘
//!          │ Allow
//!          ▼
//! ┌─────────────────┐
//! │ Ledger Commit   │──► Append to Journal
//! │                 │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ Post-commit     │──► Flag? Apply Lock
//! │ Hooks           │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ Compliance      │──► Write decision
//! │ Ledger          │
//! └─────────────────┘
//! ```

use std::sync::Arc;

use bibank_compliance::{AmlDecision, ComplianceEngine};
use tokio::sync::RwLock;

use crate::context::HookContext;
use crate::error::HookResult;
use crate::registry::HookRegistry;
use crate::traits::HookDecision;

/// Result of executing a transaction through the compliance pipeline
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Original correlation ID
    pub correlation_id: String,
    /// Whether the transaction was committed
    pub committed: bool,
    /// Final compliance decision
    pub decision: AmlDecision,
    /// Rules that were triggered
    pub rules_triggered: Vec<String>,
    /// Whether a lock was applied
    pub lock_applied: bool,
    /// Block reason (if blocked)
    pub block_reason: Option<String>,
}

impl ExecutionResult {
    /// Create a blocked result
    pub fn blocked(correlation_id: String, reason: String) -> Self {
        Self {
            correlation_id,
            committed: false,
            decision: AmlDecision::blocked(&reason, "HOOK_REJECTED"),
            rules_triggered: vec![],
            lock_applied: false,
            block_reason: Some(reason),
        }
    }

    /// Create an approved result
    pub fn approved(correlation_id: String) -> Self {
        Self {
            correlation_id,
            committed: true,
            decision: AmlDecision::Approved,
            rules_triggered: vec![],
            lock_applied: false,
            block_reason: None,
        }
    }

    /// Create a flagged result with lock
    pub fn flagged(correlation_id: String, decision: AmlDecision, rules: Vec<String>) -> Self {
        Self {
            correlation_id,
            committed: true,
            decision,
            rules_triggered: rules,
            lock_applied: true,
            block_reason: None,
        }
    }
}

/// Transaction executor with compliance hooks
///
/// This is the main orchestrator for processing transactions with:
/// - Pre-validation hooks (BLOCK rules)
/// - Ledger commit simulation
/// - Post-commit hooks (FLAG rules)
/// - Lock application for flagged transactions
pub struct TransactionExecutor {
    /// Hook registry
    registry: Arc<HookRegistry>,
    /// Compliance engine (optional - for full rule evaluation)
    #[allow(dead_code)]
    compliance_engine: Option<Arc<RwLock<ComplianceEngine>>>,
}

impl TransactionExecutor {
    /// Create a new executor with just hooks
    pub fn new(registry: Arc<HookRegistry>) -> Self {
        Self {
            registry,
            compliance_engine: None,
        }
    }

    /// Create an executor with full compliance engine
    pub fn with_compliance_engine(
        registry: Arc<HookRegistry>,
        engine: Arc<RwLock<ComplianceEngine>>,
    ) -> Self {
        Self {
            registry,
            compliance_engine: Some(engine),
        }
    }

    /// Execute a transaction through the full compliance pipeline
    ///
    /// Returns:
    /// - `Ok(ExecutionResult)` with the outcome (committed, blocked, flagged)
    /// - `Err(HookError)` if hooks fail and FailPolicy is FailClosed
    pub async fn execute(&self, ctx: &HookContext) -> HookResult<ExecutionResult> {
        let correlation_id = ctx.correlation_id.clone();

        // === Phase 1: Pre-validation hooks ===
        let pre_decision = self.registry.run_pre_validation(ctx).await?;

        match pre_decision {
            HookDecision::Block { reason, .. } => {
                // Transaction blocked - never committed
                return Ok(ExecutionResult::blocked(correlation_id, reason));
            }
            HookDecision::Allow => {
                // Continue to commit
            }
        }

        // === Phase 2: Ledger Commit (simulated) ===
        // In real implementation, this would call bibank-ledger
        // to append the transaction to the journal

        // === Phase 3: Post-commit hooks ===
        let post_result = self.registry.run_post_commit(ctx).await?;

        // Aggregate decision (using max lattice)
        let final_decision = post_result.decision;
        let rules = post_result.rules_triggered;

        // === Phase 4: Apply lock if flagged ===
        if final_decision.is_flagged() {
            // In real implementation, this would:
            // 1. Write LockApplied event to main journal
            // 2. Write TransactionFlagged event to compliance ledger

            Ok(ExecutionResult::flagged(
                correlation_id,
                final_decision,
                rules,
            ))
        } else {
            Ok(ExecutionResult::approved(correlation_id))
        }
    }

    /// Execute a batch of transactions
    pub async fn execute_batch(
        &self,
        contexts: &[HookContext],
    ) -> Vec<HookResult<ExecutionResult>> {
        let mut results = Vec::with_capacity(contexts.len());

        for ctx in contexts {
            results.push(self.execute(ctx).await);
        }

        results
    }
}

/// Builder for TransactionExecutor
pub struct ExecutorBuilder {
    registry: HookRegistry,
    compliance_engine: Option<Arc<RwLock<ComplianceEngine>>>,
}

impl ExecutorBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            registry: HookRegistry::new(),
            compliance_engine: None,
        }
    }

    /// Set the hook registry
    pub fn with_registry(mut self, registry: HookRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Add a compliance engine
    pub fn with_compliance_engine(mut self, engine: ComplianceEngine) -> Self {
        self.compliance_engine = Some(Arc::new(RwLock::new(engine)));
        self
    }

    /// Build the executor
    pub fn build(self) -> TransactionExecutor {
        let registry = Arc::new(self.registry);

        match self.compliance_engine {
            Some(engine) => TransactionExecutor::with_compliance_engine(registry, engine),
            None => TransactionExecutor::new(registry),
        }
    }
}

impl Default for ExecutorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aml::{NewAccountHook, PepCheckHook, SanctionsHook, LargeTxHook};
    use crate::context::HookMetadata;
    use rust_decimal_macros::dec;

    fn create_registry_with_hooks() -> HookRegistry {
        let mut registry = HookRegistry::new();

        // Pre-validation hooks
        let sanctions = SanctionsHook::new(10);
        sanctions.add_to_watchlist("blocked-user");
        registry.register_pre_hook(Arc::new(sanctions));
        registry.register_pre_hook(Arc::new(PepCheckHook::new(20, dec!(10000))));

        // Post-commit hooks
        registry.register_post_hook(Arc::new(NewAccountHook::new(30, 7, dec!(5000))));
        registry.register_post_hook(Arc::new(LargeTxHook::new(40, dec!(10000))));

        registry
    }

    #[tokio::test]
    async fn test_executor_approved() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let ctx = HookContext::new(
            "tx-001",
            "clean-user",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        ).with_account_age(30);

        let result = executor.execute(&ctx).await.unwrap();

        assert!(result.committed);
        assert!(result.decision.is_approved());
        assert!(!result.lock_applied);
        assert!(result.block_reason.is_none());
    }

    #[tokio::test]
    async fn test_executor_blocked_by_sanctions() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let ctx = HookContext::new(
            "tx-002",
            "blocked-user",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        );

        let result = executor.execute(&ctx).await.unwrap();

        assert!(!result.committed);
        assert!(result.decision.is_blocked());
        assert!(result.block_reason.is_some());
        assert!(result.block_reason.unwrap().contains("sanctions"));
    }

    #[tokio::test]
    async fn test_executor_blocked_by_pep() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let mut metadata = HookMetadata::default();
        metadata.is_pep = true;

        let ctx = HookContext::new(
            "tx-003",
            "pep-user",
            "DEPOSIT",
            dec!(50000),
            "USDT",
        ).with_metadata(metadata);

        let result = executor.execute(&ctx).await.unwrap();

        assert!(!result.committed);
        assert!(result.decision.is_blocked());
    }

    #[tokio::test]
    async fn test_executor_flagged_by_large_tx() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let ctx = HookContext::new(
            "tx-004",
            "normal-user",
            "DEPOSIT",
            dec!(15000),
            "USDT",
        ).with_account_age(30);

        let result = executor.execute(&ctx).await.unwrap();

        assert!(result.committed);
        assert!(result.decision.is_flagged());
        assert!(result.lock_applied);
        assert!(result.rules_triggered.contains(&"LARGE_TX_ALERT".to_string()));
    }

    #[tokio::test]
    async fn test_executor_flagged_by_new_account() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let ctx = HookContext::new(
            "tx-005",
            "new-user",
            "DEPOSIT",
            dec!(8000),
            "USDT",
        ).with_account_age(2);

        let result = executor.execute(&ctx).await.unwrap();

        assert!(result.committed);
        assert!(result.decision.is_flagged());
        assert!(result.lock_applied);
        assert!(result.rules_triggered.contains(&"NEW_ACCOUNT_LARGE_TX".to_string()));
    }

    #[tokio::test]
    async fn test_executor_batch() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let contexts = vec![
            HookContext::new("tx-001", "user-1", "DEPOSIT", dec!(100), "USDT")
                .with_account_age(30),
            HookContext::new("tx-002", "blocked-user", "DEPOSIT", dec!(100), "USDT"),
            HookContext::new("tx-003", "user-3", "DEPOSIT", dec!(20000), "USDT")
                .with_account_age(30),
        ];

        let results = executor.execute_batch(&contexts).await;

        assert_eq!(results.len(), 3);

        // First: approved
        assert!(results[0].as_ref().unwrap().committed);
        assert!(results[0].as_ref().unwrap().decision.is_approved());

        // Second: blocked
        assert!(!results[1].as_ref().unwrap().committed);
        assert!(results[1].as_ref().unwrap().decision.is_blocked());

        // Third: flagged
        assert!(results[2].as_ref().unwrap().committed);
        assert!(results[2].as_ref().unwrap().decision.is_flagged());
    }

    #[tokio::test]
    async fn test_builder() {
        let registry = create_registry_with_hooks();
        let executor = ExecutorBuilder::new()
            .with_registry(registry)
            .build();

        let ctx = HookContext::new(
            "tx-001",
            "clean-user",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        ).with_account_age(30);

        let result = executor.execute(&ctx).await.unwrap();
        assert!(result.committed);
    }
}

```

## File ./bibank\crates\hooks\src\lib.rs:
```rust
//! BiBank Hooks - Transaction Lifecycle Hooks
//!
//! Provides hook points for AML/Compliance checks at critical transaction stages:
//!
//! ```text
//! Intent Created
//!     │
//!     ▼
//! ┌─────────────────────────────┐
//! │ PRE_VALIDATION_HOOK         │ ← BLOCK rules (sanctions, KYC limits)
//! │ (reject immediately)        │
//! └─────────────────────────────┘
//!     │
//!     ▼
//! ┌─────────────────────────────┐
//! │ LEDGER COMMIT               │
//! └─────────────────────────────┘
//!     │
//!     ▼
//! ┌─────────────────────────────┐
//! │ POST_COMMIT_HOOK            │ ← FLAG rules (structuring, velocity)
//! │ (lock funds if flagged)     │
//! └─────────────────────────────┘
//! ```

pub mod aml;
pub mod context;
pub mod error;
pub mod executor;
pub mod registry;
pub mod traits;

pub use aml::{LargeTxHook, NewAccountHook, PepCheckHook, SanctionsHook};
pub use context::{HookContext, HookMetadata};
pub use error::{HookError, HookResult};
pub use executor::{ExecutionResult, ExecutorBuilder, TransactionExecutor};
pub use registry::HookRegistry;
pub use traits::{HookDecision, PostCommitHook, PreValidationHook};

```

## File ./bibank\crates\hooks\src\registry.rs:
```rust
//! Hook Registry - manages and executes hooks in order

use std::sync::Arc;

use bibank_compliance::{AmlDecision, CheckResult, FailPolicy};

use crate::context::HookContext;
use crate::error::HookResult;
use crate::traits::{HookDecision, PostCommitHook, PreValidationHook};

/// Registry for managing hooks
///
/// Hooks are executed in priority order (lower = first).
/// Pre-validation hooks can BLOCK transactions.
/// Post-commit hooks can FLAG transactions for review.
pub struct HookRegistry {
    /// Pre-validation hooks (run before ledger commit)
    pre_hooks: Vec<Arc<dyn PreValidationHook>>,

    /// Post-commit hooks (run after ledger commit)
    post_hooks: Vec<Arc<dyn PostCommitHook>>,

    /// Policy when hooks fail
    fail_policy: FailPolicy,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
            fail_policy: FailPolicy::FailClosed,
        }
    }

    /// Set the fail policy
    pub fn with_fail_policy(mut self, policy: FailPolicy) -> Self {
        self.fail_policy = policy;
        self
    }

    /// Register a pre-validation hook
    pub fn register_pre_hook(&mut self, hook: Arc<dyn PreValidationHook>) {
        self.pre_hooks.push(hook);
        // Sort by priority
        self.pre_hooks.sort_by_key(|h| h.priority());
    }

    /// Register a post-commit hook
    pub fn register_post_hook(&mut self, hook: Arc<dyn PostCommitHook>) {
        self.post_hooks.push(hook);
        // Sort by priority
        self.post_hooks.sort_by_key(|h| h.priority());
    }

    /// Run all pre-validation hooks
    ///
    /// Returns Block if any hook blocks, Allow if all pass.
    /// On hook failure, behavior depends on fail_policy.
    pub async fn run_pre_validation(&self, ctx: &HookContext) -> HookResult<HookDecision> {
        for hook in &self.pre_hooks {
            let result = hook.on_pre_validation(ctx).await;

            match result {
                Ok(HookDecision::Allow) => {
                    tracing::debug!(hook = hook.name(), "Pre-validation hook passed");
                    continue;
                }
                Ok(HookDecision::Block { reason, code }) => {
                    tracing::warn!(
                        hook = hook.name(),
                        reason = %reason,
                        code = %code,
                        "Pre-validation hook blocked transaction"
                    );
                    return Ok(HookDecision::Block { reason, code });
                }
                Err(e) => {
                    tracing::error!(
                        hook = hook.name(),
                        error = %e,
                        "Pre-validation hook failed"
                    );

                    match self.fail_policy {
                        FailPolicy::FailClosed => {
                            return Ok(HookDecision::block(
                                format!("Hook {} failed: {}", hook.name(), e),
                                "HOOK_FAILURE",
                            ));
                        }
                        FailPolicy::FailOpen => {
                            tracing::warn!(
                                hook = hook.name(),
                                "FailOpen: continuing despite hook failure"
                            );
                            continue;
                        }
                    }
                }
            }
        }

        Ok(HookDecision::Allow)
    }

    /// Run all post-commit hooks
    ///
    /// Returns aggregated check result from all hooks.
    pub async fn run_post_commit(&self, ctx: &HookContext) -> HookResult<CheckResult> {
        let mut all_decisions = Vec::new();
        let mut all_rules = Vec::new();
        let mut highest_risk = None;

        for hook in &self.post_hooks {
            let result = hook.on_post_commit(ctx).await;

            match result {
                Ok(check) => {
                    tracing::debug!(
                        hook = hook.name(),
                        decision = ?check.decision,
                        "Post-commit hook completed"
                    );

                    all_decisions.push(check.decision.clone());
                    all_rules.extend(check.rules_triggered);

                    if let Some(score) = check.risk_score {
                        highest_risk = match highest_risk {
                            None => Some(score),
                            Some(existing) if score > existing => Some(score),
                            Some(existing) => Some(existing),
                        };
                    }
                }
                Err(e) => {
                    tracing::error!(
                        hook = hook.name(),
                        error = %e,
                        "Post-commit hook failed"
                    );

                    // Post-commit hooks don't block, so we just log
                    // and continue even on FailClosed
                    all_rules.push(format!("HOOK_ERROR:{}", hook.name()));
                }
            }
        }

        // Aggregate decisions (most restrictive wins)
        let decision = AmlDecision::aggregate(all_decisions);

        Ok(CheckResult {
            decision,
            rules_triggered: all_rules,
            risk_score: highest_risk,
        })
    }

    /// Get number of registered pre-validation hooks
    pub fn pre_hook_count(&self) -> usize {
        self.pre_hooks.len()
    }

    /// Get number of registered post-commit hooks
    pub fn post_hook_count(&self) -> usize {
        self.post_hooks.len()
    }

    /// Get current fail policy
    pub fn fail_policy(&self) -> FailPolicy {
        self.fail_policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{NoOpPostCommitHook, NoOpPreValidationHook};
    use rust_decimal_macros::dec;

    fn create_ctx() -> HookContext {
        HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT")
    }

    #[test]
    fn test_empty_registry() {
        let registry = HookRegistry::new();
        assert_eq!(registry.pre_hook_count(), 0);
        assert_eq!(registry.post_hook_count(), 0);
    }

    #[test]
    fn test_register_hooks() {
        let mut registry = HookRegistry::new();

        registry.register_pre_hook(Arc::new(NoOpPreValidationHook));
        registry.register_post_hook(Arc::new(NoOpPostCommitHook));

        assert_eq!(registry.pre_hook_count(), 1);
        assert_eq!(registry.post_hook_count(), 1);
    }

    #[tokio::test]
    async fn test_run_empty_pre_validation() {
        let registry = HookRegistry::new();
        let ctx = create_ctx();

        let result = registry.run_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_run_noop_pre_validation() {
        let mut registry = HookRegistry::new();
        registry.register_pre_hook(Arc::new(NoOpPreValidationHook));

        let ctx = create_ctx();
        let result = registry.run_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_run_empty_post_commit() {
        let registry = HookRegistry::new();
        let ctx = create_ctx();

        let result = registry.run_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[tokio::test]
    async fn test_run_noop_post_commit() {
        let mut registry = HookRegistry::new();
        registry.register_post_hook(Arc::new(NoOpPostCommitHook));

        let ctx = create_ctx();
        let result = registry.run_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    // Test blocking hook
    struct BlockingHook;

    #[async_trait::async_trait]
    impl PreValidationHook for BlockingHook {
        fn name(&self) -> &str {
            "BlockingHook"
        }

        async fn on_pre_validation(&self, _ctx: &HookContext) -> HookResult<HookDecision> {
            Ok(HookDecision::block("Test block", "TEST-001"))
        }
    }

    #[tokio::test]
    async fn test_blocking_hook() {
        let mut registry = HookRegistry::new();
        registry.register_pre_hook(Arc::new(BlockingHook));

        let ctx = create_ctx();
        let result = registry.run_pre_validation(&ctx).await.unwrap();

        assert!(result.is_blocked());
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "TEST-001");
        }
    }

    // Test hook ordering
    struct PriorityHook {
        priority: u32,
        should_block: bool,
    }

    #[async_trait::async_trait]
    impl PreValidationHook for PriorityHook {
        fn name(&self) -> &str {
            "PriorityHook"
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        async fn on_pre_validation(&self, _ctx: &HookContext) -> HookResult<HookDecision> {
            if self.should_block {
                Ok(HookDecision::block(
                    format!("Blocked by priority {}", self.priority),
                    format!("P{}", self.priority),
                ))
            } else {
                Ok(HookDecision::Allow)
            }
        }
    }

    #[tokio::test]
    async fn test_hook_priority_order() {
        let mut registry = HookRegistry::new();

        // Register in reverse order to test sorting
        registry.register_pre_hook(Arc::new(PriorityHook {
            priority: 200,
            should_block: true,
        }));
        registry.register_pre_hook(Arc::new(PriorityHook {
            priority: 50,
            should_block: true,
        }));
        registry.register_pre_hook(Arc::new(PriorityHook {
            priority: 100,
            should_block: true,
        }));

        let ctx = create_ctx();
        let result = registry.run_pre_validation(&ctx).await.unwrap();

        // Should be blocked by priority 50 (runs first)
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "P50");
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_fail_policy_default() {
        let registry = HookRegistry::new();
        assert_eq!(registry.fail_policy(), FailPolicy::FailClosed);
    }

    #[test]
    fn test_fail_policy_custom() {
        let registry = HookRegistry::new().with_fail_policy(FailPolicy::FailOpen);
        assert_eq!(registry.fail_policy(), FailPolicy::FailOpen);
    }
}

```

## File ./bibank\crates\hooks\src\traits.rs:
```rust
//! Hook traits - interfaces for implementing hooks

use async_trait::async_trait;

use crate::context::HookContext;
use crate::error::HookResult;
use bibank_compliance::{AmlDecision, CheckResult};

/// Decision from pre-validation hook
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    /// Allow transaction to proceed
    Allow,
    /// Block transaction with reason
    Block { reason: String, code: String },
}

impl HookDecision {
    /// Create a block decision
    pub fn block(reason: impl Into<String>, code: impl Into<String>) -> Self {
        HookDecision::Block {
            reason: reason.into(),
            code: code.into(),
        }
    }

    /// Check if this is an allow decision
    pub fn is_allowed(&self) -> bool {
        matches!(self, HookDecision::Allow)
    }

    /// Check if this is a block decision
    pub fn is_blocked(&self) -> bool {
        matches!(self, HookDecision::Block { .. })
    }
}

/// Pre-validation hook - runs BEFORE ledger commit
///
/// Use for BLOCK rules that should reject transactions immediately:
/// - Sanctions/Watchlist checks
/// - KYC limit enforcement
/// - Hard policy violations
///
/// If any pre-validation hook returns Block, the transaction is rejected
/// and never enters the ledger.
#[async_trait]
pub trait PreValidationHook: Send + Sync {
    /// Hook name for logging/debugging
    fn name(&self) -> &str;

    /// Priority (lower = runs first)
    fn priority(&self) -> u32 {
        100
    }

    /// Called before transaction validation
    ///
    /// Return `Ok(HookDecision::Allow)` to proceed
    /// Return `Ok(HookDecision::Block { .. })` to reject
    /// Return `Err(_)` for hook failure (behavior depends on FailPolicy)
    async fn on_pre_validation(&self, ctx: &HookContext) -> HookResult<HookDecision>;
}

/// Post-commit hook - runs AFTER ledger commit
///
/// Use for FLAG rules that should lock funds for review:
/// - Structuring detection
/// - Velocity anomalies
/// - Risk scoring
///
/// These hooks cannot reject the transaction (already committed),
/// but can lock funds and create review requests.
#[async_trait]
pub trait PostCommitHook: Send + Sync {
    /// Hook name for logging/debugging
    fn name(&self) -> &str;

    /// Priority (lower = runs first)
    fn priority(&self) -> u32 {
        100
    }

    /// Called after transaction is committed to ledger
    ///
    /// Returns the compliance check result (may contain FLAG decision)
    async fn on_post_commit(&self, ctx: &HookContext) -> HookResult<CheckResult>;
}

/// A no-op pre-validation hook (for testing)
pub struct NoOpPreValidationHook;

#[async_trait]
impl PreValidationHook for NoOpPreValidationHook {
    fn name(&self) -> &str {
        "NoOpPreValidation"
    }

    async fn on_pre_validation(&self, _ctx: &HookContext) -> HookResult<HookDecision> {
        Ok(HookDecision::Allow)
    }
}

/// A no-op post-commit hook (for testing)
pub struct NoOpPostCommitHook;

#[async_trait]
impl PostCommitHook for NoOpPostCommitHook {
    fn name(&self) -> &str {
        "NoOpPostCommit"
    }

    async fn on_post_commit(&self, _ctx: &HookContext) -> HookResult<CheckResult> {
        Ok(CheckResult {
            decision: AmlDecision::Approved,
            rules_triggered: vec![],
            risk_score: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_hook_decision_allow() {
        let decision = HookDecision::Allow;
        assert!(decision.is_allowed());
        assert!(!decision.is_blocked());
    }

    #[test]
    fn test_hook_decision_block() {
        let decision = HookDecision::block("Sanctions match", "OFAC-001");
        assert!(!decision.is_allowed());
        assert!(decision.is_blocked());

        if let HookDecision::Block { reason, code } = decision {
            assert_eq!(reason, "Sanctions match");
            assert_eq!(code, "OFAC-001");
        }
    }

    #[tokio::test]
    async fn test_noop_pre_validation() {
        let hook = NoOpPreValidationHook;
        let ctx = HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT");

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_noop_post_commit() {
        let hook = NoOpPostCommitHook;
        let ctx = HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT");

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[test]
    fn test_hook_priority() {
        let hook = NoOpPreValidationHook;
        assert_eq!(hook.priority(), 100);
    }
}

```

## File ./bibank\crates\ledger\src\account.rs:
```rust
//! Ledger Account - Hierarchical account identifiers
//!
//! Format: CATEGORY:SEGMENT:ID:ASSET:SUB_ACCOUNT
//! Example: LIAB:USER:ALICE:USDT:AVAILABLE

use crate::entry::Side;
use crate::error::LedgerError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use strum_macros::{Display, EnumString};

/// Account category following standard accounting principles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountCategory {
    /// Assets - Resources owned by the system (Cash, Crypto vault)
    #[strum(serialize = "ASSET")]
    Asset,

    /// Liabilities - Obligations to users (User balances)
    #[strum(serialize = "LIAB")]
    Liability,

    /// Equity - Owner's stake in the system
    #[strum(serialize = "EQUITY")]
    Equity,

    /// Revenue - Income earned (Fees collected)
    #[strum(serialize = "REV")]
    Revenue,

    /// Expenses - Costs incurred
    #[strum(serialize = "EXP")]
    Expense,
}

impl AccountCategory {
    /// Returns the normal balance side for this category.
    ///
    /// - Assets and Expenses increase on Debit
    /// - Liabilities, Equity, and Revenue increase on Credit
    pub fn normal_balance(&self) -> Side {
        match self {
            AccountCategory::Asset | AccountCategory::Expense => Side::Debit,
            AccountCategory::Liability | AccountCategory::Equity | AccountCategory::Revenue => {
                Side::Credit
            }
        }
    }

    /// Short code for serialization
    pub fn code(&self) -> &'static str {
        match self {
            AccountCategory::Asset => "ASSET",
            AccountCategory::Liability => "LIAB",
            AccountCategory::Equity => "EQUITY",
            AccountCategory::Revenue => "REV",
            AccountCategory::Expense => "EXP",
        }
    }
}

/// Hierarchical ledger account key
///
/// Format: `CATEGORY:SEGMENT:ID:ASSET:SUB_ACCOUNT`
///
/// # Examples
/// - `ASSET:SYSTEM:VAULT:USDT:MAIN` - System USDT vault
/// - `LIAB:USER:ALICE:USDT:AVAILABLE` - Alice's available USDT balance
/// - `REV:SYSTEM:FEE:USDT:REVENUE` - USDT fee revenue
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountKey {
    /// Accounting category (ASSET, LIAB, EQUITY, REV, EXP)
    pub category: AccountCategory,

    /// Segment (USER, SYSTEM)
    pub segment: String,

    /// Entity identifier (ALICE, VAULT, FEE)
    pub id: String,

    /// Asset/currency code (USDT, BTC, USD)
    pub asset: String,

    /// Sub-account type (AVAILABLE, LOCKED, MAIN, VAULT, REVENUE)
    pub sub_account: String,
}

impl AccountKey {
    /// Create a new AccountKey
    pub fn new(
        category: AccountCategory,
        segment: impl Into<String>,
        id: impl Into<String>,
        asset: impl Into<String>,
        sub_account: impl Into<String>,
    ) -> Self {
        Self {
            category,
            segment: segment.into().to_uppercase(),
            id: id.into().to_uppercase(),
            asset: asset.into().to_uppercase(),
            sub_account: sub_account.into().to_uppercase(),
        }
    }

    /// Create a user available balance account
    pub fn user_available(user_id: impl Into<String>, asset: impl Into<String>) -> Self {
        Self::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "AVAILABLE",
        )
    }

    /// Create a user locked balance account
    pub fn user_locked(user_id: impl Into<String>, asset: impl Into<String>) -> Self {
        Self::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOCKED",
        )
    }

    /// Create a system vault account
    pub fn system_vault(asset: impl Into<String>) -> Self {
        Self::new(AccountCategory::Asset, "SYSTEM", "VAULT", asset, "MAIN")
    }

    /// Create a fee revenue account
    pub fn fee_revenue(asset: impl Into<String>) -> Self {
        Self::new(AccountCategory::Revenue, "SYSTEM", "FEE", asset, "REVENUE")
    }
}

impl fmt::Display for AccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.category.code(),
            self.segment,
            self.id,
            self.asset,
            self.sub_account
        )
    }
}

impl FromStr for AccountKey {
    type Err = LedgerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 5 {
            return Err(LedgerError::InvalidAccountFormat(format!(
                "Expected 5 parts separated by ':', got {}: {}",
                parts.len(),
                s
            )));
        }

        let category = match parts[0].to_uppercase().as_str() {
            "ASSET" => AccountCategory::Asset,
            "LIAB" => AccountCategory::Liability,
            "EQUITY" => AccountCategory::Equity,
            "REV" => AccountCategory::Revenue,
            "EXP" => AccountCategory::Expense,
            other => return Err(LedgerError::UnknownCategory(other.to_string())),
        };

        Ok(AccountKey {
            category,
            segment: parts[1].to_uppercase(),
            id: parts[2].to_uppercase(),
            asset: parts[3].to_uppercase(),
            sub_account: parts[4].to_uppercase(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_account_key() {
        let key: AccountKey = "LIAB:USER:ALICE:USDT:AVAILABLE".parse().unwrap();
        assert_eq!(key.category, AccountCategory::Liability);
        assert_eq!(key.segment, "USER");
        assert_eq!(key.id, "ALICE");
        assert_eq!(key.asset, "USDT");
        assert_eq!(key.sub_account, "AVAILABLE");
    }

    #[test]
    fn test_account_key_display() {
        let key = AccountKey::user_available("ALICE", "USDT");
        assert_eq!(key.to_string(), "LIAB:USER:ALICE:USDT:AVAILABLE");
    }

    #[test]
    fn test_account_key_roundtrip() {
        let original = "ASSET:SYSTEM:VAULT:BTC:MAIN";
        let key: AccountKey = original.parse().unwrap();
        assert_eq!(key.to_string(), original);
    }

    #[test]
    fn test_normal_balance() {
        assert_eq!(AccountCategory::Asset.normal_balance(), Side::Debit);
        assert_eq!(AccountCategory::Liability.normal_balance(), Side::Credit);
        assert_eq!(AccountCategory::Revenue.normal_balance(), Side::Credit);
        assert_eq!(AccountCategory::Expense.normal_balance(), Side::Debit);
    }

    #[test]
    fn test_invalid_format() {
        let result: Result<AccountKey, _> = "LIAB:USER:ALICE".parse();
        assert!(matches!(result, Err(LedgerError::InvalidAccountFormat(_))));
    }
}

```

## File ./bibank\crates\ledger\src\entry.rs:
```rust
//! Journal Entry - The atomic unit of financial state change

use crate::account::AccountKey;
use crate::error::LedgerError;
use crate::signature::EntrySignature;
use bibank_core::Amount;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transaction intent - Financial primitive (NOT workflow)
///
/// Each intent represents a specific type of financial operation.
/// The ledger validates entries based on their intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionIntent {
    /// System initialization - creates initial balances
    Genesis,

    /// External money entering the system
    Deposit,

    /// External money leaving the system
    Withdrawal,

    /// Internal transfer between accounts
    Transfer,

    /// Exchange between different assets
    Trade,

    /// Fee collection
    Fee,

    /// Manual adjustment (audit-heavy, requires approval)
    Adjustment,

    // === Phase 3: Margin Trading ===

    /// Borrow funds for margin trading
    Borrow,

    /// Repay borrowed funds
    Repay,

    /// Interest accrual on borrowed funds (daily)
    Interest,

    /// Forced liquidation of margin position
    Liquidation,

    /// Order placement (lock funds)
    OrderPlace,

    /// Order cancellation (unlock funds)
    OrderCancel,
}

/// Posting side - Debit or Credit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Debit - increases assets/expenses, decreases liabilities/equity/revenue
    Debit,

    /// Credit - decreases assets/expenses, increases liabilities/equity/revenue
    Credit,
}

/// A single posting within a journal entry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posting {
    /// The ledger account being affected
    pub account: AccountKey,

    /// The amount (always positive)
    pub amount: Amount,

    /// Debit or Credit
    pub side: Side,
}

impl Posting {
    /// Create a new posting
    pub fn new(account: AccountKey, amount: Amount, side: Side) -> Self {
        Self {
            account,
            amount,
            side,
        }
    }

    /// Create a debit posting
    pub fn debit(account: AccountKey, amount: Amount) -> Self {
        Self::new(account, amount, Side::Debit)
    }

    /// Create a credit posting
    pub fn credit(account: AccountKey, amount: Amount) -> Self {
        Self::new(account, amount, Side::Credit)
    }

    /// Get the signed amount for balance calculation
    /// Debit = positive, Credit = negative
    pub fn signed_amount(&self) -> Decimal {
        match self.side {
            Side::Debit => self.amount.value(),
            Side::Credit => -self.amount.value(),
        }
    }
}

/// Journal Entry - The atomic unit of financial state change
///
/// Every entry MUST be double-entry balanced (zero-sum per asset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    // === Ordering & Integrity ===
    /// Global sequence number (strictly increasing)
    pub sequence: u64,

    /// SHA256 hash of previous entry (or "GENESIS" for first entry)
    pub prev_hash: String,

    /// SHA256 hash of this entry's content
    pub hash: String,

    /// Timestamp when the entry was created
    pub timestamp: DateTime<Utc>,

    // === Semantics ===
    /// The type of financial operation
    pub intent: TransactionIntent,

    // === Tracing ===
    /// Request UUID from API/CLI (REQUIRED, never generated by ledger)
    pub correlation_id: String,

    /// Optional link to parent entry that caused this entry
    pub causality_id: Option<String>,

    // === Financial Data ===
    /// The list of postings (debits and credits)
    pub postings: Vec<Posting>,

    // === Metadata ===
    /// Additional metadata (opaque to ledger, used by audit/projection)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    // === Digital Signatures (Phase 2) ===
    /// Signatures from system and/or operators
    #[serde(default)]
    pub signatures: Vec<EntrySignature>,
}

impl JournalEntry {
    /// Validate the entry according to ledger invariants
    ///
    /// # Rules
    /// 1. At least 2 postings (double-entry)
    /// 2. Zero-sum per asset group
    /// 3. Non-empty correlation_id
    /// 4. Genesis entries have special requirements
    pub fn validate(&self) -> Result<(), LedgerError> {
        // Rule: correlation_id cannot be empty
        if self.correlation_id.is_empty() {
            return Err(LedgerError::EmptyCorrelationId);
        }

        // Rule: At least 2 postings
        if self.postings.len() < 2 {
            return Err(LedgerError::InsufficientPostings);
        }

        // Rule: Genesis entry special requirements
        if self.intent == TransactionIntent::Genesis {
            if self.sequence != 1 {
                return Err(LedgerError::InvalidGenesisSequence);
            }
            if self.prev_hash != "GENESIS" {
                return Err(LedgerError::InvalidGenesisPrevHash);
            }
        }

        // Rule: Zero-sum per asset
        let mut sums: HashMap<String, Decimal> = HashMap::new();
        for posting in &self.postings {
            let asset = &posting.account.asset;
            *sums.entry(asset.clone()).or_default() += posting.signed_amount();
        }

        for (asset, sum) in sums {
            if !sum.is_zero() {
                return Err(LedgerError::UnbalancedEntry {
                    asset,
                    imbalance: sum,
                });
            }
        }

        Ok(())
    }

    /// Get all unique assets in this entry
    pub fn assets(&self) -> Vec<String> {
        let mut assets: Vec<_> = self
            .postings
            .iter()
            .map(|p| p.account.asset.clone())
            .collect();
        assets.sort();
        assets.dedup();
        assets
    }
}

/// Builder for creating JournalEntry with fluent API
#[derive(Debug, Default)]
pub struct JournalEntryBuilder {
    intent: Option<TransactionIntent>,
    correlation_id: Option<String>,
    causality_id: Option<String>,
    postings: Vec<Posting>,
    metadata: HashMap<String, serde_json::Value>,
}

impl JournalEntryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intent(mut self, intent: TransactionIntent) -> Self {
        self.intent = Some(intent);
        self
    }

    pub fn correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    pub fn causality_id(mut self, id: impl Into<String>) -> Self {
        self.causality_id = Some(id.into());
        self
    }

    pub fn posting(mut self, posting: Posting) -> Self {
        self.postings.push(posting);
        self
    }

    pub fn debit(mut self, account: AccountKey, amount: Amount) -> Self {
        self.postings.push(Posting::debit(account, amount));
        self
    }

    pub fn credit(mut self, account: AccountKey, amount: Amount) -> Self {
        self.postings.push(Posting::credit(account, amount));
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Build the entry (sequence, prev_hash, hash, timestamp will be set by ledger)
    pub fn build_unsigned(self) -> Result<UnsignedEntry, LedgerError> {
        let intent = self.intent.unwrap_or(TransactionIntent::Transfer);
        let correlation_id = self
            .correlation_id
            .ok_or(LedgerError::EmptyCorrelationId)?;

        if correlation_id.is_empty() {
            return Err(LedgerError::EmptyCorrelationId);
        }

        if self.postings.len() < 2 {
            return Err(LedgerError::InsufficientPostings);
        }

        let unsigned = UnsignedEntry {
            intent,
            correlation_id,
            causality_id: self.causality_id,
            postings: self.postings,
            metadata: self.metadata,
        };

        // Validate double-entry balance before returning
        unsigned.validate_balance()?;

        Ok(unsigned)
    }
}

/// An entry that hasn't been signed with sequence/hash yet
#[derive(Debug, Clone)]
pub struct UnsignedEntry {
    pub intent: TransactionIntent,
    pub correlation_id: String,
    pub causality_id: Option<String>,
    pub postings: Vec<Posting>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl UnsignedEntry {
    /// Validate double-entry balance
    pub fn validate_balance(&self) -> Result<(), LedgerError> {
        let mut sums: HashMap<String, Decimal> = HashMap::new();
        for posting in &self.postings {
            let asset = &posting.account.asset;
            *sums.entry(asset.clone()).or_default() += posting.signed_amount();
        }

        for (asset, sum) in sums {
            if !sum.is_zero() {
                return Err(LedgerError::UnbalancedEntry {
                    asset,
                    imbalance: sum,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    #[test]
    fn test_balanced_entry() {
        let entry = JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Genesis,
            correlation_id: "test-123".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(1000)),
                Posting::credit(
                    AccountKey::new(
                        crate::AccountCategory::Equity,
                        "SYSTEM",
                        "CAPITAL",
                        "USDT",
                        "MAIN",
                    ),
                    amount(1000),
                ),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        assert!(entry.validate().is_ok());
    }

    #[test]
    fn test_unbalanced_entry() {
        let entry = JournalEntry {
            sequence: 2,
            prev_hash: "abc".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test-123".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(50)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let result = entry.validate();
        assert!(matches!(result, Err(LedgerError::UnbalancedEntry { .. })));
    }

    #[test]
    fn test_multi_asset_trade() {
        let entry = JournalEntry {
            sequence: 2,
            prev_hash: "abc".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Trade,
            correlation_id: "test-123".to_string(),
            causality_id: None,
            postings: vec![
                // USDT leg: Alice pays, Bob receives
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                // BTC leg: Bob pays, Alice receives
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        assert!(entry.validate().is_ok());
        assert_eq!(entry.assets(), vec!["BTC", "USDT"]);
    }

    #[test]
    fn test_empty_correlation_id() {
        let entry = JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Genesis,
            correlation_id: "".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(1000)),
                Posting::credit(
                    AccountKey::new(
                        crate::AccountCategory::Equity,
                        "SYSTEM",
                        "CAPITAL",
                        "USDT",
                        "MAIN",
                    ),
                    amount(1000),
                ),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        assert!(matches!(
            entry.validate(),
            Err(LedgerError::EmptyCorrelationId)
        ));
    }

    #[test]
    fn test_builder() {
        let unsigned = JournalEntryBuilder::new()
            .intent(TransactionIntent::Deposit)
            .correlation_id("req-123")
            .debit(AccountKey::system_vault("USDT"), amount(100))
            .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .build_unsigned()
            .unwrap();

        assert_eq!(unsigned.intent, TransactionIntent::Deposit);
        assert_eq!(unsigned.postings.len(), 2);
        assert!(unsigned.validate_balance().is_ok());
    }
}

```

## File ./bibank\crates\ledger\src\error.rs:
```rust
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

    // === Phase 2: Intent-specific validation errors ===

    #[error("Invalid {intent} posting on {account}: {reason}")]
    InvalidIntentPosting {
        intent: &'static str,
        account: String,
        reason: &'static str,
    },

    #[error("Trade requires {expected} postings, got {actual}: {reason}")]
    InvalidTradePostings {
        expected: usize,
        actual: usize,
        reason: &'static str,
    },

    #[error("Trade requires exactly {expected} assets, got {actual}: {assets:?}")]
    InvalidTradeAssets {
        expected: usize,
        actual: usize,
        assets: Vec<String>,
    },

    // === Phase 2: Signature errors ===

    #[error("Missing system signature")]
    MissingSystemSignature,

    #[error("Invalid signature from {signer}: {reason}")]
    InvalidSignature {
        signer: String,
        reason: String,
    },

    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),
}

```

## File ./bibank\crates\ledger\src\hash.rs:
```rust
//! Hash chain utilities for ledger integrity

use crate::entry::JournalEntry;
use sha2::{Digest, Sha256};

/// Calculate SHA256 hash of entry content (excluding the hash field itself)
pub fn calculate_entry_hash(entry: &JournalEntry) -> String {
    let mut hasher = Sha256::new();

    // Include all fields except `hash`
    hasher.update(entry.sequence.to_le_bytes());
    hasher.update(entry.prev_hash.as_bytes());
    hasher.update(entry.timestamp.to_rfc3339().as_bytes());
    hasher.update(format!("{:?}", entry.intent).as_bytes());
    hasher.update(entry.correlation_id.as_bytes());

    if let Some(ref causality_id) = entry.causality_id {
        hasher.update(causality_id.as_bytes());
    }

    // Hash postings
    for posting in &entry.postings {
        hasher.update(posting.account.to_string().as_bytes());
        hasher.update(posting.amount.value().to_string().as_bytes());
        hasher.update(format!("{:?}", posting.side).as_bytes());
    }

    // Hash metadata keys (sorted for determinism)
    let mut keys: Vec<_> = entry.metadata.keys().collect();
    keys.sort();
    for key in keys {
        hasher.update(key.as_bytes());
        if let Some(value) = entry.metadata.get(key) {
            hasher.update(value.to_string().as_bytes());
        }
    }

    hex::encode(hasher.finalize())
}

/// Verify hash chain integrity
pub fn verify_chain(entries: &[JournalEntry]) -> Result<(), ChainError> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut prev_hash = "GENESIS".to_string();

    for (i, entry) in entries.iter().enumerate() {
        // Verify prev_hash links correctly
        if entry.prev_hash != prev_hash {
            return Err(ChainError::BrokenLink {
                sequence: entry.sequence,
                expected: prev_hash,
                actual: entry.prev_hash.clone(),
            });
        }

        // Verify hash is correct
        let calculated = calculate_entry_hash(entry);
        if entry.hash != calculated {
            return Err(ChainError::InvalidHash {
                sequence: entry.sequence,
                expected: calculated,
                actual: entry.hash.clone(),
            });
        }

        // Verify sequence is strictly increasing
        if i > 0 && entry.sequence != entries[i - 1].sequence + 1 {
            return Err(ChainError::InvalidSequence {
                expected: entries[i - 1].sequence + 1,
                actual: entry.sequence,
            });
        }

        prev_hash = entry.hash.clone();
    }

    Ok(())
}

/// Errors in hash chain verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    BrokenLink {
        sequence: u64,
        expected: String,
        actual: String,
    },
    InvalidHash {
        sequence: u64,
        expected: String,
        actual: String,
    },
    InvalidSequence {
        expected: u64,
        actual: u64,
    },
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainError::BrokenLink {
                sequence,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Broken link at seq {}: expected prev_hash '{}', got '{}'",
                    sequence, expected, actual
                )
            }
            ChainError::InvalidHash {
                sequence,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Invalid hash at seq {}: expected '{}', got '{}'",
                    sequence, expected, actual
                )
            }
            ChainError::InvalidSequence { expected, actual } => {
                write!(
                    f,
                    "Invalid sequence: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for ChainError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountKey, Posting, TransactionIntent};
    use bibank_core::Amount;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    fn create_entry(sequence: u64, prev_hash: &str) -> JournalEntry {
        let amount = Amount::new(Decimal::new(100, 0)).unwrap();
        let mut entry = JournalEntry {
            sequence,
            prev_hash: prev_hash.to_string(),
            hash: String::new(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: format!("test-{}", sequence),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };
        entry.hash = calculate_entry_hash(&entry);
        entry
    }

    #[test]
    fn test_hash_deterministic() {
        let entry = create_entry(1, "GENESIS");
        let hash1 = calculate_entry_hash(&entry);
        let hash2 = calculate_entry_hash(&entry);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_verify_valid_chain() {
        let entry1 = create_entry(1, "GENESIS");
        let entry2 = create_entry(2, &entry1.hash);
        let entry3 = create_entry(3, &entry2.hash);

        let entries = vec![entry1, entry2, entry3];
        assert!(verify_chain(&entries).is_ok());
    }

    #[test]
    fn test_verify_broken_chain() {
        let entry1 = create_entry(1, "GENESIS");
        let entry2 = create_entry(2, "wrong_hash");

        let entries = vec![entry1, entry2];
        let result = verify_chain(&entries);
        assert!(matches!(result, Err(ChainError::BrokenLink { .. })));
    }
}

```

## File ./bibank\crates\ledger\src\lib.rs:
```rust
//! BiBank Ledger - Double-entry accounting core
//!
//! This is the HEART of BiBank. All financial state changes go through this crate.
//!
//! # Key Types
//! - `AccountKey`: Hierarchical account identifier (CAT:SEGMENT:ID:ASSET:SUB_ACCOUNT)
//! - `JournalEntry`: Atomic unit of financial state change
//! - `Posting`: Single debit/credit in an entry
//! - `Side`: Debit or Credit

pub mod account;
pub mod entry;
pub mod error;
pub mod hash;
pub mod signature;
pub mod validation;

pub use account::{AccountCategory, AccountKey};
pub use entry::{JournalEntry, JournalEntryBuilder, Posting, Side, TransactionIntent, UnsignedEntry};
pub use error::LedgerError;
pub use signature::{EntrySignature, SignatureAlgorithm, SignablePayload, Signer, SystemSigner};
pub use validation::validate_intent;

```

## File ./bibank\crates\ledger\src\signature.rs:
```rust
//! Digital signatures for journal entries
//!
//! Each entry is signed by the system key, and optionally by operator keys
//! for Adjustment entries requiring human approval.

use crate::entry::{JournalEntry, Posting, TransactionIntent};
use crate::error::LedgerError;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer as DalekSigner, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Signature algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureAlgorithm {
    /// Ed25519 (default)
    Ed25519,
    /// Secp256k1 (for future blockchain compatibility)
    Secp256k1,
}

impl Default for SignatureAlgorithm {
    fn default() -> Self {
        Self::Ed25519
    }
}

/// Digital signature attached to a journal entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySignature {
    /// Signer identifier ("SYSTEM" or operator ID)
    pub signer_id: String,

    /// Signature algorithm used
    pub algorithm: SignatureAlgorithm,

    /// Public key (hex-encoded)
    pub public_key: String,

    /// Signature bytes (hex-encoded)
    pub signature: String,

    /// Timestamp when signature was created
    pub signed_at: DateTime<Utc>,
}

impl EntrySignature {
    /// Verify this signature against a payload
    pub fn verify(&self, payload: &[u8]) -> Result<(), LedgerError> {
        match self.algorithm {
            SignatureAlgorithm::Ed25519 => {
                let pk_bytes = hex::decode(&self.public_key).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid public key hex: {}", e),
                    }
                })?;

                let sig_bytes = hex::decode(&self.signature).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid signature hex: {}", e),
                    }
                })?;

                let pk_array: [u8; 32] = pk_bytes.try_into().map_err(|_| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: "Public key must be 32 bytes".to_string(),
                    }
                })?;

                let sig_array: [u8; 64] = sig_bytes.try_into().map_err(|_| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: "Signature must be 64 bytes".to_string(),
                    }
                })?;

                let verifying_key = VerifyingKey::from_bytes(&pk_array).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid public key: {}", e),
                    }
                })?;

                let signature = Signature::from_bytes(&sig_array);

                verifying_key.verify(payload, &signature).map_err(|e| {
                    LedgerError::SignatureVerificationFailed(format!(
                        "Signature from {} failed: {}",
                        self.signer_id, e
                    ))
                })?;

                Ok(())
            }
            SignatureAlgorithm::Secp256k1 => {
                // Future: implement secp256k1 verification
                Err(LedgerError::SignatureVerificationFailed(
                    "Secp256k1 not yet implemented".to_string(),
                ))
            }
        }
    }
}

/// Signable payload - the 8 fields that are signed
///
/// This is a deterministic representation of the entry for signing.
/// Order matters for consistent hashing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePayload {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub intent: TransactionIntent,
    pub postings: Vec<Posting>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub prev_hash: String,
    pub hash: String,
    pub signed_at: DateTime<Utc>,
}

impl SignablePayload {
    /// Create from a journal entry and signing timestamp
    pub fn from_entry(entry: &JournalEntry, signed_at: DateTime<Utc>) -> Self {
        Self {
            sequence: entry.sequence,
            timestamp: entry.timestamp,
            intent: entry.intent,
            postings: entry.postings.clone(),
            metadata: entry.metadata.clone(),
            prev_hash: entry.prev_hash.clone(),
            hash: entry.hash.clone(),
            signed_at,
        }
    }

    /// Serialize to canonical JSON bytes for signing
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("SignablePayload serialization should never fail")
    }
}

/// Trait for signers
pub trait Signer: Send + Sync {
    /// Get the signer ID
    fn signer_id(&self) -> &str;

    /// Get the public key (hex-encoded)
    fn public_key_hex(&self) -> String;

    /// Sign a payload and return the signature
    fn sign(&self, entry: &JournalEntry) -> EntrySignature;
}

/// System signer using Ed25519
pub struct SystemSigner {
    signing_key: SigningKey,
}

impl SystemSigner {
    /// Create from a 32-byte seed (hex-encoded in env var)
    pub fn from_hex(hex_seed: &str) -> Result<Self, LedgerError> {
        let bytes = hex::decode(hex_seed).map_err(|e| {
            LedgerError::InvalidSignature {
                signer: "SYSTEM".to_string(),
                reason: format!("Invalid key hex: {}", e),
            }
        })?;

        let seed: [u8; 32] = bytes.try_into().map_err(|_| {
            LedgerError::InvalidSignature {
                signer: "SYSTEM".to_string(),
                reason: "Key must be 32 bytes".to_string(),
            }
        })?;

        Ok(Self {
            signing_key: SigningKey::from_bytes(&seed),
        })
    }

    /// Generate a new random signing key
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            signing_key: SigningKey::generate(&mut rng),
        }
    }

    /// Export the seed as hex (for storage)
    pub fn seed_hex(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }
}

impl Signer for SystemSigner {
    fn signer_id(&self) -> &str {
        "SYSTEM"
    }

    fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    fn sign(&self, entry: &JournalEntry) -> EntrySignature {
        let signed_at = Utc::now();
        let payload = SignablePayload::from_entry(entry, signed_at);
        let payload_bytes = payload.to_bytes();

        let signature = self.signing_key.sign(&payload_bytes);

        EntrySignature {
            signer_id: self.signer_id().to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: self.public_key_hex(),
            signature: hex::encode(signature.to_bytes()),
            signed_at,
        }
    }
}

impl JournalEntry {
    /// Verify all signatures on this entry
    pub fn verify_signatures(&self) -> Result<(), LedgerError> {
        if self.signatures.is_empty() {
            // Phase 1 entries have no signatures - that's OK
            return Ok(());
        }

        for sig in &self.signatures {
            let payload = SignablePayload::from_entry(self, sig.signed_at);
            sig.verify(&payload.to_bytes())?;
        }

        Ok(())
    }

    /// Check if entry has a system signature
    pub fn has_system_signature(&self) -> bool {
        self.signatures.iter().any(|s| s.signer_id == "SYSTEM")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountKey;
    use bibank_core::Amount;
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    fn make_test_entry() -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "abc123".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_system_signer_sign_and_verify() {
        let signer = SystemSigner::generate();
        let entry = make_test_entry();

        let signature = signer.sign(&entry);

        assert_eq!(signature.signer_id, "SYSTEM");
        assert_eq!(signature.algorithm, SignatureAlgorithm::Ed25519);

        // Verify the signature
        let payload = SignablePayload::from_entry(&entry, signature.signed_at);
        assert!(signature.verify(&payload.to_bytes()).is_ok());
    }

    #[test]
    fn test_signature_roundtrip() {
        let signer = SystemSigner::generate();
        let seed = signer.seed_hex();

        // Recreate signer from seed
        let signer2 = SystemSigner::from_hex(&seed).unwrap();
        assert_eq!(signer.public_key_hex(), signer2.public_key_hex());
    }

    #[test]
    fn test_entry_with_signature() {
        let signer = SystemSigner::generate();
        let mut entry = make_test_entry();

        let signature = signer.sign(&entry);
        entry.signatures.push(signature);

        assert!(entry.verify_signatures().is_ok());
        assert!(entry.has_system_signature());
    }

    #[test]
    fn test_tampered_entry_fails_verification() {
        let signer = SystemSigner::generate();
        let mut entry = make_test_entry();

        let signature = signer.sign(&entry);
        entry.signatures.push(signature);

        // Tamper with the entry
        entry.sequence = 999;

        // Verification should fail
        assert!(entry.verify_signatures().is_err());
    }
}

```

## File ./bibank\crates\ledger\src\validation.rs:
```rust
//! Intent-specific validation rules
//!
//! Each TransactionIntent has specific validation rules beyond basic double-entry.

use crate::account::AccountCategory;
use crate::entry::{Posting, Side, TransactionIntent, UnsignedEntry};
use crate::error::LedgerError;

/// Validation result with detailed error
pub type ValidationResult = Result<(), LedgerError>;

/// Validate an unsigned entry according to its intent
pub fn validate_intent(entry: &UnsignedEntry) -> ValidationResult {
    match entry.intent {
        TransactionIntent::Genesis => validate_genesis(entry),
        TransactionIntent::Deposit => validate_deposit(entry),
        TransactionIntent::Withdrawal => validate_withdrawal(entry),
        TransactionIntent::Transfer => validate_transfer(entry),
        TransactionIntent::Trade => validate_trade(entry),
        TransactionIntent::Fee => validate_fee(entry),
        TransactionIntent::Adjustment => validate_adjustment(entry),
        // Phase 3: Margin Trading
        TransactionIntent::Borrow => validate_borrow(entry),
        TransactionIntent::Repay => validate_repay(entry),
        TransactionIntent::Interest => validate_interest(entry),
        TransactionIntent::Liquidation => validate_liquidation(entry),
        TransactionIntent::OrderPlace => validate_order_place(entry),
        TransactionIntent::OrderCancel => validate_order_cancel(entry),
    }
}

/// Genesis: ASSET ↑, EQUITY ↑
fn validate_genesis(entry: &UnsignedEntry) -> ValidationResult {
    for posting in &entry.postings {
        match posting.account.category {
            AccountCategory::Asset | AccountCategory::Equity => {}
            _ => {
                return Err(LedgerError::InvalidIntentPosting {
                    intent: "Genesis",
                    account: posting.account.to_string(),
                    reason: "Genesis only allows ASSET and EQUITY accounts",
                });
            }
        }
    }
    Ok(())
}

/// Deposit: ASSET ↑, LIAB ↑
fn validate_deposit(entry: &UnsignedEntry) -> ValidationResult {
    let has_asset_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset && p.side == Side::Debit
    });
    let has_liab_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Credit
    });

    if !has_asset_debit || !has_liab_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Deposit",
            account: String::new(),
            reason: "Deposit requires ASSET debit and LIAB credit",
        });
    }
    Ok(())
}

/// Withdrawal: ASSET ↓, LIAB ↓
fn validate_withdrawal(entry: &UnsignedEntry) -> ValidationResult {
    let has_asset_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset && p.side == Side::Credit
    });
    let has_liab_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Debit
    });

    if !has_asset_credit || !has_liab_debit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Withdrawal",
            account: String::new(),
            reason: "Withdrawal requires ASSET credit and LIAB debit",
        });
    }
    Ok(())
}

/// Transfer: LIAB only
fn validate_transfer(entry: &UnsignedEntry) -> ValidationResult {
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Transfer",
                account: posting.account.to_string(),
                reason: "Transfer only allows LIAB accounts",
            });
        }
    }
    Ok(())
}

/// Trade: LIAB only, exactly 2 assets, min 4 postings, zero-sum per asset
pub fn validate_trade(entry: &UnsignedEntry) -> ValidationResult {
    // Rule 1: Min 4 postings (2 legs × 2 sides)
    if entry.postings.len() < 4 {
        return Err(LedgerError::InvalidTradePostings {
            expected: 4,
            actual: entry.postings.len(),
            reason: "Trade requires at least 4 postings (2 assets × 2 sides)",
        });
    }

    // Rule 2: LIAB accounts only
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Trade",
                account: posting.account.to_string(),
                reason: "Trade only allows LIAB accounts",
            });
        }
    }

    // Rule 3: Exactly 2 assets
    let assets = collect_assets(&entry.postings);
    if assets.len() != 2 {
        return Err(LedgerError::InvalidTradeAssets {
            expected: 2,
            actual: assets.len(),
            assets: assets.into_iter().collect(),
        });
    }

    // Rule 4: Zero-sum per asset (already checked in validate_balance)
    // Rule 5: At least 2 users (implicit from 4 postings with different accounts)

    Ok(())
}

/// Fee: LIAB ↓, REV ↑
pub fn validate_fee(entry: &UnsignedEntry) -> ValidationResult {
    let has_liab_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Debit
    });
    let has_rev_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Revenue && p.side == Side::Credit
    });

    if !has_liab_debit || !has_rev_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Fee",
            account: String::new(),
            reason: "Fee requires LIAB debit and REV credit",
        });
    }

    // All postings must be either LIAB debit or REV credit
    for posting in &entry.postings {
        let valid = match (&posting.account.category, &posting.side) {
            (AccountCategory::Liability, Side::Debit) => true,
            (AccountCategory::Revenue, Side::Credit) => true,
            _ => false,
        };
        if !valid {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Fee",
                account: posting.account.to_string(),
                reason: "Fee only allows LIAB debit or REV credit",
            });
        }
    }

    Ok(())
}

/// Adjustment: Any accounts (audit-heavy)
fn validate_adjustment(_entry: &UnsignedEntry) -> ValidationResult {
    // Adjustment allows any accounts but requires approval flag
    // Approval is checked at RPC layer
    Ok(())
}

// === Phase 3: Margin Trading Validation ===

/// Borrow: ASSET:LOAN ↑ (debit), LIAB:AVAILABLE ↑ (credit)
/// User borrows funds - BiBank's receivable increases, User's balance increases
fn validate_borrow(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Borrow",
            account: String::new(),
            reason: "Borrow requires at least 2 postings",
        });
    }

    // Must have ASSET:*:*:LOAN debit (BiBank's receivable increases)
    let has_loan_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset
            && p.account.sub_account == "LOAN"
            && p.side == Side::Debit
    });

    // Must have LIAB:*:*:AVAILABLE credit (User's balance increases)
    let has_avail_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "AVAILABLE"
            && p.side == Side::Credit
    });

    if !has_loan_debit || !has_avail_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Borrow",
            account: String::new(),
            reason: "Borrow requires ASSET:*:LOAN debit and LIAB:*:AVAILABLE credit",
        });
    }

    // All postings must be ASSET:LOAN or LIAB:AVAILABLE
    for posting in &entry.postings {
        let valid = match (&posting.account.category, posting.account.sub_account.as_str()) {
            (AccountCategory::Asset, "LOAN") => true,
            (AccountCategory::Liability, "AVAILABLE") => true,
            _ => false,
        };
        if !valid {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Borrow",
                account: posting.account.to_string(),
                reason: "Borrow only allows ASSET:LOAN or LIAB:AVAILABLE accounts",
            });
        }
    }

    Ok(())
}

/// Repay: LIAB:AVAILABLE ↓ (debit), ASSET:LOAN ↓ (credit)
/// User repays loan - User's balance decreases, BiBank's receivable decreases
fn validate_repay(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Repay",
            account: String::new(),
            reason: "Repay requires at least 2 postings",
        });
    }

    // Must have LIAB:*:*:AVAILABLE debit (User pays)
    let has_avail_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "AVAILABLE"
            && p.side == Side::Debit
    });

    // Must have ASSET:*:*:LOAN credit (Loan decreases)
    let has_loan_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset
            && p.account.sub_account == "LOAN"
            && p.side == Side::Credit
    });

    if !has_avail_debit || !has_loan_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Repay",
            account: String::new(),
            reason: "Repay requires LIAB:*:AVAILABLE debit and ASSET:*:LOAN credit",
        });
    }

    Ok(())
}

/// Interest: ASSET:LOAN ↑ (debit), REV:INTEREST ↑ (credit)
/// Interest accrual - Loan increases, Revenue increases (compound interest)
fn validate_interest(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Interest",
            account: String::new(),
            reason: "Interest requires at least 2 postings",
        });
    }

    // Must have ASSET:*:*:LOAN debit (Loan principal increases)
    let has_loan_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset
            && p.account.sub_account == "LOAN"
            && p.side == Side::Debit
    });

    // Must have REV:*:INTEREST:*:* credit (Revenue increases)
    let has_rev_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Revenue && p.side == Side::Credit
    });

    if !has_loan_debit || !has_rev_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Interest",
            account: String::new(),
            reason: "Interest requires ASSET:LOAN debit and REV credit",
        });
    }

    Ok(())
}

/// Liquidation: Multiple accounts - close position, settle loan
/// Complex validation - at least 4 postings (position close + settlement)
fn validate_liquidation(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 4 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Liquidation",
            account: String::new(),
            reason: "Liquidation requires at least 4 postings",
        });
    }

    // Liquidation can involve LIAB (user balance), ASSET (loan), EQUITY (insurance)
    // We don't restrict categories heavily for liquidation - it's a complex operation
    Ok(())
}

/// OrderPlace: LIAB:AVAILABLE ↓, LIAB:LOCKED ↑
/// Lock funds for order
fn validate_order_place(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "OrderPlace",
            account: String::new(),
            reason: "OrderPlace requires at least 2 postings",
        });
    }

    // Must have LIAB:*:*:AVAILABLE debit (funds leave available)
    let has_avail_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "AVAILABLE"
            && p.side == Side::Debit
    });

    // Must have LIAB:*:*:LOCKED credit (funds go to locked)
    let has_locked_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "LOCKED"
            && p.side == Side::Credit
    });

    if !has_avail_debit || !has_locked_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "OrderPlace",
            account: String::new(),
            reason: "OrderPlace requires LIAB:AVAILABLE debit and LIAB:LOCKED credit",
        });
    }

    // All postings must be LIAB
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "OrderPlace",
                account: posting.account.to_string(),
                reason: "OrderPlace only allows LIAB accounts",
            });
        }
    }

    Ok(())
}

/// OrderCancel: LIAB:LOCKED ↓, LIAB:AVAILABLE ↑
/// Unlock funds from cancelled order
fn validate_order_cancel(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "OrderCancel",
            account: String::new(),
            reason: "OrderCancel requires at least 2 postings",
        });
    }

    // Must have LIAB:*:*:LOCKED debit (funds leave locked)
    let has_locked_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "LOCKED"
            && p.side == Side::Debit
    });

    // Must have LIAB:*:*:AVAILABLE credit (funds return to available)
    let has_avail_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "AVAILABLE"
            && p.side == Side::Credit
    });

    if !has_locked_debit || !has_avail_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "OrderCancel",
            account: String::new(),
            reason: "OrderCancel requires LIAB:LOCKED debit and LIAB:AVAILABLE credit",
        });
    }

    // All postings must be LIAB
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "OrderCancel",
                account: posting.account.to_string(),
                reason: "OrderCancel only allows LIAB accounts",
            });
        }
    }

    Ok(())
}

/// Collect unique assets from postings
fn collect_assets(postings: &[Posting]) -> std::collections::HashSet<String> {
    postings.iter().map(|p| p.account.asset.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountKey;
    use bibank_core::Amount;
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    #[test]
    fn test_validate_trade_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // USDT leg
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                // BTC leg
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_trade(&entry).is_ok());
    }

    #[test]
    fn test_validate_trade_insufficient_postings() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidTradePostings { .. })));
    }

    #[test]
    fn test_validate_trade_wrong_asset_count() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Only USDT, no second asset
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(50)),
                Posting::debit(AccountKey::user_available("CHARLIE", "USDT"), amount(50)),
                Posting::credit(AccountKey::user_available("DAVE", "USDT"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidTradeAssets { .. })));
    }

    #[test]
    fn test_validate_trade_non_liab_account() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // ASSET account not allowed in Trade
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { .. })));
    }

    #[test]
    fn test_validate_fee_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Fee,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1)),
                Posting::credit(AccountKey::fee_revenue("USDT"), amount(1)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_fee(&entry).is_ok());
    }

    #[test]
    fn test_validate_fee_wrong_direction() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Fee,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Wrong: LIAB credit instead of debit
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1)),
                Posting::debit(AccountKey::fee_revenue("USDT"), amount(1)),
            ],
            metadata: Default::default(),
        };

        let result = validate_fee(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { .. })));
    }

    // === Phase 3: Borrow/Repay Tests ===

    fn loan_account(user: &str, asset: &str) -> AccountKey {
        AccountKey::new(AccountCategory::Asset, "USER", user, asset, "LOAN")
    }

    #[test]
    fn test_validate_borrow_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Borrow,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // BiBank's receivable increases (ASSET:LOAN debit)
                Posting::debit(loan_account("ALICE", "USDT"), amount(1000)),
                // User's balance increases (LIAB:AVAILABLE credit)
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_borrow(&entry).is_ok());
    }

    #[test]
    fn test_validate_borrow_missing_loan() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Borrow,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Missing ASSET:LOAN account
                Posting::debit(AccountKey::user_available("BOB", "USDT"), amount(1000)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        let result = validate_borrow(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Borrow", .. })));
    }

    #[test]
    fn test_validate_borrow_wrong_account_type() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Borrow,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(loan_account("ALICE", "USDT"), amount(1000)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
                // Not allowed: EQUITY account in Borrow
                Posting::debit(AccountKey::new(AccountCategory::Equity, "SYSTEM", "INSURANCE", "USDT", "FUND"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_borrow(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Borrow", .. })));
    }

    #[test]
    fn test_validate_repay_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Repay,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // User's balance decreases (LIAB:AVAILABLE debit)
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(500)),
                // BiBank's receivable decreases (ASSET:LOAN credit)
                Posting::credit(loan_account("ALICE", "USDT"), amount(500)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_repay(&entry).is_ok());
    }

    #[test]
    fn test_validate_repay_missing_loan_credit() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Repay,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Has LIAB debit but missing ASSET:LOAN credit
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(500)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(500)),
            ],
            metadata: Default::default(),
        };

        let result = validate_repay(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Repay", .. })));
    }

    // === Phase 3: Interest Tests ===

    fn interest_revenue(asset: &str) -> AccountKey {
        AccountKey::new(AccountCategory::Revenue, "SYSTEM", "INTEREST", asset, "INCOME")
    }

    #[test]
    fn test_validate_interest_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Interest,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Loan increases (compound interest)
                Posting::debit(loan_account("ALICE", "USDT"), amount(5)),
                // Revenue increases
                Posting::credit(interest_revenue("USDT"), amount(5)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_interest(&entry).is_ok());
    }

    #[test]
    fn test_validate_interest_missing_revenue() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Interest,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(loan_account("ALICE", "USDT"), amount(5)),
                // Wrong: No REV credit
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(5)),
            ],
            metadata: Default::default(),
        };

        let result = validate_interest(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Interest", .. })));
    }

    // === Phase 3: Liquidation Tests ===

    #[test]
    fn test_validate_liquidation_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Liquidation,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Close user's position - user loses BTC
                Posting::debit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
                Posting::credit(AccountKey::system_vault("BTC"), amount(1)),
                // Settle loan - loan cleared
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(500)),
                Posting::credit(loan_account("ALICE", "USDT"), amount(500)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_liquidation(&entry).is_ok());
    }

    #[test]
    fn test_validate_liquidation_insufficient_postings() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Liquidation,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Only 2 postings - not enough for liquidation
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(500)),
                Posting::credit(loan_account("ALICE", "USDT"), amount(500)),
            ],
            metadata: Default::default(),
        };

        let result = validate_liquidation(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Liquidation", .. })));
    }

    // === Phase 3: OrderPlace/OrderCancel Tests ===

    fn locked_account(user: &str, asset: &str) -> AccountKey {
        AccountKey::user_locked(user, asset)
    }

    #[test]
    fn test_validate_order_place_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderPlace,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Funds leave AVAILABLE
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
                // Funds go to LOCKED
                Posting::credit(locked_account("ALICE", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_order_place(&entry).is_ok());
    }

    #[test]
    fn test_validate_order_place_missing_locked() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderPlace,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Has AVAILABLE debit but no LOCKED credit
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        let result = validate_order_place(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "OrderPlace", .. })));
    }

    #[test]
    fn test_validate_order_place_non_liab_account() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderPlace,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
                Posting::credit(locked_account("ALICE", "USDT"), amount(1000)),
                // Not allowed: ASSET account in OrderPlace
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_order_place(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "OrderPlace", .. })));
    }

    #[test]
    fn test_validate_order_cancel_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderCancel,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Funds leave LOCKED
                Posting::debit(locked_account("ALICE", "USDT"), amount(1000)),
                // Funds return to AVAILABLE
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_order_cancel(&entry).is_ok());
    }

    #[test]
    fn test_validate_order_cancel_missing_available_credit() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderCancel,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Has LOCKED debit but wrong credit account
                Posting::debit(locked_account("ALICE", "USDT"), amount(1000)),
                Posting::credit(locked_account("BOB", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        let result = validate_order_cancel(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "OrderCancel", .. })));
    }
}

```

## File ./bibank\crates\matching\src\engine.rs:
```rust
//! Matching engine for multiple trading pairs

use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::error::MatchingError;
use crate::fill::MatchResult;
use crate::order::{Order, OrderId, OrderSide, TradingPair};
use crate::orderbook::OrderBook;

/// Central matching engine managing multiple order books
#[derive(Debug)]
pub struct MatchingEngine {
    /// Order books indexed by trading pair symbol (e.g., "BTC/USDT")
    books: HashMap<String, OrderBook>,
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchingEngine {
    /// Create a new matching engine
    pub fn new() -> Self {
        Self {
            books: HashMap::new(),
        }
    }

    /// Create a matching engine with pre-configured trading pairs
    pub fn with_pairs(pairs: Vec<TradingPair>) -> Self {
        let mut engine = Self::new();
        for pair in pairs {
            engine.add_pair(pair);
        }
        engine
    }

    /// Add a new trading pair
    pub fn add_pair(&mut self, pair: TradingPair) {
        let key = pair.to_string();
        self.books.entry(key).or_insert_with(|| OrderBook::new(pair));
    }

    /// Check if a trading pair exists
    pub fn has_pair(&self, pair: &TradingPair) -> bool {
        self.books.contains_key(&pair.to_string())
    }

    /// Get a reference to an order book
    pub fn get_book(&self, pair: &TradingPair) -> Option<&OrderBook> {
        self.books.get(&pair.to_string())
    }

    /// Get a mutable reference to an order book
    fn get_book_mut(&mut self, pair: &TradingPair) -> Option<&mut OrderBook> {
        self.books.get_mut(&pair.to_string())
    }

    /// Place a new order
    ///
    /// The order will be matched against the opposite side of the book.
    /// Any remaining quantity will be added to the book as a resting order.
    pub fn place_order(&mut self, order: Order) -> Result<MatchResult, MatchingError> {
        let pair = order.pair.clone();

        let book = self
            .get_book_mut(&pair)
            .ok_or_else(|| MatchingError::PairNotFound(pair.to_string()))?;

        book.match_order(order)
    }

    /// Place a new limit order with parameters
    pub fn place_limit_order(
        &mut self,
        user_id: impl Into<String>,
        pair: TradingPair,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
    ) -> Result<(Order, MatchResult), MatchingError> {
        let order = Order::new(user_id, pair, side, price, quantity);
        let order_clone = order.clone();
        let result = self.place_order(order)?;
        Ok((order_clone, result))
    }

    /// Cancel an order
    pub fn cancel_order(&mut self, pair: &TradingPair, order_id: &str) -> Result<Order, MatchingError> {
        let book = self
            .get_book_mut(pair)
            .ok_or_else(|| MatchingError::PairNotFound(pair.to_string()))?;

        book.cancel_order(order_id)
    }

    /// Get an order by ID
    pub fn get_order(&self, pair: &TradingPair, order_id: &str) -> Option<&Order> {
        self.get_book(pair)?.get_order(order_id)
    }

    /// Get the best bid price for a pair
    pub fn best_bid(&self, pair: &TradingPair) -> Option<Decimal> {
        self.get_book(pair)?.best_bid()
    }

    /// Get the best ask price for a pair
    pub fn best_ask(&self, pair: &TradingPair) -> Option<Decimal> {
        self.get_book(pair)?.best_ask()
    }

    /// Get the spread for a pair
    pub fn spread(&self, pair: &TradingPair) -> Option<Decimal> {
        self.get_book(pair)?.spread()
    }

    /// Get the mid price for a pair
    pub fn mid_price(&self, pair: &TradingPair) -> Option<Decimal> {
        self.get_book(pair)?.mid_price()
    }

    /// Get order book depth
    pub fn get_depth(
        &self,
        pair: &TradingPair,
        levels: usize,
    ) -> Option<OrderBookDepth> {
        let book = self.get_book(pair)?;
        Some(OrderBookDepth {
            pair: pair.clone(),
            bids: book.get_bids(levels),
            asks: book.get_asks(levels),
        })
    }

    /// Get all trading pairs
    pub fn pairs(&self) -> Vec<TradingPair> {
        self.books.values().map(|b| b.pair().clone()).collect()
    }

    /// Total order count across all books
    pub fn total_order_count(&self) -> usize {
        self.books.values().map(|b| b.order_count()).sum()
    }
}

/// Order book depth snapshot
#[derive(Debug, Clone)]
pub struct OrderBookDepth {
    pub pair: TradingPair,
    /// Bids: (price, quantity) sorted by price descending
    pub bids: Vec<(Decimal, Decimal)>,
    /// Asks: (price, quantity) sorted by price ascending
    pub asks: Vec<(Decimal, Decimal)>,
}

impl OrderBookDepth {
    /// Get the best bid price
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.first().map(|(p, _)| *p)
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.first().map(|(p, _)| *p)
    }

    /// Get the spread
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }
}

/// Builder for placing orders with validation
pub struct OrderBuilder {
    user_id: Option<String>,
    pair: Option<TradingPair>,
    side: Option<OrderSide>,
    price: Option<Decimal>,
    quantity: Option<Decimal>,
}

impl OrderBuilder {
    pub fn new() -> Self {
        Self {
            user_id: None,
            pair: None,
            side: None,
            price: None,
            quantity: None,
        }
    }

    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn pair(mut self, pair: TradingPair) -> Self {
        self.pair = Some(pair);
        self
    }

    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    pub fn buy(self) -> Self {
        self.side(OrderSide::Buy)
    }

    pub fn sell(self) -> Self {
        self.side(OrderSide::Sell)
    }

    pub fn price(mut self, price: Decimal) -> Self {
        self.price = Some(price);
        self
    }

    pub fn quantity(mut self, quantity: Decimal) -> Self {
        self.quantity = Some(quantity);
        self
    }

    pub fn build(self) -> Result<Order, MatchingError> {
        let user_id = self.user_id.ok_or_else(|| {
            MatchingError::InvalidQuantity(Decimal::ZERO) // TODO: Better error
        })?;
        let pair = self.pair.ok_or_else(|| {
            MatchingError::PairNotFound("unspecified".to_string())
        })?;
        let side = self.side.ok_or_else(|| {
            MatchingError::InvalidQuantity(Decimal::ZERO) // TODO: Better error
        })?;
        let price = self.price.ok_or_else(|| {
            MatchingError::InvalidPrice(Decimal::ZERO)
        })?;
        let quantity = self.quantity.ok_or_else(|| {
            MatchingError::InvalidQuantity(Decimal::ZERO)
        })?;

        if price <= Decimal::ZERO {
            return Err(MatchingError::InvalidPrice(price));
        }
        if quantity <= Decimal::ZERO {
            return Err(MatchingError::InvalidQuantity(quantity));
        }

        Ok(Order::new(user_id, pair, side, price, quantity))
    }
}

impl Default for OrderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_engine() -> MatchingEngine {
        MatchingEngine::with_pairs(vec![
            TradingPair::btc_usdt(),
            TradingPair::eth_usdt(),
        ])
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_engine();
        assert!(engine.has_pair(&TradingPair::btc_usdt()));
        assert!(engine.has_pair(&TradingPair::eth_usdt()));
        assert!(!engine.has_pair(&TradingPair::new("SOL", "USDT")));
    }

    #[test]
    fn test_place_order() {
        let mut engine = create_engine();

        let order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );

        let result = engine.place_order(order).unwrap();
        assert!(result.fills.is_empty());
        assert_eq!(engine.best_bid(&TradingPair::btc_usdt()), Some(dec!(50000)));
    }

    #[test]
    fn test_place_limit_order() {
        let mut engine = create_engine();

        let (order, result) = engine
            .place_limit_order("ALICE", TradingPair::btc_usdt(), OrderSide::Sell, dec!(51000), dec!(2))
            .unwrap();

        assert!(result.fills.is_empty());
        assert_eq!(order.quantity, dec!(2));
        assert_eq!(engine.best_ask(&TradingPair::btc_usdt()), Some(dec!(51000)));
    }

    #[test]
    fn test_matching() {
        let mut engine = create_engine();

        // Place sell order
        engine
            .place_limit_order("BOB", TradingPair::btc_usdt(), OrderSide::Sell, dec!(50000), dec!(1))
            .unwrap();

        // Place matching buy order
        let (_, result) = engine
            .place_limit_order("ALICE", TradingPair::btc_usdt(), OrderSide::Buy, dec!(50000), dec!(1))
            .unwrap();

        assert_eq!(result.fills.len(), 1);
        assert!(result.fully_filled);
        assert_eq!(result.fills[0].quantity, dec!(1));
        assert_eq!(engine.total_order_count(), 0);
    }

    #[test]
    fn test_cancel_order() {
        let mut engine = create_engine();

        let order = Order::with_id(
            "order-1",
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        engine.place_order(order).unwrap();

        let cancelled = engine.cancel_order(&TradingPair::btc_usdt(), "order-1").unwrap();
        assert_eq!(cancelled.id, "order-1");
        assert_eq!(engine.total_order_count(), 0);
    }

    #[test]
    fn test_pair_not_found() {
        let mut engine = create_engine();

        let order = Order::new(
            "ALICE",
            TradingPair::new("SOL", "USDT"),
            OrderSide::Buy,
            dec!(100),
            dec!(10),
        );

        let result = engine.place_order(order);
        assert!(matches!(result, Err(MatchingError::PairNotFound(_))));
    }

    #[test]
    fn test_order_book_depth() {
        let mut engine = create_engine();

        // Add some orders
        engine
            .place_limit_order("ALICE", TradingPair::btc_usdt(), OrderSide::Buy, dec!(50000), dec!(1))
            .unwrap();
        engine
            .place_limit_order("BOB", TradingPair::btc_usdt(), OrderSide::Buy, dec!(49900), dec!(2))
            .unwrap();
        engine
            .place_limit_order("CAROL", TradingPair::btc_usdt(), OrderSide::Sell, dec!(50100), dec!(1))
            .unwrap();

        let depth = engine.get_depth(&TradingPair::btc_usdt(), 10).unwrap();

        assert_eq!(depth.bids.len(), 2);
        assert_eq!(depth.asks.len(), 1);
        assert_eq!(depth.best_bid(), Some(dec!(50000)));
        assert_eq!(depth.best_ask(), Some(dec!(50100)));
        assert_eq!(depth.spread(), Some(dec!(100)));
    }

    #[test]
    fn test_order_builder() {
        let order = OrderBuilder::new()
            .user_id("ALICE")
            .pair(TradingPair::btc_usdt())
            .buy()
            .price(dec!(50000))
            .quantity(dec!(1))
            .build()
            .unwrap();

        assert_eq!(order.user_id, "ALICE");
        assert_eq!(order.side, OrderSide::Buy);
    }

    #[test]
    fn test_order_builder_invalid() {
        let result = OrderBuilder::new()
            .user_id("ALICE")
            .pair(TradingPair::btc_usdt())
            .buy()
            .price(dec!(-100)) // Invalid
            .quantity(dec!(1))
            .build();

        assert!(matches!(result, Err(MatchingError::InvalidPrice(_))));
    }

    #[test]
    fn test_multiple_pairs() {
        let mut engine = create_engine();

        // Place orders on different pairs
        engine
            .place_limit_order("ALICE", TradingPair::btc_usdt(), OrderSide::Buy, dec!(50000), dec!(1))
            .unwrap();
        engine
            .place_limit_order("BOB", TradingPair::eth_usdt(), OrderSide::Sell, dec!(3000), dec!(10))
            .unwrap();

        assert_eq!(engine.best_bid(&TradingPair::btc_usdt()), Some(dec!(50000)));
        assert_eq!(engine.best_ask(&TradingPair::eth_usdt()), Some(dec!(3000)));
        assert_eq!(engine.total_order_count(), 2);
    }
}

```

## File ./bibank\crates\matching\src\error.rs:
```rust
//! Matching engine errors

use rust_decimal::Decimal;
use thiserror::Error;

/// Matching engine errors
#[derive(Debug, Error)]
pub enum MatchingError {
    /// Order not found
    #[error("Order not found: {0}")]
    OrderNotFound(String),

    /// Order already exists
    #[error("Order already exists: {0}")]
    OrderAlreadyExists(String),

    /// Invalid order quantity
    #[error("Invalid order quantity: {0}")]
    InvalidQuantity(Decimal),

    /// Invalid order price
    #[error("Invalid order price: {0}")]
    InvalidPrice(Decimal),

    /// Order already cancelled
    #[error("Order already cancelled: {0}")]
    OrderAlreadyCancelled(String),

    /// Order already filled
    #[error("Order already filled: {0}")]
    OrderAlreadyFilled(String),

    /// Trading pair not found
    #[error("Trading pair not found: {0}")]
    PairNotFound(String),

    /// Self-trade prevention
    #[error("Self-trade not allowed")]
    SelfTradeNotAllowed,
}

```

## File ./bibank\crates\matching\src\fill.rs:
```rust
//! Fill (trade match) structures

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::order::{OrderId, OrderSide, TradingPair};

/// Unique fill identifier
pub type FillId = String;

/// A fill represents a matched trade between two orders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    /// Unique fill ID
    pub id: FillId,
    /// Trading pair
    pub pair: TradingPair,
    /// Taker order ID (the incoming order that triggered the match)
    pub taker_order_id: OrderId,
    /// Maker order ID (the resting order in the book)
    pub maker_order_id: OrderId,
    /// Taker user ID
    pub taker_user_id: String,
    /// Maker user ID
    pub maker_user_id: String,
    /// Taker side (Buy or Sell)
    pub taker_side: OrderSide,
    /// Execution price (maker's limit price)
    pub price: Decimal,
    /// Fill quantity
    pub quantity: Decimal,
    /// Timestamp of the fill
    pub timestamp: DateTime<Utc>,
}

impl Fill {
    /// Create a new fill
    pub fn new(
        pair: TradingPair,
        taker_order_id: OrderId,
        maker_order_id: OrderId,
        taker_user_id: impl Into<String>,
        maker_user_id: impl Into<String>,
        taker_side: OrderSide,
        price: Decimal,
        quantity: Decimal,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pair,
            taker_order_id,
            maker_order_id,
            taker_user_id: taker_user_id.into(),
            maker_user_id: maker_user_id.into(),
            taker_side,
            price,
            quantity,
            timestamp: Utc::now(),
        }
    }

    /// Notional value of the fill (price * quantity)
    pub fn notional_value(&self) -> Decimal {
        self.price * self.quantity
    }

    /// Get the buyer user ID
    pub fn buyer_id(&self) -> &str {
        match self.taker_side {
            OrderSide::Buy => &self.taker_user_id,
            OrderSide::Sell => &self.maker_user_id,
        }
    }

    /// Get the seller user ID
    pub fn seller_id(&self) -> &str {
        match self.taker_side {
            OrderSide::Buy => &self.maker_user_id,
            OrderSide::Sell => &self.taker_user_id,
        }
    }
}

/// Result of a match operation
#[derive(Debug, Clone, Default)]
pub struct MatchResult {
    /// List of fills generated by the match
    pub fills: Vec<Fill>,
    /// Remaining quantity of the taker order (0 if fully filled)
    pub remaining_quantity: Decimal,
    /// Whether the taker order was fully filled
    pub fully_filled: bool,
}

impl MatchResult {
    /// Create an empty match result (no fills)
    pub fn empty(remaining: Decimal) -> Self {
        Self {
            fills: Vec::new(),
            remaining_quantity: remaining,
            fully_filled: false,
        }
    }

    /// Total quantity filled
    pub fn total_filled(&self) -> Decimal {
        self.fills.iter().map(|f| f.quantity).sum()
    }

    /// Total notional value of all fills
    pub fn total_notional(&self) -> Decimal {
        self.fills.iter().map(|f| f.notional_value()).sum()
    }

    /// Average execution price (if any fills)
    pub fn average_price(&self) -> Option<Decimal> {
        let total_qty = self.total_filled();
        if total_qty > Decimal::ZERO {
            Some(self.total_notional() / total_qty)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_creation() {
        let fill = Fill::new(
            TradingPair::btc_usdt(),
            "order-1".to_string(),
            "order-2".to_string(),
            "ALICE",
            "BOB",
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );

        assert_eq!(fill.taker_user_id, "ALICE");
        assert_eq!(fill.maker_user_id, "BOB");
        assert_eq!(fill.notional_value(), Decimal::from(50000));
    }

    #[test]
    fn test_fill_buyer_seller() {
        // Taker is buyer
        let fill = Fill::new(
            TradingPair::btc_usdt(),
            "order-1".to_string(),
            "order-2".to_string(),
            "ALICE",
            "BOB",
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );
        assert_eq!(fill.buyer_id(), "ALICE");
        assert_eq!(fill.seller_id(), "BOB");

        // Taker is seller
        let fill = Fill::new(
            TradingPair::btc_usdt(),
            "order-1".to_string(),
            "order-2".to_string(),
            "ALICE",
            "BOB",
            OrderSide::Sell,
            Decimal::from(50000),
            Decimal::from(1),
        );
        assert_eq!(fill.buyer_id(), "BOB");
        assert_eq!(fill.seller_id(), "ALICE");
    }

    #[test]
    fn test_match_result() {
        let mut result = MatchResult::empty(Decimal::from(1));

        result.fills.push(Fill::new(
            TradingPair::btc_usdt(),
            "order-1".to_string(),
            "order-2".to_string(),
            "ALICE",
            "BOB",
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from_str_exact("0.5").unwrap(),
        ));

        result.fills.push(Fill::new(
            TradingPair::btc_usdt(),
            "order-1".to_string(),
            "order-3".to_string(),
            "ALICE",
            "CAROL",
            OrderSide::Buy,
            Decimal::from(50100),
            Decimal::from_str_exact("0.5").unwrap(),
        ));

        assert_eq!(result.total_filled(), Decimal::from(1));
        assert_eq!(result.total_notional(), Decimal::from(50050));
        assert_eq!(result.average_price(), Some(Decimal::from(50050)));
    }
}

```

## File ./bibank\crates\matching\src\lib.rs:
```rust
//! BiBank Order Matching Engine
//!
//! CLOB (Central Limit Order Book) with price-time priority.
//! Phase 3: Limit GTC orders only.

mod engine;
mod error;
mod fill;
mod order;
mod orderbook;

pub use engine::{MatchingEngine, OrderBookDepth};
pub use error::MatchingError;
pub use fill::{Fill, MatchResult};
pub use order::{Order, OrderId, OrderSide, OrderStatus, TradingPair};
pub use orderbook::OrderBook;

```

## File ./bibank\crates\matching\src\order.rs:
```rust
//! Order types and structures

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique order identifier
pub type OrderId = String;

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    /// Get the opposite side
    pub fn opposite(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        }
    }
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
        }
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Order is open and waiting for fills
    Open,
    /// Order is partially filled
    PartiallyFilled,
    /// Order is completely filled
    Filled,
    /// Order was cancelled
    Cancelled,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Open => write!(f, "open"),
            OrderStatus::PartiallyFilled => write!(f, "partially_filled"),
            OrderStatus::Filled => write!(f, "filled"),
            OrderStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Trading pair (e.g., BTC/USDT)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradingPair {
    /// Base asset (e.g., BTC)
    pub base: String,
    /// Quote asset (e.g., USDT)
    pub quote: String,
}

impl TradingPair {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into().to_uppercase(),
            quote: quote.into().to_uppercase(),
        }
    }

    pub fn btc_usdt() -> Self {
        Self::new("BTC", "USDT")
    }

    pub fn eth_usdt() -> Self {
        Self::new("ETH", "USDT")
    }
}

impl std::fmt::Display for TradingPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// A limit order in the order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Unique order ID
    pub id: OrderId,
    /// User who placed the order
    pub user_id: String,
    /// Trading pair
    pub pair: TradingPair,
    /// Buy or Sell
    pub side: OrderSide,
    /// Limit price
    pub price: Decimal,
    /// Original quantity
    pub quantity: Decimal,
    /// Filled quantity
    pub filled: Decimal,
    /// Current status
    pub status: OrderStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Order {
    /// Create a new order
    pub fn new(
        user_id: impl Into<String>,
        pair: TradingPair,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.into(),
            pair,
            side,
            price,
            quantity,
            filled: Decimal::ZERO,
            status: OrderStatus::Open,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create order with specific ID (for testing)
    pub fn with_id(
        id: impl Into<String>,
        user_id: impl Into<String>,
        pair: TradingPair,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            user_id: user_id.into(),
            pair,
            side,
            price,
            quantity,
            filled: Decimal::ZERO,
            status: OrderStatus::Open,
            created_at: now,
            updated_at: now,
        }
    }

    /// Remaining unfilled quantity
    pub fn remaining(&self) -> Decimal {
        self.quantity - self.filled
    }

    /// Check if order is fully filled
    pub fn is_filled(&self) -> bool {
        self.remaining() <= Decimal::ZERO
    }

    /// Check if order is active (can be matched)
    pub fn is_active(&self) -> bool {
        matches!(self.status, OrderStatus::Open | OrderStatus::PartiallyFilled)
    }

    /// Fill the order by a given quantity
    pub fn fill(&mut self, fill_quantity: Decimal) {
        self.filled += fill_quantity;
        self.updated_at = Utc::now();

        if self.is_filled() {
            self.status = OrderStatus::Filled;
        } else {
            self.status = OrderStatus::PartiallyFilled;
        }
    }

    /// Cancel the order
    pub fn cancel(&mut self) {
        self.status = OrderStatus::Cancelled;
        self.updated_at = Utc::now();
    }

    /// Notional value (price * quantity)
    pub fn notional_value(&self) -> Decimal {
        self.price * self.quantity
    }

    /// Remaining notional value
    pub fn remaining_notional(&self) -> Decimal {
        self.price * self.remaining()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_creation() {
        let order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );

        assert_eq!(order.user_id, "ALICE");
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.price, Decimal::from(50000));
        assert_eq!(order.quantity, Decimal::from(1));
        assert_eq!(order.filled, Decimal::ZERO);
        assert_eq!(order.status, OrderStatus::Open);
    }

    #[test]
    fn test_order_fill() {
        let mut order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );

        // Partial fill
        order.fill(Decimal::from_str_exact("0.3").unwrap());
        assert_eq!(order.filled, Decimal::from_str_exact("0.3").unwrap());
        assert_eq!(order.remaining(), Decimal::from_str_exact("0.7").unwrap());
        assert_eq!(order.status, OrderStatus::PartiallyFilled);

        // Complete fill
        order.fill(Decimal::from_str_exact("0.7").unwrap());
        assert_eq!(order.filled, Decimal::from(1));
        assert!(order.is_filled());
        assert_eq!(order.status, OrderStatus::Filled);
    }

    #[test]
    fn test_order_cancel() {
        let mut order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );

        order.cancel();
        assert_eq!(order.status, OrderStatus::Cancelled);
        assert!(!order.is_active());
    }

    #[test]
    fn test_trading_pair_display() {
        let pair = TradingPair::btc_usdt();
        assert_eq!(pair.to_string(), "BTC/USDT");
    }

    #[test]
    fn test_order_side_opposite() {
        assert_eq!(OrderSide::Buy.opposite(), OrderSide::Sell);
        assert_eq!(OrderSide::Sell.opposite(), OrderSide::Buy);
    }
}

```

## File ./bibank\crates\matching\src\orderbook.rs:
```rust
//! Order book structure with price-time priority

use std::collections::{BTreeMap, VecDeque};

use rust_decimal::Decimal;

use crate::error::MatchingError;
use crate::fill::{Fill, MatchResult};
use crate::order::{Order, OrderId, OrderSide, OrderStatus, TradingPair};

/// An order book for a single trading pair
///
/// Uses CLOB (Central Limit Order Book) structure:
/// - Bids (buy orders): sorted by price descending (highest first)
/// - Asks (sell orders): sorted by price ascending (lowest first)
/// - At each price level: FIFO queue (time priority)
#[derive(Debug)]
pub struct OrderBook {
    /// Trading pair
    pair: TradingPair,
    /// Buy orders: price -> orders at that price (price descending for matching)
    bids: BTreeMap<PriceLevel, VecDeque<Order>>,
    /// Sell orders: price -> orders at that price (price ascending for matching)
    asks: BTreeMap<PriceLevel, VecDeque<Order>>,
    /// All orders indexed by ID (for fast lookup and cancellation)
    orders: std::collections::HashMap<OrderId, OrderLocation>,
}

/// Price level wrapper for custom ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PriceLevel(Decimal);

impl PriceLevel {
    fn new(price: Decimal) -> Self {
        Self(price)
    }
}

impl PartialOrd for PriceLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriceLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Use total ordering for Decimal
        self.0.cmp(&other.0)
    }
}

/// Location of an order in the book
#[derive(Debug, Clone)]
struct OrderLocation {
    side: OrderSide,
    price: PriceLevel,
}

impl OrderBook {
    /// Create a new order book for a trading pair
    pub fn new(pair: TradingPair) -> Self {
        Self {
            pair,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: std::collections::HashMap::new(),
        }
    }

    /// Get the trading pair
    pub fn pair(&self) -> &TradingPair {
        &self.pair
    }

    /// Get the best bid price (highest buy price)
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.keys().next_back().map(|p| p.0)
    }

    /// Get the best ask price (lowest sell price)
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.keys().next().map(|p| p.0)
    }

    /// Get the spread (ask - bid)
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Get the mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some((ask + bid) / Decimal::from(2)),
            _ => None,
        }
    }

    /// Count of all active orders
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    /// Total bid volume
    pub fn total_bid_volume(&self) -> Decimal {
        self.bids
            .values()
            .flat_map(|q| q.iter())
            .map(|o| o.remaining())
            .sum()
    }

    /// Total ask volume
    pub fn total_ask_volume(&self) -> Decimal {
        self.asks
            .values()
            .flat_map(|q| q.iter())
            .map(|o| o.remaining())
            .sum()
    }

    /// Add an order to the book (after matching, if any remaining)
    fn add_order(&mut self, order: Order) {
        let price = PriceLevel::new(order.price);
        let side = order.side;
        let order_id = order.id.clone();

        let book = match side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        };

        book.entry(price).or_insert_with(VecDeque::new).push_back(order);
        self.orders.insert(order_id, OrderLocation { side, price });
    }

    /// Remove an order from the book
    fn remove_order(&mut self, order_id: &str) -> Option<Order> {
        let location = self.orders.remove(order_id)?;

        let book = match location.side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        };

        if let Some(queue) = book.get_mut(&location.price) {
            let pos = queue.iter().position(|o| o.id == order_id)?;
            let order = queue.remove(pos)?;

            // Clean up empty price levels
            if queue.is_empty() {
                book.remove(&location.price);
            }

            return Some(order);
        }

        None
    }

    /// Get an order by ID
    pub fn get_order(&self, order_id: &str) -> Option<&Order> {
        let location = self.orders.get(order_id)?;

        let book = match location.side {
            OrderSide::Buy => &self.bids,
            OrderSide::Sell => &self.asks,
        };

        book.get(&location.price)?
            .iter()
            .find(|o| o.id == order_id)
    }

    /// Match an incoming order against the book
    ///
    /// Returns fills and any remaining quantity
    pub fn match_order(&mut self, mut order: Order) -> Result<MatchResult, MatchingError> {
        // Validate order
        if order.quantity <= Decimal::ZERO {
            return Err(MatchingError::InvalidQuantity(order.quantity));
        }
        if order.price <= Decimal::ZERO {
            return Err(MatchingError::InvalidPrice(order.price));
        }
        if order.pair != self.pair {
            return Err(MatchingError::PairNotFound(order.pair.to_string()));
        }

        let mut result = MatchResult::empty(order.quantity);

        // Get opposite side book
        let can_match = match order.side {
            OrderSide::Buy => {
                // Buy order matches against asks (sellers)
                // Match if ask price <= buy price
                |ask_price: Decimal, buy_price: Decimal| ask_price <= buy_price
            }
            OrderSide::Sell => {
                // Sell order matches against bids (buyers)
                // Match if bid price >= sell price
                |bid_price: Decimal, sell_price: Decimal| bid_price >= sell_price
            }
        };

        // Collect matched orders to remove after iteration
        let mut filled_order_ids: Vec<OrderId> = Vec::new();

        loop {
            if order.remaining() <= Decimal::ZERO {
                result.fully_filled = true;
                break;
            }

            // Get best price from opposite side
            let best_price = match order.side {
                OrderSide::Buy => self.best_ask(),
                OrderSide::Sell => self.best_bid(),
            };

            let best_price = match best_price {
                Some(p) if can_match(p, order.price) => p,
                _ => break, // No more matchable orders
            };

            // Get orders at best price
            let opposite_book = match order.side {
                OrderSide::Buy => &mut self.asks,
                OrderSide::Sell => &mut self.bids,
            };

            let price_level = PriceLevel::new(best_price);
            let queue = match opposite_book.get_mut(&price_level) {
                Some(q) => q,
                None => break,
            };

            // Match against orders at this price level (FIFO)
            while let Some(maker_order) = queue.front_mut() {
                if order.remaining() <= Decimal::ZERO {
                    break;
                }

                // Self-trade prevention
                if maker_order.user_id == order.user_id {
                    return Err(MatchingError::SelfTradeNotAllowed);
                }

                let fill_qty = order.remaining().min(maker_order.remaining());

                // Create fill
                let fill = Fill::new(
                    self.pair.clone(),
                    order.id.clone(),
                    maker_order.id.clone(),
                    order.user_id.clone(),
                    maker_order.user_id.clone(),
                    order.side,
                    maker_order.price, // Execute at maker's price
                    fill_qty,
                );

                result.fills.push(fill);

                // Update quantities
                order.fill(fill_qty);
                maker_order.fill(fill_qty);

                // Remove filled maker orders
                if maker_order.is_filled() {
                    let maker_id = maker_order.id.clone();
                    filled_order_ids.push(maker_id);
                    queue.pop_front();
                }
            }

            // Clean up empty price levels
            if queue.is_empty() {
                opposite_book.remove(&price_level);
            }
        }

        // Remove filled orders from index
        for order_id in filled_order_ids {
            self.orders.remove(&order_id);
        }

        result.remaining_quantity = order.remaining();

        // If order has remaining quantity, add it to the book
        if order.remaining() > Decimal::ZERO && order.is_active() {
            self.add_order(order);
        }

        Ok(result)
    }

    /// Cancel an order
    pub fn cancel_order(&mut self, order_id: &str) -> Result<Order, MatchingError> {
        let mut order = self.remove_order(order_id)
            .ok_or_else(|| MatchingError::OrderNotFound(order_id.to_string()))?;

        if order.status == OrderStatus::Cancelled {
            return Err(MatchingError::OrderAlreadyCancelled(order_id.to_string()));
        }
        if order.status == OrderStatus::Filled {
            return Err(MatchingError::OrderAlreadyFilled(order_id.to_string()));
        }

        order.cancel();
        Ok(order)
    }

    /// Get all bids as (price, total_quantity) tuples, sorted by price descending
    pub fn get_bids(&self, depth: usize) -> Vec<(Decimal, Decimal)> {
        self.bids
            .iter()
            .rev() // Descending by price
            .take(depth)
            .map(|(price, orders)| {
                let total_qty: Decimal = orders.iter().map(|o| o.remaining()).sum();
                (price.0, total_qty)
            })
            .collect()
    }

    /// Get all asks as (price, total_quantity) tuples, sorted by price ascending
    pub fn get_asks(&self, depth: usize) -> Vec<(Decimal, Decimal)> {
        self.asks
            .iter()
            .take(depth)
            .map(|(price, orders)| {
                let total_qty: Decimal = orders.iter().map(|o| o.remaining()).sum();
                (price.0, total_qty)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_book() -> OrderBook {
        OrderBook::new(TradingPair::btc_usdt())
    }

    #[test]
    fn test_empty_orderbook() {
        let book = create_test_book();
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.spread(), None);
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_add_buy_order_no_match() {
        let mut book = create_test_book();

        let order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );

        let result = book.match_order(order).unwrap();
        assert!(result.fills.is_empty());
        assert_eq!(result.remaining_quantity, dec!(1));
        assert!(!result.fully_filled);
        assert_eq!(book.best_bid(), Some(dec!(50000)));
        assert_eq!(book.order_count(), 1);
    }

    #[test]
    fn test_add_sell_order_no_match() {
        let mut book = create_test_book();

        let order = Order::new(
            "BOB",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(51000),
            dec!(2),
        );

        let result = book.match_order(order).unwrap();
        assert!(result.fills.is_empty());
        assert_eq!(book.best_ask(), Some(dec!(51000)));
    }

    #[test]
    fn test_full_match() {
        let mut book = create_test_book();

        // Add sell order
        let sell = Order::new(
            "BOB",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(1),
        );
        book.match_order(sell).unwrap();

        // Add matching buy order
        let buy = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        let result = book.match_order(buy).unwrap();

        assert_eq!(result.fills.len(), 1);
        assert!(result.fully_filled);
        assert_eq!(result.fills[0].quantity, dec!(1));
        assert_eq!(result.fills[0].price, dec!(50000));
        assert_eq!(book.order_count(), 0); // Both orders filled
    }

    #[test]
    fn test_partial_match() {
        let mut book = create_test_book();

        // Add small sell order
        let sell = Order::new(
            "BOB",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(0.5),
        );
        book.match_order(sell).unwrap();

        // Add larger buy order
        let buy = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        let result = book.match_order(buy).unwrap();

        assert_eq!(result.fills.len(), 1);
        assert!(!result.fully_filled);
        assert_eq!(result.remaining_quantity, dec!(0.5));
        assert_eq!(book.best_bid(), Some(dec!(50000))); // Remaining buy order
        assert_eq!(book.best_ask(), None); // Sell order fully filled
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = create_test_book();

        // Add two sell orders at same price
        let sell1 = Order::with_id(
            "sell-1",
            "BOB",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(1),
        );
        book.match_order(sell1).unwrap();

        let sell2 = Order::with_id(
            "sell-2",
            "CAROL",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(1),
        );
        book.match_order(sell2).unwrap();

        // Buy should match with first seller (BOB)
        let buy = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        let result = book.match_order(buy).unwrap();

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].maker_order_id, "sell-1");
        assert_eq!(result.fills[0].maker_user_id, "BOB");
    }

    #[test]
    fn test_cancel_order() {
        let mut book = create_test_book();

        let order = Order::with_id(
            "order-1",
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        book.match_order(order).unwrap();

        let cancelled = book.cancel_order("order-1").unwrap();
        assert_eq!(cancelled.status, OrderStatus::Cancelled);
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_cancel_nonexistent_order() {
        let mut book = create_test_book();
        let result = book.cancel_order("nonexistent");
        assert!(matches!(result, Err(MatchingError::OrderNotFound(_))));
    }

    #[test]
    fn test_self_trade_prevention() {
        let mut book = create_test_book();

        let sell = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(1),
        );
        book.match_order(sell).unwrap();

        // Same user tries to buy
        let buy = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        let result = book.match_order(buy);
        assert!(matches!(result, Err(MatchingError::SelfTradeNotAllowed)));
    }

    #[test]
    fn test_depth() {
        let mut book = create_test_book();

        // Add bids at different prices
        for price in [49000, 49500, 50000].iter() {
            let order = Order::new(
                "ALICE",
                TradingPair::btc_usdt(),
                OrderSide::Buy,
                Decimal::from(*price),
                dec!(1),
            );
            book.match_order(order).unwrap();
        }

        let bids = book.get_bids(10);
        assert_eq!(bids.len(), 3);
        // Sorted descending
        assert_eq!(bids[0].0, dec!(50000));
        assert_eq!(bids[1].0, dec!(49500));
        assert_eq!(bids[2].0, dec!(49000));
    }
}

```

## File ./bibank\crates\oracle\src\error.rs:
```rust
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

```

## File ./bibank\crates\oracle\src\lib.rs:
```rust
//! BiBank Price Oracle
//!
//! Provides price feeds for margin calculation and PnL computation.
//! Currently implements MockOracle for testing; can be extended for external feeds.

mod error;
mod mock;
mod types;

pub use error::OracleError;
pub use mock::MockOracle;
pub use types::{Price, PriceOracle, TradingPair};

```

## File ./bibank\crates\oracle\src\mock.rs:
```rust
//! Mock Oracle for testing
//!
//! Provides configurable fixed prices for testing margin calculations.

use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::error::OracleError;
use crate::types::{Price, PriceOracle, TradingPair};

/// Mock Price Oracle for testing
///
/// Stores fixed prices that can be updated programmatically.
/// Useful for unit tests and integration tests.
pub struct MockOracle {
    /// Stored prices (pair -> price)
    prices: RwLock<HashMap<String, Price>>,
}

impl MockOracle {
    /// Create a new empty mock oracle
    pub fn new() -> Self {
        Self {
            prices: RwLock::new(HashMap::new()),
        }
    }

    /// Create a mock oracle with default trading pairs
    pub fn with_defaults() -> Self {
        let oracle = Self::new();

        // Set some default prices
        oracle.set_price(TradingPair::btc_usdt(), Decimal::from(50000));
        oracle.set_price(TradingPair::eth_usdt(), Decimal::from(3000));
        oracle.set_price(TradingPair::new("SOL", "USDT"), Decimal::from(100));
        oracle.set_price(TradingPair::new("BNB", "USDT"), Decimal::from(300));

        oracle
    }

    /// Set a fixed price for a trading pair
    pub fn set_price(&self, pair: TradingPair, price: Decimal) {
        let price_obj = Price::simple(pair.clone(), price);
        let mut prices = self.prices.write().unwrap();
        prices.insert(pair.to_string(), price_obj);
    }

    /// Set a price with bid/ask spread
    pub fn set_price_with_spread(&self, pair: TradingPair, bid: Decimal, ask: Decimal) {
        let last = (bid + ask) / Decimal::from(2);
        let price_obj = Price::new(pair.clone(), bid, ask, last);
        let mut prices = self.prices.write().unwrap();
        prices.insert(pair.to_string(), price_obj);
    }

    /// Remove a price (for testing pair not found error)
    pub fn remove_price(&self, pair: &TradingPair) {
        let mut prices = self.prices.write().unwrap();
        prices.remove(&pair.to_string());
    }

    /// Get number of configured pairs
    pub fn pair_count(&self) -> usize {
        self.prices.read().unwrap().len()
    }
}

impl Default for MockOracle {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[async_trait]
impl PriceOracle for MockOracle {
    async fn get_price(&self, pair: &TradingPair) -> Result<Price, OracleError> {
        let prices = self.prices.read().unwrap();
        prices
            .get(&pair.to_string())
            .cloned()
            .ok_or_else(|| OracleError::PairNotFound {
                pair: pair.to_string(),
            })
    }

    async fn supported_pairs(&self) -> Vec<TradingPair> {
        let prices = self.prices.read().unwrap();
        prices.values().map(|p| p.pair.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_oracle_default_prices() {
        let oracle = MockOracle::with_defaults();

        let btc_price = oracle.get_price(&TradingPair::btc_usdt()).await.unwrap();
        assert_eq!(btc_price.last, Decimal::from(50000));

        let eth_price = oracle.get_price(&TradingPair::eth_usdt()).await.unwrap();
        assert_eq!(eth_price.last, Decimal::from(3000));
    }

    #[tokio::test]
    async fn test_mock_oracle_set_price() {
        let oracle = MockOracle::new();
        let pair = TradingPair::new("DOGE", "USDT");

        // Initially not set
        assert!(oracle.get_price(&pair).await.is_err());

        // Set price
        oracle.set_price(pair.clone(), Decimal::from_str_exact("0.08").unwrap());

        // Now available
        let price = oracle.get_price(&pair).await.unwrap();
        assert_eq!(price.last, Decimal::from_str_exact("0.08").unwrap());
    }

    #[tokio::test]
    async fn test_mock_oracle_pair_not_found() {
        let oracle = MockOracle::new();
        let pair = TradingPair::new("UNKNOWN", "USDT");

        let result = oracle.get_price(&pair).await;
        assert!(matches!(result, Err(OracleError::PairNotFound { .. })));
    }

    #[tokio::test]
    async fn test_mock_oracle_supported_pairs() {
        let oracle = MockOracle::with_defaults();
        let pairs = oracle.supported_pairs().await;

        assert!(pairs.len() >= 4);
        assert!(pairs.contains(&TradingPair::btc_usdt()));
        assert!(pairs.contains(&TradingPair::eth_usdt()));
    }

    #[tokio::test]
    async fn test_mock_oracle_with_spread() {
        let oracle = MockOracle::new();
        let pair = TradingPair::btc_usdt();

        oracle.set_price_with_spread(
            pair.clone(),
            Decimal::from(49900),
            Decimal::from(50100),
        );

        let price = oracle.get_price(&pair).await.unwrap();
        assert_eq!(price.bid, Decimal::from(49900));
        assert_eq!(price.ask, Decimal::from(50100));
        assert_eq!(price.mid(), Decimal::from(50000));
    }
}

```

## File ./bibank\crates\oracle\src\types.rs:
```rust
//! Core oracle types

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::OracleError;

/// A trading pair (e.g., BTC/USDT)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradingPair {
    /// Base asset (e.g., BTC)
    pub base: String,
    /// Quote asset (e.g., USDT)
    pub quote: String,
}

impl TradingPair {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into().to_uppercase(),
            quote: quote.into().to_uppercase(),
        }
    }

    /// Common pair constructors
    pub fn btc_usdt() -> Self {
        Self::new("BTC", "USDT")
    }

    pub fn eth_usdt() -> Self {
        Self::new("ETH", "USDT")
    }
}

impl std::fmt::Display for TradingPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// A price quote with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    /// The trading pair
    pub pair: TradingPair,
    /// Bid price (highest buy order)
    pub bid: Decimal,
    /// Ask price (lowest sell order)
    pub ask: Decimal,
    /// Last traded price (or mid-price if no trades)
    pub last: Decimal,
    /// Timestamp when this price was fetched
    pub timestamp: DateTime<Utc>,
    /// Source of the price (e.g., "mock", "binance", "chainlink")
    pub source: String,
}

impl Price {
    /// Create a new price with bid/ask/last
    pub fn new(pair: TradingPair, bid: Decimal, ask: Decimal, last: Decimal) -> Self {
        Self {
            pair,
            bid,
            ask,
            last,
            timestamp: Utc::now(),
            source: "unknown".to_string(),
        }
    }

    /// Create a simple price with same bid/ask/last (for mocking)
    pub fn simple(pair: TradingPair, price: Decimal) -> Self {
        Self {
            pair,
            bid: price,
            ask: price,
            last: price,
            timestamp: Utc::now(),
            source: "mock".to_string(),
        }
    }

    /// Get mid-price (average of bid and ask)
    pub fn mid(&self) -> Decimal {
        (self.bid + self.ask) / Decimal::from(2)
    }

    /// Get spread (ask - bid)
    pub fn spread(&self) -> Decimal {
        self.ask - self.bid
    }

    /// Check if price is stale (older than threshold)
    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        let age = Utc::now().signed_duration_since(self.timestamp);
        age.num_seconds() > max_age_secs as i64
    }
}

/// Price Oracle trait - interface for price feeds
///
/// Implementations can be:
/// - MockOracle: For testing with fixed prices
/// - BinanceOracle: Real-time prices from Binance API
/// - ChainlinkOracle: On-chain prices from Chainlink
#[async_trait]
pub trait PriceOracle: Send + Sync {
    /// Get the current price for a trading pair
    async fn get_price(&self, pair: &TradingPair) -> Result<Price, OracleError>;

    /// Get prices for multiple pairs at once
    async fn get_prices(&self, pairs: &[TradingPair]) -> Vec<Result<Price, OracleError>> {
        let mut results = Vec::new();
        for pair in pairs {
            results.push(self.get_price(pair).await);
        }
        results
    }

    /// Get a list of all supported trading pairs
    async fn supported_pairs(&self) -> Vec<TradingPair>;

    /// Check if a trading pair is supported
    async fn is_supported(&self, pair: &TradingPair) -> bool {
        self.supported_pairs().await.contains(pair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trading_pair_display() {
        let pair = TradingPair::btc_usdt();
        assert_eq!(pair.to_string(), "BTC/USDT");
    }

    #[test]
    fn test_price_mid() {
        let pair = TradingPair::btc_usdt();
        let price = Price::new(
            pair,
            Decimal::from(99),
            Decimal::from(101),
            Decimal::from(100),
        );
        assert_eq!(price.mid(), Decimal::from(100));
    }

    #[test]
    fn test_price_spread() {
        let pair = TradingPair::btc_usdt();
        let price = Price::new(
            pair,
            Decimal::from(99),
            Decimal::from(101),
            Decimal::from(100),
        );
        assert_eq!(price.spread(), Decimal::from(2));
    }
}

```

## File ./bibank\crates\projection\src\balance.rs:
```rust
//! Balance projection - tracks account balances from events

use bibank_ledger::JournalEntry;
use rust_decimal::Decimal;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

/// Balance projection - tracks account balances
pub struct BalanceProjection {
    pool: SqlitePool,
}

impl BalanceProjection {
    /// Create a new balance projection
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the schema
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS balances (
                account_key TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                segment TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                asset TEXT NOT NULL,
                sub_account TEXT NOT NULL,
                balance TEXT NOT NULL DEFAULT '0',
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_balances_entity
            ON balances(entity_id, asset)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Apply a journal entry to update balances
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), sqlx::Error> {
        for posting in &entry.postings {
            let key = posting.account.to_string();
            let normal_side = posting.account.category.normal_balance();

            let delta = if posting.side == normal_side {
                posting.amount.value()
            } else {
                -posting.amount.value()
            };

            // Upsert balance
            sqlx::query(
                r#"
                INSERT INTO balances (account_key, category, segment, entity_id, asset, sub_account, balance, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(account_key) DO UPDATE SET
                    balance = CAST((CAST(balance AS REAL) + CAST(? AS REAL)) AS TEXT),
                    updated_at = ?
                "#,
            )
            .bind(&key)
            .bind(posting.account.category.code())
            .bind(&posting.account.segment)
            .bind(&posting.account.id)
            .bind(&posting.account.asset)
            .bind(&posting.account.sub_account)
            .bind(delta.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .bind(delta.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get balance for a specific account
    pub async fn get_balance(&self, account_key: &str) -> Result<Decimal, sqlx::Error> {
        let row = sqlx::query("SELECT balance FROM balances WHERE account_key = ?")
            .bind(account_key)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let balance_str: String = row.get("balance");
                Ok(balance_str.parse().unwrap_or(Decimal::ZERO))
            }
            None => Ok(Decimal::ZERO),
        }
    }

    /// Get all balances for a user
    pub async fn get_user_balances(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, Decimal>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT asset, balance
            FROM balances
            WHERE segment = 'USER' AND entity_id = ? AND sub_account = 'AVAILABLE'
            "#,
        )
        .bind(user_id.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        let mut balances = HashMap::new();
        for row in rows {
            let asset: String = row.get("asset");
            let balance_str: String = row.get("balance");
            let balance: Decimal = balance_str.parse().unwrap_or(Decimal::ZERO);
            balances.insert(asset, balance);
        }

        Ok(balances)
    }

    /// Clear all balances (for replay)
    pub async fn clear(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM balances")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

```

## File ./bibank\crates\projection\src\engine.rs:
```rust
//! Projection engine - coordinates replay and updates

use crate::balance::BalanceProjection;
use crate::error::ProjectionError;
use crate::trade::TradeProjection;
use bibank_bus::EventBus;
use bibank_ledger::JournalEntry;
use sqlx::SqlitePool;
use std::path::Path;

/// Projection engine - coordinates replay and updates
pub struct ProjectionEngine {
    pub balance: BalanceProjection,
    pub trade: TradeProjection,
}

impl ProjectionEngine {
    /// Create a new projection engine
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self, ProjectionError> {
        let db_url = format!("sqlite:{}?mode=rwc", db_path.as_ref().display());
        let pool = SqlitePool::connect(&db_url).await?;

        let balance = BalanceProjection::new(pool.clone());
        balance.init().await?;

        let trade = TradeProjection::new(pool);
        trade.init().await?;

        Ok(Self { balance, trade })
    }

    /// Apply a single entry
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), ProjectionError> {
        self.balance.apply(entry).await?;
        self.trade.apply(entry).await?;
        Ok(())
    }

    /// Replay all events from the bus
    pub async fn replay(&self, bus: &EventBus) -> Result<usize, ProjectionError> {
        let reader = bus.reader()?;
        let entries = reader.read_all()?;

        self.balance.clear().await?;
        self.trade.clear().await?;

        let count = entries.len();
        for entry in &entries {
            self.balance.apply(entry).await?;
            self.trade.apply(entry).await?;
        }

        Ok(count)
    }

    /// Get the balance projection
    pub fn balance(&self) -> &BalanceProjection {
        &self.balance
    }

    /// Get the trade projection
    pub fn trade(&self) -> &TradeProjection {
        &self.trade
    }
}

```

## File ./bibank\crates\projection\src\error.rs:
```rust
//! Projection errors

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProjectionError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Event error: {0}")]
    Event(#[from] bibank_events::EventError),

    #[error("Projection not initialized")]
    NotInitialized,
}

```

## File ./bibank\crates\projection\src\lib.rs:
```rust
//! BiBank Projection - Event to SQLite views
//!
//! Projections are DISPOSABLE - they can be rebuilt from events at any time.

pub mod balance;
pub mod engine;
pub mod error;
pub mod trade;

pub use balance::BalanceProjection;
pub use engine::ProjectionEngine;
pub use error::ProjectionError;
pub use trade::{TradeProjection, TradeRecord};

```

## File ./bibank\crates\projection\src\trade.rs:
```rust
//! Trade projection - tracks trade history from events

use bibank_ledger::{JournalEntry, TransactionIntent};
use rust_decimal::Decimal;
use sqlx::{Row, SqlitePool};

/// Trade record from projection
#[derive(Debug, Clone)]
pub struct TradeRecord {
    /// Trade ID (sequence number)
    pub trade_id: u64,
    /// Seller user ID
    pub seller: String,
    /// Buyer user ID
    pub buyer: String,
    /// Asset being sold
    pub sell_asset: String,
    /// Amount being sold
    pub sell_amount: Decimal,
    /// Asset being bought
    pub buy_asset: String,
    /// Amount being bought
    pub buy_amount: Decimal,
    /// Trade timestamp
    pub timestamp: String,
    /// Entry hash
    pub hash: String,
}

/// Trade projection - tracks trade history
pub struct TradeProjection {
    pool: SqlitePool,
}

impl TradeProjection {
    /// Create a new trade projection
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the schema
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                trade_id INTEGER PRIMARY KEY,
                seller TEXT NOT NULL,
                buyer TEXT NOT NULL,
                sell_asset TEXT NOT NULL,
                sell_amount TEXT NOT NULL,
                buy_asset TEXT NOT NULL,
                buy_amount TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                hash TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_seller
            ON trades(seller)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_buyer
            ON trades(buyer)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_assets
            ON trades(sell_asset, buy_asset)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Apply a journal entry to update trades
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), sqlx::Error> {
        // Only process Trade entries
        if entry.intent != TransactionIntent::Trade {
            return Ok(());
        }

        // Extract trade info from postings
        // Trade has 4+ postings:
        // - Seller DEBIT (loses sell_asset)
        // - Seller CREDIT (gains buy_asset)
        // - Buyer DEBIT (loses buy_asset)
        // - Buyer CREDIT (gains sell_asset)

        let mut seller: Option<String> = None;
        let mut buyer: Option<String> = None;
        let mut sell_asset: Option<String> = None;
        let mut sell_amount: Option<Decimal> = None;
        let mut buy_asset: Option<String> = None;
        let mut buy_amount: Option<Decimal> = None;

        for posting in &entry.postings {
            // Only look at user LIAB accounts
            if posting.account.segment != "USER" {
                continue;
            }

            let user_id = &posting.account.id;
            let asset = &posting.account.asset;

            // DEBIT on LIAB = user is paying (losing)
            // CREDIT on LIAB = user is receiving (gaining)
            use bibank_ledger::entry::Side;

            match posting.side {
                Side::Debit => {
                    // User is losing this asset
                    if seller.is_none() || seller.as_ref() == Some(user_id) {
                        seller = Some(user_id.clone());
                        sell_asset = Some(asset.clone());
                        sell_amount = Some(posting.amount.value());
                    } else {
                        buyer = Some(user_id.clone());
                        buy_asset = Some(asset.clone());
                        buy_amount = Some(posting.amount.value());
                    }
                }
                Side::Credit => {
                    // User is gaining this asset
                    if buyer.is_none() || buyer.as_ref() == Some(user_id) {
                        buyer = Some(user_id.clone());
                    } else {
                        seller = Some(user_id.clone());
                    }
                }
            }
        }

        // Insert trade record
        if let (
            Some(seller),
            Some(buyer),
            Some(sell_asset),
            Some(sell_amount),
            Some(buy_asset),
            Some(buy_amount),
        ) = (seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount)
        {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO trades
                (trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(entry.sequence as i64)
            .bind(&seller)
            .bind(&buyer)
            .bind(&sell_asset)
            .bind(sell_amount.to_string())
            .bind(&buy_asset)
            .bind(buy_amount.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .bind(&entry.hash)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get all trades for a user (as seller or buyer)
    pub async fn get_user_trades(&self, user_id: &str) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            WHERE seller = ? OR buyer = ?
            ORDER BY trade_id DESC
            "#,
        )
        .bind(user_id.to_uppercase())
        .bind(user_id.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get trades for a trading pair
    pub async fn get_pair_trades(
        &self,
        base_asset: &str,
        quote_asset: &str,
    ) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            WHERE (sell_asset = ? AND buy_asset = ?) OR (sell_asset = ? AND buy_asset = ?)
            ORDER BY trade_id DESC
            "#,
        )
        .bind(base_asset.to_uppercase())
        .bind(quote_asset.to_uppercase())
        .bind(quote_asset.to_uppercase())
        .bind(base_asset.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get recent trades (all)
    pub async fn get_recent_trades(&self, limit: u32) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            ORDER BY trade_id DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get trade count
    pub async fn count(&self) -> Result<u64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM trades")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64, _>("count") as u64)
    }

    /// Clear all trades (for replay)
    pub async fn clear(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM trades")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

```

## File ./bibank\crates\risk\src\engine.rs:
```rust
//! Risk engine implementation

use crate::error::RiskError;
use crate::state::RiskState;
use bibank_ledger::{JournalEntry, UnsignedEntry};

/// Risk Engine - Pre-commit gatekeeper
///
/// Validates transactions before they are committed to the ledger.
/// Maintains in-memory state rebuilt from event replay.
pub struct RiskEngine {
    state: RiskState,
}

impl RiskEngine {
    /// Create a new empty risk engine
    pub fn new() -> Self {
        Self {
            state: RiskState::new(),
        }
    }

    /// Get reference to internal state
    pub fn state(&self) -> &RiskState {
        &self.state
    }

    /// Get mutable reference to internal state
    pub fn state_mut(&mut self) -> &mut RiskState {
        &mut self.state
    }

    /// Check if an unsigned entry passes all risk checks
    ///
    /// Returns Ok(()) if the entry is allowed, Err otherwise.
    pub fn check(&self, entry: &UnsignedEntry) -> Result<(), RiskError> {
        // Build a temporary JournalEntry for checking
        let temp_entry = JournalEntry {
            sequence: 0,
            prev_hash: String::new(),
            hash: String::new(),
            timestamp: chrono::Utc::now(),
            intent: entry.intent,
            correlation_id: entry.correlation_id.clone(),
            causality_id: entry.causality_id.clone(),
            postings: entry.postings.clone(),
            metadata: entry.metadata.clone(),
            signatures: Vec::new(),
        };

        self.check_entry(&temp_entry)
    }

    /// Check if a journal entry passes all risk checks
    pub fn check_entry(&self, entry: &JournalEntry) -> Result<(), RiskError> {
        // Rule 1: Check sufficient balance for liability accounts
        let violations = self.state.check_sufficient_balance(entry);

        if let Some((account, _balance)) = violations.first() {
            // Find the required amount from postings
            let required = entry
                .postings
                .iter()
                .find(|p| p.account.to_string() == *account)
                .map(|p| p.amount.value())
                .unwrap_or_default();

            let available = self.state.get_balance(
                &account
                    .parse()
                    .map_err(|_| RiskError::AccountNotFound(account.clone()))?,
            );

            return Err(RiskError::InsufficientBalance {
                account: account.clone(),
                available: available.to_string(),
                required: required.to_string(),
            });
        }

        Ok(())
    }

    /// Apply a committed entry to update internal state
    ///
    /// This should be called AFTER the entry is committed to ledger.
    pub fn apply(&mut self, entry: &JournalEntry) {
        self.state.apply_entry(entry);
    }

    /// Rebuild state from a sequence of entries (replay)
    pub fn replay<'a>(&mut self, entries: impl Iterator<Item = &'a JournalEntry>) {
        self.state.clear();
        for entry in entries {
            self.state.apply_entry(entry);
        }
    }
}

impl Default for RiskEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_core::Amount;
    use bibank_ledger::{AccountKey, JournalEntryBuilder, TransactionIntent};
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    #[test]
    fn test_deposit_always_allowed() {
        let engine = RiskEngine::new();

        let entry = JournalEntryBuilder::new()
            .intent(TransactionIntent::Deposit)
            .correlation_id("test-1")
            .debit(AccountKey::system_vault("USDT"), amount(100))
            .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .build_unsigned()
            .unwrap();

        assert!(engine.check(&entry).is_ok());
    }

    #[test]
    fn test_withdrawal_blocked_on_insufficient_balance() {
        let engine = RiskEngine::new();

        // No prior deposits, try to withdraw
        let entry = JournalEntryBuilder::new()
            .intent(TransactionIntent::Withdrawal)
            .correlation_id("test-1")
            .debit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .credit(AccountKey::system_vault("USDT"), amount(100))
            .build_unsigned()
            .unwrap();

        let result = engine.check(&entry);
        assert!(matches!(result, Err(RiskError::InsufficientBalance { .. })));
    }
}

```

## File ./bibank\crates\risk\src\error.rs:
```rust
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

```

## File ./bibank\crates\risk\src\interest.rs:
```rust
//! Interest Accrual Module
//!
//! Handles compound interest calculation for margin loans.
//! Interest is accrued daily and added to the loan principal.

use bibank_core::Amount;
use bibank_ledger::{AccountKey, Posting, TransactionIntent, UnsignedEntry};
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;

use crate::state::RiskState;

/// Default interest rate (0.05% daily = ~18.25% APR)
pub const DEFAULT_DAILY_RATE: Decimal = Decimal::from_parts(5, 0, 0, false, 4); // 0.0005

/// Interest accrual calculator
pub struct InterestCalculator {
    /// Daily interest rate (e.g., 0.0005 for 0.05%)
    daily_rate: Decimal,
}

impl InterestCalculator {
    /// Create a new calculator with default rate
    pub fn new() -> Self {
        Self {
            daily_rate: DEFAULT_DAILY_RATE,
        }
    }

    /// Create with custom daily rate
    pub fn with_rate(daily_rate: Decimal) -> Self {
        Self { daily_rate }
    }

    /// Calculate interest for a single loan
    pub fn calculate_interest(&self, principal: Decimal) -> Decimal {
        principal * self.daily_rate
    }

    /// Generate interest entries for all active loans
    ///
    /// Returns a list of UnsignedEntry for each user/asset with a loan.
    /// Each entry has:
    /// - Debit: ASSET:USER:*:*:LOAN (loan increases)
    /// - Credit: REV:SYSTEM:INTEREST:*:INCOME (revenue increases)
    pub fn generate_interest_entries(
        &self,
        state: &RiskState,
        correlation_prefix: &str,
    ) -> Vec<UnsignedEntry> {
        let mut entries = Vec::new();

        // Scan all balances for LOAN accounts
        for (account_key, &balance) in state.all_balances() {
            // Only process ASSET:USER:*:*:LOAN with positive balance
            if account_key.starts_with("ASSET:USER:")
                && account_key.ends_with(":LOAN")
                && balance > Decimal::ZERO
            {
                // Parse: ASSET:USER:alice:USDT:LOAN
                let parts: Vec<&str> = account_key.split(':').collect();
                if parts.len() == 5 {
                    let user = parts[2];
                    let asset = parts[3];

                    let interest = self.calculate_interest(balance);
                    if interest > Decimal::ZERO {
                        if let Some(entry) = self.create_interest_entry(
                            user,
                            asset,
                            interest,
                            &format!("{}-{}-{}", correlation_prefix, user, asset),
                        ) {
                            entries.push(entry);
                        }
                    }
                }
            }
        }

        entries
    }

    /// Create an interest entry for a single user/asset
    fn create_interest_entry(
        &self,
        user: &str,
        asset: &str,
        interest: Decimal,
        correlation_id: &str,
    ) -> Option<UnsignedEntry> {
        let amount = Amount::new(interest).ok()?;

        // LOAN account (BiBank's receivable increases)
        let loan_account = RiskState::loan_account(user, asset);

        // Revenue account (BiBank's income increases)
        let revenue_account = AccountKey::new(
            bibank_ledger::AccountCategory::Revenue,
            "SYSTEM",
            "INTEREST",
            asset,
            "INCOME",
        );

        let mut metadata = HashMap::new();
        metadata.insert("interest_rate".to_string(), json!(self.daily_rate.to_string()));
        metadata.insert("principal".to_string(), json!(interest.to_string()));
        metadata.insert("accrual_type".to_string(), json!("compound"));

        Some(UnsignedEntry {
            intent: TransactionIntent::Interest,
            correlation_id: correlation_id.to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(loan_account, amount.clone()),
                Posting::credit(revenue_account, amount),
            ],
            metadata,
        })
    }

    /// Get the daily rate
    pub fn daily_rate(&self) -> Decimal {
        self.daily_rate
    }

    /// Get annualized rate (APR, simple approximation)
    pub fn annual_rate(&self) -> Decimal {
        self.daily_rate * Decimal::from(365)
    }
}

impl Default for InterestCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_ledger::{JournalEntry, Posting};
    use chrono::Utc;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    fn deposit_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    fn borrow_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Borrow,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(RiskState::loan_account(user, "USDT"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_calculate_interest_basic() {
        let calc = InterestCalculator::new();

        // 1000 USDT loan at 0.05% daily = 0.5 USDT interest
        let interest = calc.calculate_interest(Decimal::from(1000));
        assert_eq!(interest, Decimal::from_str_exact("0.5").unwrap());
    }

    #[test]
    fn test_calculate_interest_custom_rate() {
        // 0.1% daily rate
        let calc = InterestCalculator::with_rate(Decimal::from_str_exact("0.001").unwrap());

        // 1000 USDT loan at 0.1% daily = 1 USDT interest
        let interest = calc.calculate_interest(Decimal::from(1000));
        assert_eq!(interest, Decimal::from(1));
    }

    #[test]
    fn test_generate_interest_entries_single_user() {
        let mut state = RiskState::new();
        let calc = InterestCalculator::new();

        // Alice deposits 100 and borrows 500
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 500));

        // Generate interest entries
        let entries = calc.generate_interest_entries(&state, "daily-2025-01-01");

        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.intent, TransactionIntent::Interest);
        assert_eq!(entry.postings.len(), 2);

        // Interest = 500 * 0.0005 = 0.25 USDT
        assert_eq!(
            entry.postings[0].amount.value(),
            Decimal::from_str_exact("0.25").unwrap()
        );
    }

    #[test]
    fn test_generate_interest_entries_multiple_users() {
        let mut state = RiskState::new();
        let calc = InterestCalculator::new();

        // Alice borrows 1000
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 1000));

        // Bob borrows 2000
        state.apply_entry(&deposit_entry("BOB", 200));
        state.apply_entry(&borrow_entry("BOB", 2000));

        let entries = calc.generate_interest_entries(&state, "daily");

        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_no_interest_for_zero_loan() {
        let mut state = RiskState::new();
        let calc = InterestCalculator::new();

        // Alice only deposits, no loan
        state.apply_entry(&deposit_entry("ALICE", 100));

        let entries = calc.generate_interest_entries(&state, "daily");

        assert!(entries.is_empty());
    }

    #[test]
    fn test_compound_interest_simulation() {
        let mut state = RiskState::new();
        let calc = InterestCalculator::new();

        // Alice deposits 100 and borrows 1000
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 1000));

        // Day 1: Interest = 1000 * 0.0005 = 0.5
        let day1_entries = calc.generate_interest_entries(&state, "day1");
        assert_eq!(day1_entries.len(), 1);

        // Simulate applying interest (convert to JournalEntry)
        let day1_interest = JournalEntry {
            sequence: 10,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Interest,
            correlation_id: "day1".to_string(),
            causality_id: None,
            postings: day1_entries[0].postings.clone(),
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };
        state.apply_entry(&day1_interest);

        // Day 2: Loan is now 1000.5, Interest = 1000.5 * 0.0005 = 0.50025
        let loan_after = state.get_loan_balance("ALICE", "USDT");
        assert_eq!(loan_after, Decimal::from_str_exact("1000.5").unwrap());

        // Generate day 2 interest
        let day2_entries = calc.generate_interest_entries(&state, "day2");
        assert_eq!(
            day2_entries[0].postings[0].amount.value(),
            Decimal::from_str_exact("0.50025").unwrap()
        );
    }
}

```

## File ./bibank\crates\risk\src\lib.rs:
```rust
//! BiBank Risk Engine - Pre-commit gatekeeper
//!
//! The Risk Engine validates transactions BEFORE they are committed to the ledger.
//! It maintains in-memory state rebuilt from event replay on startup.

pub mod engine;
pub mod error;
pub mod interest;
pub mod liquidation;
pub mod state;

pub use engine::RiskEngine;
pub use error::RiskError;
pub use interest::{InterestCalculator, DEFAULT_DAILY_RATE};
pub use liquidation::{LiquidationConfig, LiquidationEngine, LiquidationResult};
pub use state::{MarginError, RiskState, INITIAL_MARGIN, LIQUIDATION_THRESHOLD, MAINTENANCE_MARGIN, MAX_LEVERAGE};

```

## File ./bibank\crates\risk\src\liquidation.rs:
```rust
//! Liquidation engine for margin positions
//!
//! Handles auto-liquidation when margin ratio drops below maintenance threshold.
//! Integrates with insurance fund for shortfall coverage.

use bibank_core::Amount;
use bibank_ledger::{AccountCategory, AccountKey, JournalEntryBuilder, TransactionIntent, UnsignedEntry, LedgerError};
use rust_decimal::Decimal;
use serde_json::json;

use crate::state::{RiskState, LIQUIDATION_THRESHOLD};

/// Liquidation result
#[derive(Debug, Clone)]
pub struct LiquidationResult {
    /// User being liquidated
    pub user_id: String,
    /// Asset being liquidated
    pub asset: String,
    /// Collateral seized
    pub collateral_seized: Decimal,
    /// Loan repaid
    pub loan_repaid: Decimal,
    /// Penalty amount
    pub penalty: Decimal,
    /// Insurance fund contribution (if any shortfall)
    pub insurance_contribution: Decimal,
    /// Whether liquidation was full or partial
    pub is_full_liquidation: bool,
}

/// Configuration for liquidation engine
#[derive(Debug, Clone)]
pub struct LiquidationConfig {
    /// Liquidation penalty rate (e.g., 0.05 = 5%)
    pub penalty_rate: Decimal,
    /// Maximum liquidation per call (e.g., 0.5 = 50% of position)
    pub max_liquidation_ratio: Decimal,
    /// Minimum profit for liquidator incentive
    pub liquidator_bonus_rate: Decimal,
}

impl Default for LiquidationConfig {
    fn default() -> Self {
        Self {
            penalty_rate: Decimal::new(5, 2),           // 5%
            max_liquidation_ratio: Decimal::new(50, 2), // 50%
            liquidator_bonus_rate: Decimal::new(1, 2),  // 1%
        }
    }
}

/// Liquidation engine
#[derive(Debug)]
pub struct LiquidationEngine {
    config: LiquidationConfig,
}

impl Default for LiquidationEngine {
    fn default() -> Self {
        Self::new(LiquidationConfig::default())
    }
}

impl LiquidationEngine {
    /// Create a new liquidation engine
    pub fn new(config: LiquidationConfig) -> Self {
        Self { config }
    }

    /// Get the configuration
    pub fn config(&self) -> &LiquidationConfig {
        &self.config
    }

    /// Check if a user is eligible for liquidation
    pub fn is_liquidatable(&self, state: &RiskState, user_id: &str, asset: &str) -> bool {
        state.is_liquidatable(user_id, asset)
    }

    /// Calculate liquidation amount for a user
    ///
    /// Returns (collateral_to_seize, loan_to_repay, penalty)
    pub fn calculate_liquidation(
        &self,
        state: &RiskState,
        user_id: &str,
        asset: &str,
        price: Decimal,
    ) -> Option<(Decimal, Decimal, Decimal)> {
        // Get user's loan balance
        let loan_account = AccountKey::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOAN",
        );
        let loan_balance = state.get_balance(&loan_account);

        if loan_balance.is_zero() {
            return None;
        }

        // Get user's collateral (available balance)
        let collateral = state.get_balance(&AccountKey::user_available(user_id, asset));

        // Check margin ratio
        let margin_ratio = if !loan_balance.is_zero() {
            (collateral * price) / loan_balance
        } else {
            return None;
        };

        // Only liquidate if below threshold
        if margin_ratio >= LIQUIDATION_THRESHOLD {
            return None;
        }

        // Calculate how much to liquidate (partial liquidation)
        let max_liquidation = loan_balance * self.config.max_liquidation_ratio;
        let liquidation_amount = loan_balance.min(max_liquidation);

        // Calculate penalty
        let penalty = liquidation_amount * self.config.penalty_rate;

        // Total collateral to seize
        let collateral_to_seize = liquidation_amount + penalty;

        Some((collateral_to_seize, liquidation_amount, penalty))
    }

    /// Generate a liquidation journal entry
    pub fn generate_liquidation_entry(
        &self,
        user_id: &str,
        liquidator_id: &str,
        asset: &str,
        collateral_seized: Decimal,
        loan_repaid: Decimal,
        penalty: Decimal,
        correlation_id: &str,
    ) -> Result<UnsignedEntry, LedgerError> {
        let collateral_amt = Amount::new(collateral_seized)
            .map_err(|_| LedgerError::InvalidAccountFormat("invalid collateral amount".to_string()))?;
        let loan_amt = Amount::new(loan_repaid)
            .map_err(|_| LedgerError::InvalidAccountFormat("invalid loan amount".to_string()))?;

        // User accounts
        let user_available = AccountKey::user_available(user_id, asset);
        let user_loan = AccountKey::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOAN",
        );

        // Liquidator account
        let liquidator_available = AccountKey::user_available(liquidator_id, asset);

        // System accounts
        let insurance_fund = AccountKey::new(
            AccountCategory::Asset,
            "SYSTEM",
            "INSURANCE_FUND",
            asset,
            "MAIN",
        );

        // Liquidator bonus is a portion of the penalty
        let liquidator_bonus = penalty * self.config.liquidator_bonus_rate / self.config.penalty_rate;
        let insurance_portion = penalty - liquidator_bonus;

        let liquidator_bonus_amt = Amount::new(liquidator_bonus)
            .map_err(|_| LedgerError::InvalidAccountFormat("invalid bonus amount".to_string()))?;
        let insurance_portion_amt = Amount::new(insurance_portion)
            .map_err(|_| LedgerError::InvalidAccountFormat("invalid insurance amount".to_string()))?;

        // Double-entry accounting for liquidation:
        // collateral_seized = loan_repaid + penalty = loan_repaid + insurance_portion + liquidator_bonus
        //
        // 1. User loses collateral (Credit user_available = -collateral_seized)
        // 2. User's loan liability decreases (Debit user_loan = +loan_repaid)
        // 3. Insurance fund receives portion of penalty (Debit insurance_fund = +insurance_portion)
        // 4. Liquidator receives bonus (Debit liquidator_available = +liquidator_bonus)
        //
        // Balance check:
        // -collateral_seized + loan_repaid + insurance_portion + liquidator_bonus = 0 ✓

        let entry = JournalEntryBuilder::new()
            .intent(TransactionIntent::Liquidation)
            .correlation_id(correlation_id)
            // Seize collateral from user (reduces user's asset balance)
            .credit(user_available.clone(), collateral_amt)
            // Reduce user's loan liability
            .debit(user_loan, loan_amt)
            // Insurance fund receives portion of penalty
            .debit(insurance_fund, insurance_portion_amt)
            // Liquidator receives bonus
            .debit(liquidator_available, liquidator_bonus_amt)
            .metadata("liquidated_user", json!(user_id))
            .metadata("liquidator", json!(liquidator_id))
            .metadata("asset", json!(asset))
            .metadata("collateral_seized", json!(collateral_seized.to_string()))
            .metadata("loan_repaid", json!(loan_repaid.to_string()))
            .metadata("penalty", json!(penalty.to_string()))
            .build_unsigned()?;

        Ok(entry)
    }

    /// Execute liquidation for a user
    ///
    /// Returns the liquidation result or None if user is not liquidatable
    pub fn execute_liquidation(
        &self,
        state: &RiskState,
        user_id: &str,
        liquidator_id: &str,
        asset: &str,
        price: Decimal,
        correlation_id: &str,
    ) -> Result<Option<(UnsignedEntry, LiquidationResult)>, LedgerError> {
        // Calculate liquidation amounts
        let (collateral_seized, loan_repaid, penalty) =
            match self.calculate_liquidation(state, user_id, asset, price) {
                Some(amounts) => amounts,
                None => return Ok(None),
            };

        // Check if user has enough collateral
        let user_balance = state.get_balance(&AccountKey::user_available(user_id, asset));

        let (actual_seized, insurance_contribution) = if collateral_seized > user_balance {
            // Shortfall - insurance fund covers the difference
            let shortfall = collateral_seized - user_balance;
            (user_balance, shortfall)
        } else {
            (collateral_seized, Decimal::ZERO)
        };

        // Generate journal entry
        let entry = self.generate_liquidation_entry(
            user_id,
            liquidator_id,
            asset,
            actual_seized,
            loan_repaid,
            penalty,
            correlation_id,
        )?;

        let result = LiquidationResult {
            user_id: user_id.to_string(),
            asset: asset.to_string(),
            collateral_seized: actual_seized,
            loan_repaid,
            penalty,
            insurance_contribution,
            is_full_liquidation: actual_seized >= collateral_seized,
        };

        Ok(Some((entry, result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_ledger::{JournalEntry, Posting, Side};
    use chrono::Utc;

    fn create_test_state() -> RiskState {
        RiskState::new()
    }

    // Helper to create a deposit entry for testing
    fn deposit_entry(user: &str, amount: i64) -> JournalEntry {
        let amt = Amount::new(Decimal::from(amount)).unwrap();
        JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::new(AccountKey::system_vault("USDT"), amt, Side::Debit),
                Posting::new(AccountKey::user_available(user, "USDT"), amt, Side::Credit),
            ],
            metadata: Default::default(),
            signatures: vec![],
        }
    }

    // Helper to create a borrow entry for testing
    fn borrow_entry(user: &str, amount: i64) -> JournalEntry {
        let amt = Amount::new(Decimal::from(amount)).unwrap();
        JournalEntry {
            sequence: 2,
            prev_hash: "test".to_string(),
            hash: "test2".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Borrow,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::new(
                    AccountKey::new(AccountCategory::Asset, "SYSTEM", "LENDING_POOL", "USDT", "MAIN"),
                    amt,
                    Side::Debit,
                ),
                Posting::new(
                    AccountKey::new(AccountCategory::Liability, "USER", user, "USDT", "LOAN"),
                    amt,
                    Side::Credit,
                ),
                Posting::new(
                    AccountKey::user_available(user, "USDT"),
                    amt,
                    Side::Credit,
                ),
                Posting::new(
                    AccountKey::new(AccountCategory::Asset, "SYSTEM", "LOANS_RECEIVABLE", "USDT", "MAIN"),
                    amt,
                    Side::Debit,
                ),
            ],
            metadata: Default::default(),
            signatures: vec![],
        }
    }

    #[test]
    fn test_liquidation_config_default() {
        let config = LiquidationConfig::default();
        assert_eq!(config.penalty_rate, Decimal::new(5, 2));
        assert_eq!(config.max_liquidation_ratio, Decimal::new(50, 2));
    }

    #[test]
    fn test_liquidation_engine_creation() {
        let engine = LiquidationEngine::default();
        assert_eq!(engine.config().penalty_rate, Decimal::new(5, 2));
    }

    #[test]
    fn test_calculate_liquidation_no_loan() {
        let engine = LiquidationEngine::default();
        let state = create_test_state();

        let result = engine.calculate_liquidation(&state, "ALICE", "USDT", Decimal::ONE);
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_liquidation_healthy_position() {
        let engine = LiquidationEngine::default();
        let mut state = create_test_state();

        // Deposit 1000 USDT
        state.apply_entry(&deposit_entry("ALICE", 1000));

        // Borrow only 100 USDT (very healthy 11:1 ratio after receiving borrowed funds)
        state.apply_entry(&borrow_entry("ALICE", 100));

        let result = engine.calculate_liquidation(&state, "ALICE", "USDT", Decimal::ONE);
        assert!(result.is_none()); // Should not be liquidatable
    }

    #[test]
    fn test_calculate_liquidation_underwater_position() {
        let engine = LiquidationEngine::default();
        let mut state = create_test_state();

        // Deposit 100 USDT
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Borrow 100 USDT (will have 200 balance, 100 loan = 2:1 ratio)
        state.apply_entry(&borrow_entry("ALICE", 100));

        // Now balance is 200, loan is 100 - ratio is 2:1 (healthy)
        let result = engine.calculate_liquidation(&state, "ALICE", "USDT", Decimal::ONE);
        // This should return None because 200/100 = 2.0 > LIQUIDATION_THRESHOLD (1.1)
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_liquidation_entry() {
        let engine = LiquidationEngine::default();

        let entry = engine
            .generate_liquidation_entry("ALICE", "LIQUIDATOR", "USDT", Decimal::from(100), Decimal::from(95), Decimal::from(5), "liq-001")
            .unwrap();

        assert_eq!(entry.intent, TransactionIntent::Liquidation);
        assert!(!entry.postings.is_empty());
    }
}

```

## File ./bibank\crates\risk\src\state.rs:
```rust
//! In-memory risk state
//!
//! This state is rebuilt from ledger replay on startup.
//! It tracks balances for pre-commit validation.

use bibank_ledger::{AccountCategory, AccountKey, JournalEntry, Posting};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Constants for margin trading
pub const MAX_LEVERAGE: Decimal = Decimal::from_parts(10, 0, 0, false, 0); // 10x
pub const MAINTENANCE_MARGIN: Decimal = Decimal::from_parts(5, 0, 0, false, 2); // 5%
pub const INITIAL_MARGIN: Decimal = Decimal::from_parts(10, 0, 0, false, 2); // 10%
pub const LIQUIDATION_THRESHOLD: Decimal = Decimal::from_parts(1, 0, 0, false, 0); // Ratio < 1.0

/// In-memory balance state for risk checking
#[derive(Debug, Default)]
pub struct RiskState {
    /// Balance per account (AccountKey string -> Decimal)
    /// Note: Can be negative during calculation, but post-check should be >= 0
    balances: HashMap<String, Decimal>,
}

impl RiskState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get balance for an account (returns 0 if not found)
    pub fn get_balance(&self, account: &AccountKey) -> Decimal {
        self.balances
            .get(&account.to_string())
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Apply a journal entry to update balances
    ///
    /// For liability accounts (user balances):
    /// - Credit increases balance (money owed to user)
    /// - Debit decreases balance (user spending)
    pub fn apply_entry(&mut self, entry: &JournalEntry) {
        for posting in &entry.postings {
            self.apply_posting(posting);
        }
    }

    /// Apply a single posting
    fn apply_posting(&mut self, posting: &Posting) {
        let key = posting.account.to_string();
        let current = self.balances.entry(key).or_insert(Decimal::ZERO);

        // For LIAB accounts: Credit = increase, Debit = decrease
        // For ASSET accounts: Debit = increase, Credit = decrease
        let normal_side = posting.account.category.normal_balance();

        let delta = if posting.side == normal_side {
            // Same as normal balance = increase
            posting.amount.value()
        } else {
            // Opposite = decrease
            -posting.amount.value()
        };

        *current += delta;
    }

    /// Calculate projected balances after applying an entry (without mutating state)
    pub fn project_balances(&self, entry: &JournalEntry) -> HashMap<String, Decimal> {
        let mut projected = self.balances.clone();

        for posting in &entry.postings {
            let key = posting.account.to_string();
            let current = projected.entry(key).or_insert(Decimal::ZERO);

            let normal_side = posting.account.category.normal_balance();
            let delta = if posting.side == normal_side {
                posting.amount.value()
            } else {
                -posting.amount.value()
            };

            *current += delta;
        }

        projected
    }

    /// Check if all LIAB accounts would have non-negative balance after applying entry
    pub fn check_sufficient_balance(&self, entry: &JournalEntry) -> Vec<(String, Decimal)> {
        let projected = self.project_balances(entry);
        let mut violations = Vec::new();

        for posting in &entry.postings {
            // Only check liability accounts (user balances)
            if posting.account.category == bibank_ledger::AccountCategory::Liability {
                let key = posting.account.to_string();
                if let Some(&balance) = projected.get(&key) {
                    if balance < Decimal::ZERO {
                        violations.push((key, balance));
                    }
                }
            }
        }

        violations
    }

    /// Get all account balances (for debugging/testing)
    pub fn all_balances(&self) -> &HashMap<String, Decimal> {
        &self.balances
    }

    /// Clear all state (for testing)
    pub fn clear(&mut self) {
        self.balances.clear();
    }

    // === Phase 3: Margin Trading Methods ===

    /// Create a LOAN account key for a user
    pub fn loan_account(user: &str, asset: &str) -> AccountKey {
        AccountKey::new(AccountCategory::Asset, "USER", user, asset, "LOAN")
    }

    /// Get the loan balance for a user/asset
    /// LOAN is stored in ASSET:USER:*:*:LOAN
    pub fn get_loan_balance(&self, user: &str, asset: &str) -> Decimal {
        let account = Self::loan_account(user, asset);
        self.get_balance(&account)
    }

    /// Get available balance for a user/asset
    pub fn get_available_balance(&self, user: &str, asset: &str) -> Decimal {
        let account = AccountKey::user_available(user, asset);
        self.get_balance(&account)
    }

    /// Calculate equity for a user/asset
    /// Equity = Available - Loan
    /// Note: This is simplified - real equity would include unrealized PnL
    pub fn get_equity(&self, user: &str, asset: &str) -> Decimal {
        let available = self.get_available_balance(user, asset);
        let loan = self.get_loan_balance(user, asset);
        available - loan
    }

    /// Calculate margin ratio for a user/asset
    /// Margin Ratio = (Available / Loan) if Loan > 0, else infinity (represented as 100.0)
    pub fn get_margin_ratio(&self, user: &str, asset: &str) -> Decimal {
        let available = self.get_available_balance(user, asset);
        let loan = self.get_loan_balance(user, asset);

        if loan <= Decimal::ZERO {
            // No loan = infinite margin (represented as 100.0)
            Decimal::from(100)
        } else {
            available / loan
        }
    }

    /// Check if a borrow would exceed max leverage
    /// Returns true if the borrow is allowed
    pub fn check_borrow_allowed(
        &self,
        user: &str,
        asset: &str,
        borrow_amount: Decimal,
    ) -> Result<(), MarginError> {
        let current_available = self.get_available_balance(user, asset);
        let current_loan = self.get_loan_balance(user, asset);

        // After borrow: available += borrow_amount, loan += borrow_amount
        let new_loan = current_loan + borrow_amount;

        // When you borrow, your available goes up, your loan goes up by same amount
        // So your equity (Available - Loan) stays the same
        // The constraint is: Equity >= Loan * Initial_Margin
        // Or: Equity / Loan >= Initial_Margin
        // Or: current_available / new_loan >= Initial_Margin

        let equity = current_available; // Equity doesn't change after borrow
        if new_loan > Decimal::ZERO {
            let margin_ratio = equity / new_loan;
            if margin_ratio < INITIAL_MARGIN {
                return Err(MarginError::ExceedsMaxLeverage {
                    requested: borrow_amount.to_string(),
                    max_allowed: (equity / INITIAL_MARGIN - current_loan).max(Decimal::ZERO).to_string(),
                    current_margin_ratio: margin_ratio.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Check if a user is subject to liquidation
    /// Returns true if margin ratio < 1.0
    pub fn is_liquidatable(&self, user: &str, asset: &str) -> bool {
        let margin_ratio = self.get_margin_ratio(user, asset);
        margin_ratio < LIQUIDATION_THRESHOLD
    }

    /// Get all users with positions that may need liquidation
    /// Scans all LOAN accounts and checks margin ratio
    pub fn get_liquidatable_positions(&self) -> Vec<(String, String, Decimal)> {
        let mut positions = Vec::new();

        for (key, &balance) in &self.balances {
            // Only check ASSET:USER:*:*:LOAN accounts with positive balance
            if key.starts_with("ASSET:USER:") && key.ends_with(":LOAN") && balance > Decimal::ZERO {
                // Parse: ASSET:USER:alice:USDT:LOAN
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() == 5 {
                    let user = parts[2];
                    let asset = parts[3];
                    if self.is_liquidatable(user, asset) {
                        let margin_ratio = self.get_margin_ratio(user, asset);
                        positions.push((user.to_string(), asset.to_string(), margin_ratio));
                    }
                }
            }
        }

        positions
    }
}

/// Margin-related errors
#[derive(Debug, Clone)]
pub enum MarginError {
    ExceedsMaxLeverage {
        requested: String,
        max_allowed: String,
        current_margin_ratio: String,
    },
    InsufficientMargin {
        user: String,
        asset: String,
        current_ratio: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_core::Amount;
    use bibank_ledger::TransactionIntent;
    use chrono::Utc;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    fn deposit_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_deposit_increases_user_balance() {
        let mut state = RiskState::new();
        let entry = deposit_entry("ALICE", 100);

        state.apply_entry(&entry);

        let alice_balance = state.get_balance(&AccountKey::user_available("ALICE", "USDT"));
        assert_eq!(alice_balance, Decimal::new(100, 0));
    }

    #[test]
    fn test_transfer_balance_check() {
        let mut state = RiskState::new();

        // Alice deposits 100
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Alice tries to transfer 150 to Bob
        let transfer = JournalEntry {
            sequence: 2,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Transfer,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(150)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(150)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&transfer);
        assert!(!violations.is_empty());
        assert!(violations[0].0.contains("ALICE"));
    }

    #[test]
    fn test_valid_transfer() {
        let mut state = RiskState::new();

        // Alice deposits 100
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Alice transfers 50 to Bob
        let transfer = JournalEntry {
            sequence: 2,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Transfer,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(50)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(50)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&transfer);
        assert!(violations.is_empty());
    }

    // === Phase 2: Trade tests ===

    fn deposit_btc_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("BTC"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "BTC"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_trade_balance_check_both_users() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT
        state.apply_entry(&deposit_entry("ALICE", 100));
        // Bob deposits 1 BTC
        state.apply_entry(&deposit_btc_entry("BOB", 1));

        // Trade: Alice sells 100 USDT for 1 BTC
        let trade = JournalEntry {
            sequence: 3,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Trade,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                // USDT leg
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                // BTC leg
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&trade);
        assert!(violations.is_empty(), "Valid trade should pass risk check");
    }

    #[test]
    fn test_trade_insufficient_balance_seller() {
        let mut state = RiskState::new();

        // Alice only has 50 USDT
        state.apply_entry(&deposit_entry("ALICE", 50));
        // Bob deposits 1 BTC
        state.apply_entry(&deposit_btc_entry("BOB", 1));

        // Trade: Alice tries to sell 100 USDT (insufficient)
        let trade = JournalEntry {
            sequence: 3,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Trade,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&trade);
        assert!(!violations.is_empty(), "Trade should fail - Alice has insufficient USDT");
        assert!(violations.iter().any(|(acc, _)| acc.contains("ALICE")));
    }

    // === Phase 3: Margin Trading tests ===

    fn borrow_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Borrow,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                // ASSET:LOAN increases (BiBank's receivable)
                Posting::debit(RiskState::loan_account(user, "USDT"), amount(val)),
                // LIAB:AVAILABLE increases (User's balance)
                Posting::credit(AccountKey::user_available(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    fn repay_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Repay,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                // LIAB:AVAILABLE decreases (User pays)
                Posting::debit(AccountKey::user_available(user, "USDT"), amount(val)),
                // ASSET:LOAN decreases (BiBank's receivable)
                Posting::credit(RiskState::loan_account(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_borrow_updates_loan_balance() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT (equity)
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Alice borrows 500 USDT (5x leverage, within 10x limit)
        state.apply_entry(&borrow_entry("ALICE", 500));

        // Check balances
        let available = state.get_available_balance("ALICE", "USDT");
        let loan = state.get_loan_balance("ALICE", "USDT");
        let equity = state.get_equity("ALICE", "USDT");

        assert_eq!(available, Decimal::new(600, 0), "Available = 100 + 500");
        assert_eq!(loan, Decimal::new(500, 0), "Loan = 500");
        assert_eq!(equity, Decimal::new(100, 0), "Equity = Available - Loan = 100");
    }

    #[test]
    fn test_repay_reduces_loan_balance() {
        let mut state = RiskState::new();

        // Setup: Alice deposits 100, borrows 500
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 500));

        // Alice repays 200
        state.apply_entry(&repay_entry("ALICE", 200));

        let available = state.get_available_balance("ALICE", "USDT");
        let loan = state.get_loan_balance("ALICE", "USDT");

        assert_eq!(available, Decimal::new(400, 0), "Available = 600 - 200");
        assert_eq!(loan, Decimal::new(300, 0), "Loan = 500 - 200");
    }

    #[test]
    fn test_margin_ratio_calculation() {
        let mut state = RiskState::new();

        // Alice deposits 100, borrows 400
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 400));

        // Available = 500, Loan = 400
        // Margin Ratio = Available / Loan = 500/400 = 1.25
        let ratio = state.get_margin_ratio("ALICE", "USDT");
        assert_eq!(ratio, Decimal::new(125, 2));
    }

    #[test]
    fn test_margin_ratio_no_loan() {
        let mut state = RiskState::new();

        // Alice only deposits, no loan
        state.apply_entry(&deposit_entry("ALICE", 100));

        // No loan = infinite margin (represented as 100.0)
        let ratio = state.get_margin_ratio("ALICE", "USDT");
        assert_eq!(ratio, Decimal::from(100));
    }

    #[test]
    fn test_borrow_allowed_within_limit() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT (equity)
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Max borrow at 10x leverage = 100 / 0.10 = 1000
        // Try to borrow 900 (within limit)
        let result = state.check_borrow_allowed("ALICE", "USDT", Decimal::new(900, 0));
        assert!(result.is_ok(), "Should allow borrow within limit");
    }

    #[test]
    fn test_borrow_rejected_exceeds_leverage() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT (equity)
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Max borrow = 100 / 0.10 = 1000
        // Try to borrow 1100 (exceeds limit)
        let result = state.check_borrow_allowed("ALICE", "USDT", Decimal::new(1100, 0));
        assert!(matches!(result, Err(MarginError::ExceedsMaxLeverage { .. })));
    }

    #[test]
    fn test_is_liquidatable() {
        let mut state = RiskState::new();

        // Alice deposits 100, borrows 900
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 900));

        // Available = 1000, Loan = 900
        // Margin Ratio = 1000/900 = 1.11 > 1.0 (safe)
        assert!(!state.is_liquidatable("ALICE", "USDT"));

        // Simulate loss: Alice loses 200 (Available drops to 800)
        // We can simulate by doing a transfer out
        let loss_entry = JournalEntry {
            sequence: 99,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Transfer,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(200)),
                Posting::credit(AccountKey::user_available("SYSTEM", "USDT"), amount(200)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };
        state.apply_entry(&loss_entry);

        // Now: Available = 800, Loan = 900
        // Margin Ratio = 800/900 = 0.889 < 1.0 (liquidatable!)
        assert!(state.is_liquidatable("ALICE", "USDT"));
    }
}

```

## File ./bibank\crates\rpc\src\commands.rs:
```rust
//! CLI commands

use bibank_core::Amount;
use bibank_ledger::{AccountCategory, AccountKey, JournalEntryBuilder, TransactionIntent, validate_intent};
use bibank_matching::{MatchingEngine, Order, OrderSide, TradingPair};
use rust_decimal::Decimal;
use serde_json::json;

use crate::context::AppContext;

/// Initialize the system with Genesis entry
pub async fn init(ctx: &mut AppContext, correlation_id: &str) -> Result<(), anyhow::Error> {
    if ctx.is_initialized() {
        anyhow::bail!("System already initialized (sequence = {})", ctx.last_sequence());
    }

    // Create Genesis entry with initial system capital
    let initial_capital = Amount::new(Decimal::new(1_000_000_000, 0))?; // 1 billion units

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id(correlation_id)
        .debit(AccountKey::system_vault("USDT"), initial_capital)
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            initial_capital,
        )
        .build_unsigned()?;

    ctx.commit(entry).await?;

    println!("✅ System initialized with Genesis entry");
    Ok(())
}

/// Deposit funds to a user
pub async fn deposit(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id(correlation_id)
        .debit(AccountKey::system_vault(asset), amount)
        .credit(AccountKey::user_available(user_id, asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Deposited {} {} to {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Transfer funds between users
pub async fn transfer(
    ctx: &mut AppContext,
    from_user: &str,
    to_user: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Transfer)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(from_user, asset), amount)
        .credit(AccountKey::user_available(to_user, asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Transferred {} {} from {} to {} (seq: {})",
        amount, asset, from_user, to_user, committed.sequence
    );
    Ok(())
}

/// Withdraw funds from a user
pub async fn withdraw(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Withdrawal)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(user_id, asset), amount)
        .credit(AccountKey::system_vault(asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Withdrew {} {} from {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Get balance for a user
pub async fn balance(ctx: &AppContext, user_id: &str) -> Result<(), anyhow::Error> {
    let account = AccountKey::user_available(user_id, "USDT");
    let balance = ctx.risk.state().get_balance(&account);

    println!("Balance for {}: {} USDT", user_id, balance);

    // Check other common assets
    for asset in ["BTC", "ETH", "USD"] {
        let account = AccountKey::user_available(user_id, asset);
        let bal = ctx.risk.state().get_balance(&account);
        if !bal.is_zero() {
            println!("              {} {}", bal, asset);
        }
    }

    Ok(())
}

// === Phase 2: Trade and Fee commands ===

/// Execute a trade between two users
///
/// Alice sells `sell_amount` of `sell_asset` and buys `buy_amount` of `buy_asset` from Bob.
pub async fn trade(
    ctx: &mut AppContext,
    maker: &str,        // Alice - the one selling
    taker: &str,        // Bob - the one buying
    sell_amount: Decimal,
    sell_asset: &str,
    buy_amount: Decimal,
    buy_asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let sell_amt = Amount::new(sell_amount)?;
    let buy_amt = Amount::new(buy_amount)?;

    // Calculate price for metadata
    let price = sell_amount / buy_amount;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Trade)
        .correlation_id(correlation_id)
        // Sell leg: Maker pays sell_asset, Taker receives
        .debit(AccountKey::user_available(maker, sell_asset), sell_amt)
        .credit(AccountKey::user_available(taker, sell_asset), sell_amt)
        // Buy leg: Taker pays buy_asset, Maker receives
        .debit(AccountKey::user_available(taker, buy_asset), buy_amt)
        .credit(AccountKey::user_available(maker, buy_asset), buy_amt)
        // Metadata
        .metadata("trade_id", json!(correlation_id))
        .metadata("base_asset", json!(buy_asset))
        .metadata("quote_asset", json!(sell_asset))
        .metadata("price", json!(price.to_string()))
        .metadata("base_amount", json!(buy_amount.to_string()))
        .metadata("quote_amount", json!(sell_amount.to_string()))
        .metadata("maker", json!(maker))
        .metadata("taker", json!(taker))
        .build_unsigned()?;

    // Validate trade-specific rules
    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Trade executed: {} sells {} {} for {} {} from {} (seq: {})",
        maker, sell_amount, sell_asset, buy_amount, buy_asset, taker, committed.sequence
    );
    Ok(())
}

/// Charge a fee from a user
pub async fn fee(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    fee_type: &str,  // "trading", "withdrawal", etc.
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    // Fee account: REV:SYSTEM:FEE:<ASSET>:<FEE_TYPE>
    let fee_account = AccountKey::new(
        AccountCategory::Revenue,
        "SYSTEM",
        "FEE",
        asset,
        fee_type.to_uppercase(),
    );

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Fee)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(user_id, asset), amount)
        .credit(fee_account, amount)
        .metadata("fee_amount", json!(amount.to_string()))
        .metadata("fee_asset", json!(asset))
        .metadata("fee_type", json!(fee_type))
        .build_unsigned()?;

    // Validate fee-specific rules
    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Fee charged: {} {} {} from {} (seq: {})",
        amount, asset, fee_type, user_id, committed.sequence
    );
    Ok(())
}

/// Execute a trade with fee (atomic: Trade + Fee entries)
pub async fn trade_with_fee(
    ctx: &mut AppContext,
    maker: &str,
    taker: &str,
    sell_amount: Decimal,
    sell_asset: &str,
    buy_amount: Decimal,
    buy_asset: &str,
    fee_amount: Decimal,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    // First execute the trade
    trade(
        ctx, maker, taker,
        sell_amount, sell_asset,
        buy_amount, buy_asset,
        correlation_id,
    ).await?;

    // Then charge the fee (separate entry, atomic in business sense)
    let fee_correlation = format!("{}-fee", correlation_id);
    fee(ctx, maker, fee_amount, sell_asset, "trading", &fee_correlation).await?;

    Ok(())
}

// === Phase 2.1: Trade History ===

/// List trade history
pub async fn trades(
    ctx: &AppContext,
    user: Option<&str>,
    pair: Option<(&str, &str)>,
    limit: u32,
) -> Result<(), anyhow::Error> {
    let Some(ref projection) = ctx.projection else {
        anyhow::bail!("Projection not available");
    };

    let trades = if let Some(user_id) = user {
        projection.trade.get_user_trades(user_id).await?
    } else if let Some((base, quote)) = pair {
        projection.trade.get_pair_trades(base, quote).await?
    } else {
        projection.trade.get_recent_trades(limit).await?
    };

    if trades.is_empty() {
        println!("No trades found");
        return Ok(());
    }

    println!("Trade History ({} trades):", trades.len());
    println!("{:-<80}", "");
    println!(
        "{:>6} | {:>8} | {:>8} | {:>12} {:>6} | {:>12} {:>6}",
        "ID", "Seller", "Buyer", "Sold", "Asset", "Bought", "Asset"
    );
    println!("{:-<80}", "");

    for trade in trades.iter().take(limit as usize) {
        println!(
            "{:>6} | {:>8} | {:>8} | {:>12} {:>6} | {:>12} {:>6}",
            trade.trade_id,
            trade.seller,
            trade.buyer,
            trade.sell_amount,
            trade.sell_asset,
            trade.buy_amount,
            trade.buy_asset,
        );
    }

    Ok(())
}

// === Phase 3: Margin Trading Commands ===

/// Borrow funds (margin trading)
///
/// Creates a loan by crediting user's available balance from system loan pool.
pub async fn borrow(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    // LIAB:USER:<USER_ID>:<ASSET>:LOAN - tracks user's loan obligation
    let loan_account = AccountKey::new(
        AccountCategory::Liability,
        "USER",
        user_id,
        asset,
        "LOAN",
    );

    // ASSET:SYSTEM:LENDING_POOL:<ASSET>:MAIN - system lending pool
    let lending_pool = AccountKey::new(
        AccountCategory::Asset,
        "SYSTEM",
        "LENDING_POOL",
        asset,
        "MAIN",
    );

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Borrow)
        .correlation_id(correlation_id)
        // Debit lending pool (reduce system's lending funds)
        .debit(lending_pool, amount)
        // Credit loan liability (increase user's loan obligation)
        .credit(loan_account, amount)
        // Credit user's available balance
        .credit(AccountKey::user_available(user_id, asset), amount)
        // Debit a receivable (to maintain balance)
        .debit(
            AccountKey::new(AccountCategory::Asset, "SYSTEM", "LOANS_RECEIVABLE", asset, "MAIN"),
            amount,
        )
        .metadata("loan_amount", json!(amount.to_string()))
        .metadata("loan_asset", json!(asset))
        .metadata("borrower", json!(user_id))
        .build_unsigned()?;

    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Borrowed {} {} for {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Repay borrowed funds
pub async fn repay(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let loan_account = AccountKey::new(
        AccountCategory::Liability,
        "USER",
        user_id,
        asset,
        "LOAN",
    );

    let lending_pool = AccountKey::new(
        AccountCategory::Asset,
        "SYSTEM",
        "LENDING_POOL",
        asset,
        "MAIN",
    );

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Repay)
        .correlation_id(correlation_id)
        // Debit user's available balance (reduce funds)
        .debit(AccountKey::user_available(user_id, asset), amount)
        // Credit lending pool (return to system)
        .credit(lending_pool, amount)
        // Debit loan liability (reduce user's loan obligation)
        .debit(loan_account, amount)
        // Credit receivable (reduce system's receivable)
        .credit(
            AccountKey::new(AccountCategory::Asset, "SYSTEM", "LOANS_RECEIVABLE", asset, "MAIN"),
            amount,
        )
        .metadata("repay_amount", json!(amount.to_string()))
        .metadata("repay_asset", json!(asset))
        .metadata("borrower", json!(user_id))
        .build_unsigned()?;

    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Repaid {} {} for {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Place a limit order
///
/// Locks collateral and submits order to matching engine.
pub async fn place_order(
    ctx: &mut AppContext,
    user_id: &str,
    side: &str,
    base: &str,
    quote: &str,
    price: Decimal,
    quantity: Decimal,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let order_side = match side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => anyhow::bail!("Invalid order side: {}. Use 'buy' or 'sell'", side),
    };

    let pair = TradingPair::new(base, quote);

    // Calculate lock amount based on side
    let (lock_asset, lock_amount) = match order_side {
        OrderSide::Buy => (quote.to_uppercase(), price * quantity), // Lock quote (USDT)
        OrderSide::Sell => (base.to_uppercase(), quantity),          // Lock base (BTC)
    };

    let lock_amt = Amount::new(lock_amount)?;

    // Create order (get order ID)
    let order = Order::new(user_id, pair.clone(), order_side, price, quantity);
    let order_id = order.id.clone();

    // Create journal entry to lock collateral
    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::OrderPlace)
        .correlation_id(correlation_id)
        // Debit from available balance
        .debit(AccountKey::user_available(user_id, &lock_asset), lock_amt)
        // Credit to locked balance
        .credit(AccountKey::user_locked(user_id, &lock_asset), lock_amt)
        .metadata("order_id", json!(order_id))
        .metadata("order_side", json!(side.to_lowercase()))
        .metadata("base_asset", json!(base.to_uppercase()))
        .metadata("quote_asset", json!(quote.to_uppercase()))
        .metadata("price", json!(price.to_string()))
        .metadata("quantity", json!(quantity.to_string()))
        .metadata("lock_asset", json!(lock_asset))
        .metadata("lock_amount", json!(lock_amount.to_string()))
        .build_unsigned()?;

    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    // TODO: Submit order to matching engine (ctx.matching_engine)
    // For now, just print success

    println!(
        "✅ Order placed: {} {} {} @ {} {} (order_id: {}, seq: {})",
        side.to_uppercase(),
        quantity,
        base.to_uppercase(),
        price,
        quote.to_uppercase(),
        order_id,
        committed.sequence
    );
    Ok(())
}

/// Cancel an open order
pub async fn cancel_order(
    ctx: &mut AppContext,
    order_id: &str,
    base: &str,
    quote: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    // For now, we need to know the lock details from the order
    // In a real implementation, we'd look this up from the matching engine

    // TODO: Look up order from matching engine
    // let order = ctx.matching_engine.get_order(&pair, order_id)?;

    // For demonstration, we'll require the user to provide unlock info
    // In reality, this would be looked up from projection/matching engine

    println!("⚠️  Order cancellation requires order lookup (not yet implemented)");
    println!("   Order ID: {}", order_id);
    println!("   Pair: {}/{}", base.to_uppercase(), quote.to_uppercase());

    Ok(())
}

/// Show margin status for a user
pub async fn margin_status(ctx: &AppContext, user_id: &str) -> Result<(), anyhow::Error> {
    let state = ctx.risk.state();

    println!("Margin Status for {}", user_id);
    println!("{:-<50}", "");

    // Show balances
    println!("\n📊 Balances:");
    for asset in ["USDT", "BTC", "ETH"] {
        let available = state.get_balance(&AccountKey::user_available(user_id, asset));
        let locked = state.get_balance(&AccountKey::user_locked(user_id, asset));

        if !available.is_zero() || !locked.is_zero() {
            println!(
                "   {}: available={}, locked={}",
                asset, available, locked
            );
        }
    }

    // Show loans
    println!("\n💰 Loans:");
    let mut has_loans = false;
    for asset in ["USDT", "BTC", "ETH"] {
        let loan_account = AccountKey::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOAN",
        );
        let loan = state.get_balance(&loan_account);

        if !loan.is_zero() {
            println!("   {}: {}", asset, loan);
            has_loans = true;
        }
    }

    if !has_loans {
        println!("   No active loans");
    }

    // Show margin ratio (simplified)
    let usdt_balance = state.get_balance(&AccountKey::user_available(user_id, "USDT"));
    let usdt_loan = state.get_balance(&AccountKey::new(
        AccountCategory::Liability,
        "USER",
        user_id,
        "USDT",
        "LOAN",
    ));

    if !usdt_loan.is_zero() {
        let margin_ratio = (usdt_balance / usdt_loan) * Decimal::from(100);
        println!("\n📈 Margin Ratio (USDT): {:.2}%", margin_ratio);

        if margin_ratio < Decimal::from(120) {
            println!("   ⚠️  WARNING: Below maintenance margin (120%)");
        } else if margin_ratio < Decimal::from(150) {
            println!("   ⚠️  CAUTION: Approaching maintenance margin");
        } else {
            println!("   ✅ Healthy margin level");
        }
    }

    Ok(())
}

/// Show order book depth
pub async fn order_book(
    ctx: &AppContext,
    base: &str,
    quote: &str,
    depth: usize,
) -> Result<(), anyhow::Error> {
    // TODO: Get from ctx.matching_engine when integrated
    println!("Order Book: {}/{}", base.to_uppercase(), quote.to_uppercase());
    println!("{:-<60}", "");
    println!("(Order book not yet integrated with context)");
    println!("\nTo see order book, matching engine needs to be integrated.");

    Ok(())
}

```

## File ./bibank\crates\rpc\src\context.rs:
```rust
//! Application context - wires everything together

use bibank_bus::EventBus;
use bibank_events::{EventReader, EventStore};
use bibank_ledger::{hash::calculate_entry_hash, JournalEntry, Signer, SystemSigner, UnsignedEntry};
use bibank_projection::ProjectionEngine;
use bibank_risk::{RiskEngine, RiskError};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Application context - wires together all components
pub struct AppContext {
    pub risk: RiskEngine,
    pub event_store: EventStore,
    pub bus: EventBus,
    pub projection: Option<ProjectionEngine>,
    pub signer: Option<Arc<dyn Signer>>,
    journal_path: PathBuf,
    projection_path: PathBuf,
    last_sequence: u64,
    last_hash: String,
}

impl AppContext {
    /// Create a new application context
    pub async fn new(data_path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let data_path = data_path.as_ref();
        let journal_path = data_path.join("journal");
        let projection_path = data_path.join("projection.db");

        // Create directories
        std::fs::create_dir_all(&journal_path)?;

        // Initialize components
        let event_store = EventStore::new(&journal_path)?;
        let bus = EventBus::new(&journal_path);
        let mut risk = RiskEngine::new();

        // Replay events to rebuild state
        let reader = EventReader::from_directory(&journal_path)?;
        let entries = reader.read_all()?;

        let (last_sequence, last_hash) = if let Some(last) = entries.last() {
            (last.sequence, last.hash.clone())
        } else {
            (0, "GENESIS".to_string())
        };

        // Rebuild risk state from events
        risk.replay(entries.iter());

        // Initialize projection
        let projection = ProjectionEngine::new(&projection_path).await.ok();

        // Replay projection if available
        if let Some(ref proj) = projection {
            proj.replay(&bus).await.ok();
        }

        // Initialize system signer from env var (Phase 2)
        let signer: Option<Arc<dyn Signer>> = std::env::var("BIBANK_SYSTEM_KEY")
            .ok()
            .and_then(|key| SystemSigner::from_hex(&key).ok())
            .map(|s| Arc::new(s) as Arc<dyn Signer>);

        Ok(Self {
            risk,
            event_store,
            bus,
            projection,
            signer,
            journal_path,
            projection_path,
            last_sequence,
            last_hash,
        })
    }

    /// Commit an unsigned entry
    ///
    /// Flow: Risk Check → Sign → Append → Apply
    pub async fn commit(&mut self, unsigned: UnsignedEntry) -> Result<JournalEntry, CommitError> {
        // 1. Validate double-entry balance
        unsigned.validate_balance().map_err(CommitError::Ledger)?;

        // 2. Risk check (pre-commit gatekeeper)
        self.risk.check(&unsigned).map_err(CommitError::Risk)?;

        // 3. Sign the entry (add sequence, prev_hash, hash, timestamp)
        let sequence = self.last_sequence + 1;
        let prev_hash = self.last_hash.clone();
        let timestamp = Utc::now();

        let mut entry = JournalEntry {
            sequence,
            prev_hash,
            hash: String::new(),
            timestamp,
            intent: unsigned.intent,
            correlation_id: unsigned.correlation_id,
            causality_id: unsigned.causality_id,
            postings: unsigned.postings,
            metadata: unsigned.metadata,
            signatures: Vec::new(), // Phase 2: Will be signed after hash calculation
        };

        entry.hash = calculate_entry_hash(&entry);

        // 4. Sign the entry with system key (Phase 2)
        if let Some(ref signer) = self.signer {
            let signature = signer.sign(&entry);
            entry.signatures.push(signature);
        }

        // 5. Validate the signed entry
        entry.validate().map_err(CommitError::Ledger)?;

        // 6. Append to event store (Source of Truth)
        self.event_store
            .append(&entry)
            .map_err(CommitError::Event)?;

        // 7. Update risk state
        self.risk.apply(&entry);

        // 8. Update projection (if available)
        if let Some(ref projection) = self.projection {
            projection.apply(&entry).await.ok();
        }

        // 9. Update last sequence/hash
        self.last_sequence = entry.sequence;
        self.last_hash = entry.hash.clone();

        Ok(entry)
    }

    /// Get journal path
    pub fn journal_path(&self) -> &Path {
        &self.journal_path
    }

    /// Get projection path
    pub fn projection_path(&self) -> &Path {
        &self.projection_path
    }

    /// Check if system is initialized (has Genesis entry)
    pub fn is_initialized(&self) -> bool {
        self.last_sequence > 0
    }

    /// Get last sequence number
    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
}

/// Errors during commit
#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error("Ledger error: {0}")]
    Ledger(#[from] bibank_ledger::LedgerError),

    #[error("Risk error: {0}")]
    Risk(RiskError),

    #[error("Event store error: {0}")]
    Event(#[from] bibank_events::EventError),
}

```

## File ./bibank\crates\rpc\src\lib.rs:
```rust
//! BiBank RPC - API/CLI orchestrator
//!
//! This crate provides the CLI binary and command orchestration.

pub mod commands;
pub mod context;

pub use context::AppContext;

```

## File ./bibank\crates\rpc\src\main.rs:
```rust
//! BiBank CLI - Main entry point

use bibank_rpc::{commands, AppContext};
use clap::{Parser, Subcommand};
use rust_decimal::Decimal;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "bibank")]
#[command(about = "BiBank - Financial State OS", long_about = None)]
struct Cli {
    /// Data directory path
    #[arg(short, long, default_value = "./data")]
    data: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the system with Genesis entry
    Init,

    /// Deposit funds to a user
    Deposit {
        /// User ID (will be uppercased)
        user: String,
        /// Amount to deposit
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Transfer funds between users
    Transfer {
        /// Source user ID
        from: String,
        /// Destination user ID
        to: String,
        /// Amount to transfer
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Withdraw funds from a user
    Withdraw {
        /// User ID
        user: String,
        /// Amount to withdraw
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Check balance for a user
    Balance {
        /// User ID
        user: String,
    },

    /// Replay events (rebuild projections)
    Replay {
        /// Drop projections before replay
        #[arg(long)]
        reset: bool,
    },

    /// Audit the ledger (verify hash chain)
    Audit {
        /// Also verify digital signatures
        #[arg(long)]
        verify_signatures: bool,
    },

    // === Phase 2: Trade and Fee ===

    /// Execute a trade between two users
    Trade {
        /// Maker user ID (seller)
        maker: String,
        /// Taker user ID (buyer)
        taker: String,
        /// Amount to sell
        #[arg(long)]
        sell: Decimal,
        /// Asset to sell
        #[arg(long)]
        sell_asset: String,
        /// Amount to buy
        #[arg(long)]
        buy: Decimal,
        /// Asset to buy
        #[arg(long)]
        buy_asset: String,
        /// Optional fee amount (charged to maker)
        #[arg(long)]
        fee: Option<Decimal>,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Charge a fee from a user
    Fee {
        /// User ID
        user: String,
        /// Fee amount
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Fee type (trading, withdrawal, etc.)
        #[arg(long, default_value = "trading")]
        fee_type: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Generate a new system key
    Keygen {
        /// Output file path
        #[arg(long, default_value = "system.key")]
        output: PathBuf,
    },

    // === Phase 2.1: Trade History ===

    /// List trade history
    Trades {
        /// Filter by user ID
        #[arg(long)]
        user: Option<String>,
        /// Filter by base asset (requires --quote)
        #[arg(long)]
        base: Option<String>,
        /// Filter by quote asset (requires --base)
        #[arg(long)]
        quote: Option<String>,
        /// Maximum number of trades to show
        #[arg(long, default_value = "20")]
        limit: u32,
    },

    // === Phase 3: Margin Trading ===

    /// Borrow funds (margin trading)
    Borrow {
        /// User ID
        user: String,
        /// Amount to borrow
        amount: Decimal,
        /// Asset to borrow
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Repay borrowed funds
    Repay {
        /// User ID
        user: String,
        /// Amount to repay
        amount: Decimal,
        /// Asset to repay
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Place a limit order
    PlaceOrder {
        /// User ID
        user: String,
        /// Order side: buy or sell
        side: String,
        /// Base asset (e.g., BTC)
        base: String,
        /// Quote asset (e.g., USDT)
        quote: String,
        /// Limit price
        price: Decimal,
        /// Quantity (in base asset)
        quantity: Decimal,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Cancel an open order
    CancelOrder {
        /// Order ID to cancel
        order_id: String,
        /// Base asset of the trading pair
        base: String,
        /// Quote asset of the trading pair
        quote: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Check margin status for a user
    MarginStatus {
        /// User ID
        user: String,
    },

    /// Show order book depth
    OrderBook {
        /// Base asset (e.g., BTC)
        base: String,
        /// Quote asset (e.g., USDT)
        quote: String,
        /// Number of price levels to show
        #[arg(long, default_value = "10")]
        depth: usize,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Create application context
    let mut ctx = AppContext::new(&cli.data).await?;

    match cli.command {
        Commands::Init => {
            let correlation_id = Uuid::new_v4().to_string();
            commands::init(&mut ctx, &correlation_id).await?;
        }

        Commands::Deposit {
            user,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::deposit(&mut ctx, &user, amount, &asset, &correlation_id).await?;
        }

        Commands::Transfer {
            from,
            to,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::transfer(&mut ctx, &from, &to, amount, &asset, &correlation_id).await?;
        }

        Commands::Withdraw {
            user,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::withdraw(&mut ctx, &user, amount, &asset, &correlation_id).await?;
        }

        Commands::Balance { user } => {
            commands::balance(&ctx, &user).await?;
        }

        Commands::Replay { reset } => {
            let projection_path = ctx.projection_path().to_path_buf();
            let data_path = cli.data.clone();

            // Drop existing context to release SQLite connection
            drop(ctx);

            if reset {
                println!("🗑️  Dropping projections...");
                if projection_path.exists() {
                    std::fs::remove_file(&projection_path)?;
                    println!("   Deleted {}", projection_path.display());
                }
            }

            // Recreate context to replay
            let new_ctx = AppContext::new(&data_path).await?;
            println!(
                "✅ Replayed {} entries",
                new_ctx.last_sequence()
            );
        }

        Commands::Audit { verify_signatures } => {
            use bibank_events::EventReader;
            use bibank_ledger::hash::verify_chain;

            let reader = EventReader::from_directory(ctx.journal_path())?;
            let entries = reader.read_all()?;

            // Verify hash chain
            match verify_chain(&entries) {
                Ok(()) => {
                    println!("✅ Hash chain verified ({} entries)", entries.len());
                }
                Err(e) => {
                    println!("❌ Hash chain broken: {}", e);
                    return Ok(());
                }
            }

            // Verify signatures if requested
            if verify_signatures {
                let mut signed_count = 0;
                let mut unsigned_count = 0;

                for entry in &entries {
                    if entry.signatures.is_empty() {
                        unsigned_count += 1;
                    } else {
                        match entry.verify_signatures() {
                            Ok(()) => signed_count += 1,
                            Err(e) => {
                                println!("❌ Signature verification failed at seq {}: {}", entry.sequence, e);
                                return Ok(());
                            }
                        }
                    }
                }

                println!("✅ Signatures verified: {} signed, {} unsigned (Phase 1)", signed_count, unsigned_count);
            }
        }

        Commands::Trade {
            maker,
            taker,
            sell,
            sell_asset,
            buy,
            buy_asset,
            fee,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());

            if let Some(fee_amount) = fee {
                commands::trade_with_fee(
                    &mut ctx, &maker, &taker,
                    sell, &sell_asset,
                    buy, &buy_asset,
                    fee_amount,
                    &correlation_id,
                ).await?;
            } else {
                commands::trade(
                    &mut ctx, &maker, &taker,
                    sell, &sell_asset,
                    buy, &buy_asset,
                    &correlation_id,
                ).await?;
            }
        }

        Commands::Fee {
            user,
            amount,
            asset,
            fee_type,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::fee(&mut ctx, &user, amount, &asset, &fee_type, &correlation_id).await?;
        }

        Commands::Keygen { output } => {
            use bibank_ledger::{Signer, SystemSigner};

            let signer = SystemSigner::generate();
            let seed = signer.seed_hex();
            let pubkey = signer.public_key_hex();

            std::fs::write(&output, &seed)?;
            println!("✅ Generated system key");
            println!("   Private key saved to: {}", output.display());
            println!("   Public key: {}", pubkey);
            println!("");
            println!("To use: export BIBANK_SYSTEM_KEY={}", seed);
        }

        Commands::Trades {
            user,
            base,
            quote,
            limit,
        } => {
            let pair = match (&base, &quote) {
                (Some(b), Some(q)) => Some((b.as_str(), q.as_str())),
                _ => None,
            };
            commands::trades(&ctx, user.as_deref(), pair, limit).await?;
        }

        // === Phase 3: Margin Trading ===

        Commands::Borrow {
            user,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::borrow(&mut ctx, &user, amount, &asset, &correlation_id).await?;
        }

        Commands::Repay {
            user,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::repay(&mut ctx, &user, amount, &asset, &correlation_id).await?;
        }

        Commands::PlaceOrder {
            user,
            side,
            base,
            quote,
            price,
            quantity,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::place_order(
                &mut ctx,
                &user,
                &side,
                &base,
                &quote,
                price,
                quantity,
                &correlation_id,
            ).await?;
        }

        Commands::CancelOrder {
            order_id,
            base,
            quote,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::cancel_order(&mut ctx, &order_id, &base, &quote, &correlation_id).await?;
        }

        Commands::MarginStatus { user } => {
            commands::margin_status(&ctx, &user).await?;
        }

        Commands::OrderBook { base, quote, depth } => {
            commands::order_book(&ctx, &base, &quote, depth).await?;
        }
    }

    Ok(())
}

```

## Cargo.toml dependencies:
```toml
members = [
resolver = "2"
version = "0.1.0"
edition = "2021"
authors = ["BiBank Team"]
license = "MIT"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rust_decimal = { version = "1.33", features = ["serde-with-str", "maths"] }
rust_decimal_macros = "1.33"
thiserror = "2.0"
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
hex = "0.4"
ed25519-dalek = { version = "2.1", features = ["rand_core"] }
rand = "0.8"
async-trait = "0.1"
strum = { version = "0.26", features = ["derive"] }
strum_macros = "0.26"
tokio = { version = "1.36", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1.7", features = ["serde", "v4"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono"] }
clap = { version = "4.4", features = ["derive"] }
tempfile = "3.24"
bibank-core = { path = "./crates/core" }
bibank-ledger = { path = "./crates/ledger" }
bibank-risk = { path = "./crates/risk" }
bibank-events = { path = "./crates/events" }
bibank-bus = { path = "./crates/bus" }
bibank-projection = { path = "./crates/projection" }
bibank-rpc = { path = "./crates/rpc" }
bibank-matching = { path = "./crates/matching" }
bibank-oracle = { path = "./crates/oracle" }
bibank-dsl = { path = "./crates/dsl" }
bibank-approval = { path = "./crates/approval" }
bibank-compliance = { path = "./crates/compliance" }
bibank-hooks = { path = "./crates/hooks" }
```

