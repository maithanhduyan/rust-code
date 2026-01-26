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
