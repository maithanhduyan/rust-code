//! BiBank Event Bus - In-process event distribution
//!
//! Distributes committed events to consumers (projections, etc.)
//! Phase 1: Simple synchronous distribution
//! Phase 2+: Async channels with replay capability

pub mod channel;

pub use channel::EventBus;
