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
