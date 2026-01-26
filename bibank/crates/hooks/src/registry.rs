//! Hook Registry - manages and executes hooks in order

use std::sync::Arc;

use bibank_compliance::{AmlDecision, CheckResult, FailPolicy};

use crate::context::HookContext;
use crate::error::HookResult;
use crate::traits::{HookDecision, PostCommitHook, PreValidationHook};

/// Registry for managing hooks
///
/// Hooks are executed in priority order (lower = first).
/// Pre-validation hooks can BLOCK transactions.
/// Post-commit hooks can FLAG transactions for review.
pub struct HookRegistry {
    /// Pre-validation hooks (run before ledger commit)
    pre_hooks: Vec<Arc<dyn PreValidationHook>>,

    /// Post-commit hooks (run after ledger commit)
    post_hooks: Vec<Arc<dyn PostCommitHook>>,

    /// Policy when hooks fail
    fail_policy: FailPolicy,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
            fail_policy: FailPolicy::FailClosed,
        }
    }

    /// Set the fail policy
    pub fn with_fail_policy(mut self, policy: FailPolicy) -> Self {
        self.fail_policy = policy;
        self
    }

    /// Register a pre-validation hook
    pub fn register_pre_hook(&mut self, hook: Arc<dyn PreValidationHook>) {
        self.pre_hooks.push(hook);
        // Sort by priority
        self.pre_hooks.sort_by_key(|h| h.priority());
    }

    /// Register a post-commit hook
    pub fn register_post_hook(&mut self, hook: Arc<dyn PostCommitHook>) {
        self.post_hooks.push(hook);
        // Sort by priority
        self.post_hooks.sort_by_key(|h| h.priority());
    }

    /// Run all pre-validation hooks
    ///
    /// Returns Block if any hook blocks, Allow if all pass.
    /// On hook failure, behavior depends on fail_policy.
    pub async fn run_pre_validation(&self, ctx: &HookContext) -> HookResult<HookDecision> {
        for hook in &self.pre_hooks {
            let result = hook.on_pre_validation(ctx).await;

            match result {
                Ok(HookDecision::Allow) => {
                    tracing::debug!(hook = hook.name(), "Pre-validation hook passed");
                    continue;
                }
                Ok(HookDecision::Block { reason, code }) => {
                    tracing::warn!(
                        hook = hook.name(),
                        reason = %reason,
                        code = %code,
                        "Pre-validation hook blocked transaction"
                    );
                    return Ok(HookDecision::Block { reason, code });
                }
                Err(e) => {
                    tracing::error!(
                        hook = hook.name(),
                        error = %e,
                        "Pre-validation hook failed"
                    );

                    match self.fail_policy {
                        FailPolicy::FailClosed => {
                            return Ok(HookDecision::block(
                                format!("Hook {} failed: {}", hook.name(), e),
                                "HOOK_FAILURE",
                            ));
                        }
                        FailPolicy::FailOpen => {
                            tracing::warn!(
                                hook = hook.name(),
                                "FailOpen: continuing despite hook failure"
                            );
                            continue;
                        }
                    }
                }
            }
        }

        Ok(HookDecision::Allow)
    }

    /// Run all post-commit hooks
    ///
    /// Returns aggregated check result from all hooks.
    pub async fn run_post_commit(&self, ctx: &HookContext) -> HookResult<CheckResult> {
        let mut all_decisions = Vec::new();
        let mut all_rules = Vec::new();
        let mut highest_risk = None;

        for hook in &self.post_hooks {
            let result = hook.on_post_commit(ctx).await;

            match result {
                Ok(check) => {
                    tracing::debug!(
                        hook = hook.name(),
                        decision = ?check.decision,
                        "Post-commit hook completed"
                    );

                    all_decisions.push(check.decision.clone());
                    all_rules.extend(check.rules_triggered);

                    if let Some(score) = check.risk_score {
                        highest_risk = match highest_risk {
                            None => Some(score),
                            Some(existing) if score > existing => Some(score),
                            Some(existing) => Some(existing),
                        };
                    }
                }
                Err(e) => {
                    tracing::error!(
                        hook = hook.name(),
                        error = %e,
                        "Post-commit hook failed"
                    );

                    // Post-commit hooks don't block, so we just log
                    // and continue even on FailClosed
                    all_rules.push(format!("HOOK_ERROR:{}", hook.name()));
                }
            }
        }

        // Aggregate decisions (most restrictive wins)
        let decision = AmlDecision::aggregate(all_decisions);

        Ok(CheckResult {
            decision,
            rules_triggered: all_rules,
            risk_score: highest_risk,
        })
    }

    /// Get number of registered pre-validation hooks
    pub fn pre_hook_count(&self) -> usize {
        self.pre_hooks.len()
    }

    /// Get number of registered post-commit hooks
    pub fn post_hook_count(&self) -> usize {
        self.post_hooks.len()
    }

    /// Get current fail policy
    pub fn fail_policy(&self) -> FailPolicy {
        self.fail_policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{NoOpPostCommitHook, NoOpPreValidationHook};
    use rust_decimal_macros::dec;

    fn create_ctx() -> HookContext {
        HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT")
    }

    #[test]
    fn test_empty_registry() {
        let registry = HookRegistry::new();
        assert_eq!(registry.pre_hook_count(), 0);
        assert_eq!(registry.post_hook_count(), 0);
    }

    #[test]
    fn test_register_hooks() {
        let mut registry = HookRegistry::new();

        registry.register_pre_hook(Arc::new(NoOpPreValidationHook));
        registry.register_post_hook(Arc::new(NoOpPostCommitHook));

        assert_eq!(registry.pre_hook_count(), 1);
        assert_eq!(registry.post_hook_count(), 1);
    }

    #[tokio::test]
    async fn test_run_empty_pre_validation() {
        let registry = HookRegistry::new();
        let ctx = create_ctx();

        let result = registry.run_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_run_noop_pre_validation() {
        let mut registry = HookRegistry::new();
        registry.register_pre_hook(Arc::new(NoOpPreValidationHook));

        let ctx = create_ctx();
        let result = registry.run_pre_validation(&ctx).await.unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_run_empty_post_commit() {
        let registry = HookRegistry::new();
        let ctx = create_ctx();

        let result = registry.run_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    #[tokio::test]
    async fn test_run_noop_post_commit() {
        let mut registry = HookRegistry::new();
        registry.register_post_hook(Arc::new(NoOpPostCommitHook));

        let ctx = create_ctx();
        let result = registry.run_post_commit(&ctx).await.unwrap();
        assert!(result.decision.is_approved());
    }

    // Test blocking hook
    struct BlockingHook;

    #[async_trait::async_trait]
    impl PreValidationHook for BlockingHook {
        fn name(&self) -> &str {
            "BlockingHook"
        }

        async fn on_pre_validation(&self, _ctx: &HookContext) -> HookResult<HookDecision> {
            Ok(HookDecision::block("Test block", "TEST-001"))
        }
    }

    #[tokio::test]
    async fn test_blocking_hook() {
        let mut registry = HookRegistry::new();
        registry.register_pre_hook(Arc::new(BlockingHook));

        let ctx = create_ctx();
        let result = registry.run_pre_validation(&ctx).await.unwrap();

        assert!(result.is_blocked());
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "TEST-001");
        }
    }

    // Test hook ordering
    struct PriorityHook {
        priority: u32,
        should_block: bool,
    }

    #[async_trait::async_trait]
    impl PreValidationHook for PriorityHook {
        fn name(&self) -> &str {
            "PriorityHook"
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        async fn on_pre_validation(&self, _ctx: &HookContext) -> HookResult<HookDecision> {
            if self.should_block {
                Ok(HookDecision::block(
                    format!("Blocked by priority {}", self.priority),
                    format!("P{}", self.priority),
                ))
            } else {
                Ok(HookDecision::Allow)
            }
        }
    }

    #[tokio::test]
    async fn test_hook_priority_order() {
        let mut registry = HookRegistry::new();

        // Register in reverse order to test sorting
        registry.register_pre_hook(Arc::new(PriorityHook {
            priority: 200,
            should_block: true,
        }));
        registry.register_pre_hook(Arc::new(PriorityHook {
            priority: 50,
            should_block: true,
        }));
        registry.register_pre_hook(Arc::new(PriorityHook {
            priority: 100,
            should_block: true,
        }));

        let ctx = create_ctx();
        let result = registry.run_pre_validation(&ctx).await.unwrap();

        // Should be blocked by priority 50 (runs first)
        if let HookDecision::Block { code, .. } = result {
            assert_eq!(code, "P50");
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_fail_policy_default() {
        let registry = HookRegistry::new();
        assert_eq!(registry.fail_policy(), FailPolicy::FailClosed);
    }

    #[test]
    fn test_fail_policy_custom() {
        let registry = HookRegistry::new().with_fail_policy(FailPolicy::FailOpen);
        assert_eq!(registry.fail_policy(), FailPolicy::FailOpen);
    }
}
