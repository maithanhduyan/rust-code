//! AML-specific hook implementations
//!
//! Built-in hooks for common AML checks:
//! - [`SanctionsHook`] - Pre-validation check against watchlist (BLOCK)
//! - [`PepCheckHook`] - Pre-validation check for PEPs (BLOCK)
//! - [`NewAccountHook`] - Post-commit check for new accounts (FLAG)

use std::collections::HashSet;
use std::sync::RwLock;

use async_trait::async_trait;
use rust_decimal::Decimal;

use bibank_compliance::{AmlDecision, ApprovalLevel, CheckResult, RiskScore};

use crate::context::HookContext;
use crate::error::HookResult;
use crate::traits::{HookDecision, PostCommitHook, PreValidationHook};

// =============================================================================
// SanctionsHook - Pre-validation BLOCK hook
// =============================================================================

/// Sanctions/Watchlist check hook
///
/// This pre-validation hook blocks transactions from users on the watchlist.
/// It runs BEFORE ledger commit and can reject transactions immediately.
pub struct SanctionsHook {
    /// Priority (lower = runs first)
    priority: u32,
    /// Watchlist (user IDs that are sanctioned)
    watchlist: RwLock<HashSet<String>>,
}

impl SanctionsHook {
    /// Create a new sanctions hook with empty watchlist
    pub fn new(priority: u32) -> Self {
        Self {
            priority,
            watchlist: RwLock::new(HashSet::new()),
        }
    }

    /// Add a user to the watchlist
    pub fn add_to_watchlist(&self, user_id: &str) {
        let mut watchlist = self.watchlist.write().unwrap();
        watchlist.insert(user_id.to_string());
    }

    /// Remove a user from the watchlist
    pub fn remove_from_watchlist(&self, user_id: &str) {
        let mut watchlist = self.watchlist.write().unwrap();
        watchlist.remove(user_id);
    }

    /// Check if a user is on the watchlist
    pub fn is_on_watchlist(&self, user_id: &str) -> bool {
        let watchlist = self.watchlist.read().unwrap();
        watchlist.contains(user_id)
    }
}

#[async_trait]
impl PreValidationHook for SanctionsHook {
    fn name(&self) -> &str {
        "sanctions_hook"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn on_pre_validation(&self, ctx: &HookContext) -> HookResult<HookDecision> {
        // Check if user is on watchlist
        if self.is_on_watchlist(&ctx.user_id) {
            return Ok(HookDecision::block(
                format!("User {} is on sanctions watchlist", ctx.user_id),
                "SANCTIONS_BLOCKED",
            ));
        }

        // Also check if flagged in metadata
        if ctx.metadata.is_watchlisted {
            return Ok(HookDecision::block(
                "User is flagged as watchlisted",
                "WATCHLIST_BLOCKED",
            ));
        }

        Ok(HookDecision::Allow)
    }
}

// =============================================================================
// PepCheckHook - Pre-validation BLOCK hook for PEPs
// =============================================================================

/// Politically Exposed Person (PEP) check hook
///
/// Blocks transactions from PEPs above certain thresholds or
/// requires enhanced due diligence.
pub struct PepCheckHook {
    priority: u32,
    /// Threshold above which PEP transactions are blocked
    threshold: Decimal,
}

impl PepCheckHook {
    /// Create a new PEP check hook
    pub fn new(priority: u32, threshold: Decimal) -> Self {
        Self { priority, threshold }
    }
}

#[async_trait]
impl PreValidationHook for PepCheckHook {
    fn name(&self) -> &str {
        "pep_check_hook"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn on_pre_validation(&self, ctx: &HookContext) -> HookResult<HookDecision> {
        // Only check if user is PEP
        if !ctx.metadata.is_pep {
            return Ok(HookDecision::Allow);
        }

        // PEP with transaction above threshold is blocked
        if ctx.amount >= self.threshold {
            return Ok(HookDecision::block(
                format!(
                    "PEP transaction {} {} exceeds threshold {}",
                    ctx.amount, ctx.asset, self.threshold
                ),
                "PEP_THRESHOLD_EXCEEDED",
            ));
        }

        Ok(HookDecision::Allow)
    }
}

// =============================================================================
// NewAccountHook - Post-commit FLAG hook for new accounts
// =============================================================================

/// New account large transaction hook
///
/// Flags large transactions from accounts less than N days old.
pub struct NewAccountHook {
    priority: u32,
    /// Account age threshold in days
    age_threshold_days: i64,
    /// Amount threshold for flagging
    amount_threshold: Decimal,
}

impl NewAccountHook {
    /// Create a new account hook
    pub fn new(priority: u32, age_threshold_days: i64, amount_threshold: Decimal) -> Self {
        Self {
            priority,
            age_threshold_days,
            amount_threshold,
        }
    }
}

#[async_trait]
impl PostCommitHook for NewAccountHook {
    fn name(&self) -> &str {
        "new_account_hook"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn on_post_commit(&self, ctx: &HookContext) -> HookResult<CheckResult> {
        // Check if account is new and transaction is large
        let account_age = ctx.metadata.account_age_days.unwrap_or(365);
        let is_new_account = account_age < self.age_threshold_days;
        let is_large_amount = ctx.amount >= self.amount_threshold;

        if is_new_account && is_large_amount {
            return Ok(CheckResult {
                decision: AmlDecision::flagged(
                    format!(
                        "Account {} days old, transaction {} {} >= {}",
                        account_age, ctx.amount, ctx.asset, self.amount_threshold
                    ),
                    RiskScore::Medium,
                    ApprovalLevel::L1,
                ),
                rules_triggered: vec!["NEW_ACCOUNT_LARGE_TX".to_string()],
                risk_score: Some(RiskScore::Medium),
            });
        }

        Ok(CheckResult {
            decision: AmlDecision::Approved,
            rules_triggered: vec![],
            risk_score: None,
        })
    }
}

// =============================================================================
// LargeTxHook - Post-commit FLAG hook for large transactions
// =============================================================================

/// Large transaction monitoring hook
///
/// Flags transactions above a configurable threshold.
pub struct LargeTxHook {
    priority: u32,
    /// Threshold for flagging
    threshold: Decimal,
}

impl LargeTxHook {
    /// Create a new large transaction hook
    pub fn new(priority: u32, threshold: Decimal) -> Self {
        Self { priority, threshold }
    }
}

#[async_trait]
impl PostCommitHook for LargeTxHook {
    fn name(&self) -> &str {
        "large_tx_hook"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn on_post_commit(&self, ctx: &HookContext) -> HookResult<CheckResult> {
        if ctx.amount >= self.threshold {
            return Ok(CheckResult {
                decision: AmlDecision::flagged(
                    format!("Large transaction: {} {} >= threshold {}", ctx.amount, ctx.asset, self.threshold),
                    RiskScore::Medium,
                    ApprovalLevel::L1,
                ),
                rules_triggered: vec!["LARGE_TX_ALERT".to_string()],
                risk_score: Some(RiskScore::Medium),
            });
        }

        Ok(CheckResult {
            decision: AmlDecision::Approved,
            rules_triggered: vec![],
            risk_score: None,
        })
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::HookMetadata;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_sanctions_hook_allow() {
        let hook = SanctionsHook::new(10);
        let ctx = HookContext::new(
            "corr-1",
            "user-clean",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        );

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_sanctions_hook_block() {
        let hook = SanctionsHook::new(10);
        hook.add_to_watchlist("user-bad");

        let ctx = HookContext::new(
            "corr-1",
            "user-bad",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        );

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_blocked());
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "SANCTIONS_BLOCKED");
        }
    }

    #[tokio::test]
    async fn test_sanctions_hook_remove_from_watchlist() {
        let hook = SanctionsHook::new(10);
        hook.add_to_watchlist("user-temp");
        assert!(hook.is_on_watchlist("user-temp"));

        hook.remove_from_watchlist("user-temp");
        assert!(!hook.is_on_watchlist("user-temp"));
    }

    #[tokio::test]
    async fn test_sanctions_hook_metadata_watchlist() {
        let hook = SanctionsHook::new(10);
        let mut metadata = HookMetadata::default();
        metadata.is_watchlisted = true;

        let ctx = HookContext::new(
            "corr-1",
            "user-x",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        ).with_metadata(metadata);

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_blocked());
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "WATCHLIST_BLOCKED");
        }
    }

    #[tokio::test]
    async fn test_pep_hook_non_pep() {
        let hook = PepCheckHook::new(20, dec!(10000));
        let ctx = HookContext::new(
            "corr-1",
            "user-normal",
            "DEPOSIT",
            dec!(50000),
            "USDT",
        );

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_pep_hook_pep_below_threshold() {
        let hook = PepCheckHook::new(20, dec!(10000));
        let mut metadata = HookMetadata::default();
        metadata.is_pep = true;

        let ctx = HookContext::new(
            "corr-1",
            "user-pep",
            "DEPOSIT",
            dec!(5000),
            "USDT",
        ).with_metadata(metadata);

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_pep_hook_pep_above_threshold() {
        let hook = PepCheckHook::new(20, dec!(10000));
        let mut metadata = HookMetadata::default();
        metadata.is_pep = true;

        let ctx = HookContext::new(
            "corr-1",
            "user-pep",
            "DEPOSIT",
            dec!(15000),
            "USDT",
        ).with_metadata(metadata);

        let result = hook.on_pre_validation(&ctx).await.unwrap();
        assert!(result.is_blocked());
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "PEP_THRESHOLD_EXCEEDED");
        }
    }

    #[tokio::test]
    async fn test_new_account_hook_old_account() {
        let hook = NewAccountHook::new(30, 7, dec!(5000));
        let ctx = HookContext::new(
            "corr-1",
            "user-old",
            "DEPOSIT",
            dec!(10000),
            "USDT",
        ).with_account_age(30);

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[tokio::test]
    async fn test_new_account_hook_new_small() {
        let hook = NewAccountHook::new(30, 7, dec!(5000));
        let ctx = HookContext::new(
            "corr-1",
            "user-new",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        ).with_account_age(2);

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[tokio::test]
    async fn test_new_account_hook_new_large() {
        let hook = NewAccountHook::new(30, 7, dec!(5000));
        let ctx = HookContext::new(
            "corr-1",
            "user-new",
            "DEPOSIT",
            dec!(10000),
            "USDT",
        ).with_account_age(2);

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_flagged());
        assert!(result.rules_triggered.contains(&"NEW_ACCOUNT_LARGE_TX".to_string()));
    }

    #[tokio::test]
    async fn test_large_tx_hook_below_threshold() {
        let hook = LargeTxHook::new(40, dec!(10000));
        let ctx = HookContext::new(
            "corr-1",
            "user-1",
            "DEPOSIT",
            dec!(5000),
            "USDT",
        );

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[tokio::test]
    async fn test_large_tx_hook_above_threshold() {
        let hook = LargeTxHook::new(40, dec!(10000));
        let ctx = HookContext::new(
            "corr-1",
            "user-1",
            "DEPOSIT",
            dec!(15000),
            "USDT",
        );

        let result = hook.on_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_flagged());
        assert!(result.rules_triggered.contains(&"LARGE_TX_ALERT".to_string()));
    }
}
