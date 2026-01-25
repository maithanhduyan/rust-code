//! BiBank Projection - Event to SQLite views
//!
//! Projections are DISPOSABLE - they can be rebuilt from events at any time.

pub mod balance;
pub mod engine;
pub mod error;

pub use balance::BalanceProjection;
pub use engine::ProjectionEngine;
pub use error::ProjectionError;
