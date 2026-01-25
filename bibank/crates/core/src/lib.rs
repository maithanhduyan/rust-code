//! BiBank Core - Domain types
//!
//! This crate contains the fundamental types used across BiBank:
//! - `Amount`: Non-negative decimal wrapper for financial amounts

pub mod amount;

pub use amount::Amount;
