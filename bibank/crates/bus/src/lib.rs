//! BiBank Event Bus - In-process async event distribution
//!
//! Distributes committed events to subscribers (projections, etc.)
//!
//! # Phase 2 Features
//! - Async pub/sub with tokio broadcast channel
//! - EventSubscriber trait for custom handlers
//! - Replay from JSONL (Source of Truth)
//! - No retention in bus - events only in JSONL

pub mod channel;
pub mod error;
pub mod event;
pub mod subscriber;

pub use channel::EventBus;
pub use error::BusError;
pub use event::LedgerEvent;
pub use subscriber::EventSubscriber;
