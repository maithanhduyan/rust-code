//! BiBank Ledger - Double-entry accounting core
//!
//! This is the HEART of BiBank. All financial state changes go through this crate.
//!
//! # Key Types
//! - `AccountKey`: Hierarchical account identifier (CAT:SEGMENT:ID:ASSET:SUB_ACCOUNT)
//! - `JournalEntry`: Atomic unit of financial state change
//! - `Posting`: Single debit/credit in an entry
//! - `Side`: Debit or Credit

pub mod account;
pub mod entry;
pub mod error;
pub mod hash;
pub mod signature;
pub mod validation;

pub use account::{AccountCategory, AccountKey};
pub use entry::{JournalEntry, JournalEntryBuilder, Posting, Side, TransactionIntent, UnsignedEntry};
pub use error::LedgerError;
pub use signature::{EntrySignature, SignatureAlgorithm, SignablePayload, Signer, SystemSigner};
pub use validation::validate_intent;
