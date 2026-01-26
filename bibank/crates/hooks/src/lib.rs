//! BiBank Hooks - Transaction Lifecycle Hooks
//!
//! Provides hook points for AML/Compliance checks at critical transaction stages:
//!
//! ```text
//! Intent Created
//!     │
//!     ▼
//! ┌─────────────────────────────┐
//! │ PRE_VALIDATION_HOOK         │ ← BLOCK rules (sanctions, KYC limits)
//! │ (reject immediately)        │
//! └─────────────────────────────┘
//!     │
//!     ▼
//! ┌─────────────────────────────┐
//! │ LEDGER COMMIT               │
//! └─────────────────────────────┘
//!     │
//!     ▼
//! ┌─────────────────────────────┐
//! │ POST_COMMIT_HOOK            │ ← FLAG rules (structuring, velocity)
//! │ (lock funds if flagged)     │
//! └─────────────────────────────┘
//! ```

pub mod aml;
pub mod context;
pub mod error;
pub mod executor;
pub mod registry;
pub mod traits;

pub use aml::{LargeTxHook, NewAccountHook, PepCheckHook, SanctionsHook};
pub use context::{HookContext, HookMetadata};
pub use error::{HookError, HookResult};
pub use executor::{ExecutionResult, ExecutorBuilder, TransactionExecutor};
pub use registry::HookRegistry;
pub use traits::{HookDecision, PostCommitHook, PreValidationHook};
