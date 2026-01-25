//! # Simbank Core
//!
//! Thư viện chứa các domain types cốt lõi của Simbank.
//!
//! ## Modules
//! - `money`: Currency và Money với rust_decimal
//! - `wallet`: WalletType, Wallet, Balance
//! - `person`: PersonType, Person
//! - `account`: Account
//! - `event`: Event, EventType, EventMetadata, AmlFlag
//! - `error`: Domain errors

pub mod money;
pub mod wallet;
pub mod person;
pub mod account;
pub mod event;
pub mod error;

// Re-export commonly used types
pub use money::{Currency, Money};
pub use wallet::{WalletType, Wallet, Balance};
pub use person::{PersonType, Person};
pub use account::Account;
pub use event::{Event, EventType, EventMetadata, AmlFlag};
pub use error::CoreError;
