//! BiBank Risk Engine - Pre-commit gatekeeper
//!
//! The Risk Engine validates transactions BEFORE they are committed to the ledger.
//! It maintains in-memory state rebuilt from event replay on startup.

pub mod engine;
pub mod error;
pub mod state;

pub use engine::RiskEngine;
pub use error::RiskError;
pub use state::RiskState;
