//! BiBank DSL - Domain Specific Language for Compliance Rules
//!
//! Phase 4: Declarative rule definition using macros
//!
//! # Overview
//!
//! This crate provides a DSL for defining AML/Compliance rules declaratively:
//!
//! ```ignore
//! use bibank_dsl::*;
//!
//! // Define individual rules
//! let sanctions_rule = rule! {
//!     id: "SANCTIONS_CHECK",
//!     name: "Sanctions Watchlist Check",
//!     type: block,
//!     when: is_watchlisted!(),
//!     then: RuleAction::block("SANCTIONS", "User on sanctions watchlist"),
//!     priority: 10,
//! };
//!
//! let large_tx_rule = rule! {
//!     id: "LARGE_TX_ALERT",
//!     type: flag,
//!     when: amount_gte!(dec!(10000)),
//!     then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large transaction"),
//! };
//!
//! // Group rules into a rule set
//! let ruleset = rule_set! {
//!     name: "AML_BASIC",
//!     description: "Basic AML compliance rules",
//!     rules: [sanctions_rule, large_tx_rule],
//! };
//!
//! // Evaluate against transaction context
//! let result = RuleEvaluator::eval_ruleset(&ruleset, &ctx);
//! ```
//!
//! # Rule Types
//!
//! - **BLOCK rules**: Pre-validation hooks that reject transactions immediately
//! - **FLAG rules**: Post-commit hooks that flag transactions for review
//!
//! # Conditions
//!
//! Built-in conditions:
//! - `amount_gte!(value)` - Amount >= threshold
//! - `amount_lt!(value)` - Amount < threshold
//! - `account_age_lt!(days)` - Account younger than N days
//! - `is_watchlisted!()` - User on sanctions watchlist
//! - `is_pep!()` - User is Politically Exposed Person
//! - `all_of!(cond1, cond2)` - All conditions must match (AND)
//! - `any_of!(cond1, cond2)` - Any condition must match (OR)
//!
//! # Actions
//!
//! - `RuleAction::block(code, reason)` - Block transaction
//! - `RuleAction::flag(risk, level, reason)` - Flag for review
//! - `RuleAction::Approve` - Approve (no action)

pub mod evaluator;
pub mod macros;
pub mod types;

// Re-export commonly used types
pub use bibank_compliance::{AmlDecision, ApprovalLevel, RiskScore};
pub use evaluator::{RuleEvalResult, RuleEvaluator, RuleSetEvalResult};
pub use types::{Condition, RuleAction, RuleBuilder, RuleDefinition, RuleSet, RuleType};
