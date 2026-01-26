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
