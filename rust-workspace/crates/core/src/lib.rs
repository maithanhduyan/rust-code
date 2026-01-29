//! Core library - Business logic chính của ứng dụng
//!
//! Crate này chứa các models, services và business logic.

pub mod error;
pub mod models;
pub mod services;

pub use error::{CoreError, Result};
pub use models::*;
pub use services::*;
