//! Hook traits - interfaces for implementing hooks

use async_trait::async_trait;

use crate::context::HookContext;
use crate::error::HookResult;
use bibank_compliance::{AmlDecision, CheckResult};

/// Decision from pre-validation hook
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    /// Allow transaction to proceed
    Allow,
    /// Block transaction with reason
    Block { reason: String, code: String },
}

impl HookDecision {
    /// Create a block decision
    pub fn block(reason: impl Into<String>, code: impl Into<String>) -> Self {
        HookDecision::Block {
            reason: reason.into(),
            code: code.into(),
        }
    }

    /// Check if this is an allow decision
    pub fn is_allowed(&self) -> bool {
        matches!(self, HookDecision::Allow)
    }

    /// Check if this is a block decision
    pub fn is_blocked(&self) -> bool {
        matches!(self, HookDecision::Block { .. })
    }
}

/// Pre-validation hook - runs BEFORE ledger commit
///
/// Use for BLOCK rules that should reject transactions immediately:
/// - Sanctions/Watchlist checks
/// - KYC limit enforcement
/// - Hard policy violations
///
/// If any pre-validation hook returns Block, the transaction is rejected
/// and never enters the ledger.
#[async_trait]
pub trait PreValidationHook: Send + Sync {
    /// Hook name for logging/debugging
    fn name(&self) -> &str;

    /// Priority (lower = runs first)
    fn priority(&self) -> u32 {
        100
    }

    /// Called before transaction validation
    ///
    /// Return `Ok(HookDecision::Allow)` to proceed
    /// Return `Ok(HookDecision::Block { .. })` to reject
    /// Return `Err(_)` for hook failure (behavior depends on FailPolicy)
    async fn on_pre_validation(&self, ctx: &HookContext) -> HookResult<HookDecision>;
}

/// Post-commit hook - runs AFTER ledger commit
///
/// Use for FLAG rules that should lock funds for review:
/// - Structuring detection
/// - Velocity anomalies
/// - Risk scoring
///
/// These hooks cannot reject the transaction (already committed),
/// but can lock funds and create review requests.
#[async_trait]
pub trait PostCommitHook: Send + Sync {
    /// Hook name for logging/debugging
    fn name(&self) -> &str;

    /// Priority (lower = runs first)
    fn priority(&self) -> u32 {
        100
    }

    /// Called after transaction is committed to ledger
    ///
    /// Returns the compliance check result (may contain FLAG decision)
    async fn on_post_commit(&self, ctx: &HookContext) -> HookResult<CheckResult>;
}

/// A no-op pre-validation hook (for testing)
pub struct NoOpPreValidationHook;

#[async_trait]
impl PreValidationHook for NoOpPreValidationHook {
    fn name(&self) -> &str {
        "NoOpPreValidation"
    }

    async fn on_pre_validation(&self, _ctx: &HookContext) -> HookResult<HookDecision> {
        Ok(HookDecision::Allow)
    }
}

/// A no-op post-commit hook (for testing)
pub struct NoOpPostCommitHook;

#[async_trait]
impl PostCommitHook for NoOpPostCommitHook {
    fn name(&self) -> &str {
        "NoOpPostCommit"
    }

    async fn on_post_commit(&self, _ctx: &HookContext) -> HookResult<CheckResult> {
        Ok(CheckResult {
            decision: AmlDecision::Approved,
            rules_triggered: vec![],
            risk_score: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_hook_decision_allow() {
        let decision = HookDecision::Allow;
        assert!(decision.is_allowed());
        assert!(!decision.is_blocked());
    }

    #[test]
    fn test_hook_decision_block() {
        let decision = HookDecision::block("Sanctions match", "OFAC-001");
        assert!(!decision.is_allowed());
        assert!(decision.is_blocked());

        if let HookDecision::Block { reason, code } = decision {
            assert_eq!(reason, "Sanctions match");
            assert_eq!(code, "OFAC-001");
        }
    }

    #[tokio::test]
    async fn test_noop_pre_validation() {
        let hook = NoOpPreValidationHook;
        let ctx = HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT");

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_noop_post_commit() {
        let hook = NoOpPostCommitHook;
        let ctx = HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT");

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[test]
    fn test_hook_priority() {
        let hook = NoOpPreValidationHook;
        assert_eq!(hook.priority(), 100);
    }
}
