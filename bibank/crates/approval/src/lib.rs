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
