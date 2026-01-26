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
