//! Transaction Executor - Orchestrates the full transaction lifecycle
//!
//! This module provides the main entry point for processing transactions
//! with compliance hooks:
//!
//! ```text
//! TransactionIntent
//!        │
//!        ▼
//! ┌─────────────────┐
//! │ Pre-validation  │──► Block? Return error
//! │ Hooks           │
//! └────────┬────────┘
//!          │ Allow
//!          ▼
//! ┌─────────────────┐
//! │ Ledger Commit   │──► Append to Journal
//! │                 │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ Post-commit     │──► Flag? Apply Lock
//! │ Hooks           │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ Compliance      │──► Write decision
//! │ Ledger          │
//! └─────────────────┘
//! ```

use std::sync::Arc;

use bibank_compliance::{AmlDecision, ComplianceEngine};
use tokio::sync::RwLock;

use crate::context::HookContext;
use crate::error::HookResult;
use crate::registry::HookRegistry;
use crate::traits::HookDecision;

/// Result of executing a transaction through the compliance pipeline
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Original correlation ID
    pub correlation_id: String,
    /// Whether the transaction was committed
    pub committed: bool,
    /// Final compliance decision
    pub decision: AmlDecision,
    /// Rules that were triggered
    pub rules_triggered: Vec<String>,
    /// Whether a lock was applied
    pub lock_applied: bool,
    /// Block reason (if blocked)
    pub block_reason: Option<String>,
}

impl ExecutionResult {
    /// Create a blocked result
    pub fn blocked(correlation_id: String, reason: String) -> Self {
        Self {
            correlation_id,
            committed: false,
            decision: AmlDecision::blocked(&reason, "HOOK_REJECTED"),
            rules_triggered: vec![],
            lock_applied: false,
            block_reason: Some(reason),
        }
    }

    /// Create an approved result
    pub fn approved(correlation_id: String) -> Self {
        Self {
            correlation_id,
            committed: true,
            decision: AmlDecision::Approved,
            rules_triggered: vec![],
            lock_applied: false,
            block_reason: None,
        }
    }

    /// Create a flagged result with lock
    pub fn flagged(correlation_id: String, decision: AmlDecision, rules: Vec<String>) -> Self {
        Self {
            correlation_id,
            committed: true,
            decision,
            rules_triggered: rules,
            lock_applied: true,
            block_reason: None,
        }
    }
}

/// Transaction executor with compliance hooks
///
/// This is the main orchestrator for processing transactions with:
/// - Pre-validation hooks (BLOCK rules)
/// - Ledger commit simulation
/// - Post-commit hooks (FLAG rules)
/// - Lock application for flagged transactions
pub struct TransactionExecutor {
    /// Hook registry
    registry: Arc<HookRegistry>,
    /// Compliance engine (optional - for full rule evaluation)
    #[allow(dead_code)]
    compliance_engine: Option<Arc<RwLock<ComplianceEngine>>>,
}

impl TransactionExecutor {
    /// Create a new executor with just hooks
    pub fn new(registry: Arc<HookRegistry>) -> Self {
        Self {
            registry,
            compliance_engine: None,
        }
    }

    /// Create an executor with full compliance engine
    pub fn with_compliance_engine(
        registry: Arc<HookRegistry>,
        engine: Arc<RwLock<ComplianceEngine>>,
    ) -> Self {
        Self {
            registry,
            compliance_engine: Some(engine),
        }
    }

    /// Execute a transaction through the full compliance pipeline
    ///
    /// Returns:
    /// - `Ok(ExecutionResult)` with the outcome (committed, blocked, flagged)
    /// - `Err(HookError)` if hooks fail and FailPolicy is FailClosed
    pub async fn execute(&self, ctx: &HookContext) -> HookResult<ExecutionResult> {
        let correlation_id = ctx.correlation_id.clone();

        // === Phase 1: Pre-validation hooks ===
        let pre_decision = self.registry.run_pre_validation(ctx).await?;

        match pre_decision {
            HookDecision::Block { reason, .. } => {
                // Transaction blocked - never committed
                return Ok(ExecutionResult::blocked(correlation_id, reason));
            }
            HookDecision::Allow => {
                // Continue to commit
            }
        }

        // === Phase 2: Ledger Commit (simulated) ===
        // In real implementation, this would call bibank-ledger
        // to append the transaction to the journal

        // === Phase 3: Post-commit hooks ===
        let post_result = self.registry.run_post_commit(ctx).await?;

        // Aggregate decision (using max lattice)
        let final_decision = post_result.decision;
        let rules = post_result.rules_triggered;

        // === Phase 4: Apply lock if flagged ===
        if final_decision.is_flagged() {
            // In real implementation, this would:
            // 1. Write LockApplied event to main journal
            // 2. Write TransactionFlagged event to compliance ledger

            Ok(ExecutionResult::flagged(
                correlation_id,
                final_decision,
                rules,
            ))
        } else {
            Ok(ExecutionResult::approved(correlation_id))
        }
    }

    /// Execute a batch of transactions
    pub async fn execute_batch(
        &self,
        contexts: &[HookContext],
    ) -> Vec<HookResult<ExecutionResult>> {
        let mut results = Vec::with_capacity(contexts.len());

        for ctx in contexts {
            results.push(self.execute(ctx).await);
        }

        results
    }
}

/// Builder for TransactionExecutor
pub struct ExecutorBuilder {
    registry: HookRegistry,
    compliance_engine: Option<Arc<RwLock<ComplianceEngine>>>,
}

impl ExecutorBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            registry: HookRegistry::new(),
            compliance_engine: None,
        }
    }

    /// Set the hook registry
    pub fn with_registry(mut self, registry: HookRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Add a compliance engine
    pub fn with_compliance_engine(mut self, engine: ComplianceEngine) -> Self {
        self.compliance_engine = Some(Arc::new(RwLock::new(engine)));
        self
    }

    /// Build the executor
    pub fn build(self) -> TransactionExecutor {
        let registry = Arc::new(self.registry);

        match self.compliance_engine {
            Some(engine) => TransactionExecutor::with_compliance_engine(registry, engine),
            None => TransactionExecutor::new(registry),
        }
    }
}

impl Default for ExecutorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aml::{NewAccountHook, PepCheckHook, SanctionsHook, LargeTxHook};
    use crate::context::HookMetadata;
    use rust_decimal_macros::dec;

    fn create_registry_with_hooks() -> HookRegistry {
        let mut registry = HookRegistry::new();

        // Pre-validation hooks
        let sanctions = SanctionsHook::new(10);
        sanctions.add_to_watchlist("blocked-user");
        registry.register_pre_hook(Arc::new(sanctions));
        registry.register_pre_hook(Arc::new(PepCheckHook::new(20, dec!(10000))));

        // Post-commit hooks
        registry.register_post_hook(Arc::new(NewAccountHook::new(30, 7, dec!(5000))));
        registry.register_post_hook(Arc::new(LargeTxHook::new(40, dec!(10000))));

        registry
    }

    #[tokio::test]
    async fn test_executor_approved() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let ctx = HookContext::new(
            "tx-001",
            "clean-user",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        ).with_account_age(30);

        let result = executor.execute(&ctx).await.unwrap();

        assert!(result.committed);
        assert!(result.decision.is_approved());
        assert!(!result.lock_applied);
        assert!(result.block_reason.is_none());
    }

    #[tokio::test]
    async fn test_executor_blocked_by_sanctions() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let ctx = HookContext::new(
            "tx-002",
            "blocked-user",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        );

        let result = executor.execute(&ctx).await.unwrap();

        assert!(!result.committed);
        assert!(result.decision.is_blocked());
        assert!(result.block_reason.is_some());
        assert!(result.block_reason.unwrap().contains("sanctions"));
    }

    #[tokio::test]
    async fn test_executor_blocked_by_pep() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let mut metadata = HookMetadata::default();
        metadata.is_pep = true;

        let ctx = HookContext::new(
            "tx-003",
            "pep-user",
            "DEPOSIT",
            dec!(50000),
            "USDT",
        ).with_metadata(metadata);

        let result = executor.execute(&ctx).await.unwrap();

        assert!(!result.committed);
        assert!(result.decision.is_blocked());
    }

    #[tokio::test]
    async fn test_executor_flagged_by_large_tx() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let ctx = HookContext::new(
            "tx-004",
            "normal-user",
            "DEPOSIT",
            dec!(15000),
            "USDT",
        ).with_account_age(30);

        let result = executor.execute(&ctx).await.unwrap();

        assert!(result.committed);
        assert!(result.decision.is_flagged());
        assert!(result.lock_applied);
        assert!(result.rules_triggered.contains(&"LARGE_TX_ALERT".to_string()));
    }

    #[tokio::test]
    async fn test_executor_flagged_by_new_account() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let ctx = HookContext::new(
            "tx-005",
            "new-user",
            "DEPOSIT",
            dec!(8000),
            "USDT",
        ).with_account_age(2);

        let result = executor.execute(&ctx).await.unwrap();

        assert!(result.committed);
        assert!(result.decision.is_flagged());
        assert!(result.lock_applied);
        assert!(result.rules_triggered.contains(&"NEW_ACCOUNT_LARGE_TX".to_string()));
    }

    #[tokio::test]
    async fn test_executor_batch() {
        let executor = TransactionExecutor::new(Arc::new(create_registry_with_hooks()));

        let contexts = vec![
            HookContext::new("tx-001", "user-1", "DEPOSIT", dec!(100), "USDT")
                .with_account_age(30),
            HookContext::new("tx-002", "blocked-user", "DEPOSIT", dec!(100), "USDT"),
            HookContext::new("tx-003", "user-3", "DEPOSIT", dec!(20000), "USDT")
                .with_account_age(30),
        ];

        let results = executor.execute_batch(&contexts).await;

        assert_eq!(results.len(), 3);

        // First: approved
        assert!(results[0].as_ref().unwrap().committed);
        assert!(results[0].as_ref().unwrap().decision.is_approved());

        // Second: blocked
        assert!(!results[1].as_ref().unwrap().committed);
        assert!(results[1].as_ref().unwrap().decision.is_blocked());

        // Third: flagged
        assert!(results[2].as_ref().unwrap().committed);
        assert!(results[2].as_ref().unwrap().decision.is_flagged());
    }

    #[tokio::test]
    async fn test_builder() {
        let registry = create_registry_with_hooks();
        let executor = ExecutorBuilder::new()
            .with_registry(registry)
            .build();

        let ctx = HookContext::new(
            "tx-001",
            "clean-user",
            "DEPOSIT",
            dec!(1000),
            "USDT",
        ).with_account_age(30);

        let result = executor.execute(&ctx).await.unwrap();
        assert!(result.committed);
    }
}
