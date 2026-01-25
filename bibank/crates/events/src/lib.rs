//! BiBank Events - JSONL event store
//!
//! This crate handles persistence of journal entries to JSONL files.
//! JSONL is the Source of Truth - SQLite projections are disposable.

pub mod error;
pub mod reader;
pub mod store;

pub use error::EventError;
pub use reader::EventReader;
pub use store::EventStore;
