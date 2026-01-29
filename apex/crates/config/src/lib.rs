//! Apex Config - Configuration management
//!
//! Supports hot reload via ArcSwap.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod loader;
pub mod types;

pub use loader::ConfigLoader;
pub use types::{ApexConfig, BackendConfig, RouteConfig, ServerConfig};
