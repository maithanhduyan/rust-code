//! BiBank Core - Domain types
//!
//! This crate contains the fundamental types used across BiBank:
//! - `Amount`: Non-negative decimal wrapper for financial amounts
//! - `Currency`: Type-safe currency/asset codes

pub mod amount;
pub mod currency;

pub use amount::Amount;
pub use currency::Currency;
