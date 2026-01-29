//! Apex Core - Hot path logic for reverse proxy
//!
//! This crate contains the performance-critical code paths.
//! 
//! # Invariants
//! 
//! 1. NO Mutex/RwLock in hot path
//! 2. NO allocation per-request (except arena)
//! 3. NO panic on user input

#![warn(missing_docs)]
#![warn(clippy::all)]
#![deny(unsafe_code)]

pub mod backend;
pub mod error;
pub mod router;

pub use backend::{Backend, BackendPool};
pub use error::ProxyError;
pub use router::{Route, RouteMatch, Router};
