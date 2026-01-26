//! Integration tests for hooks + ledger + compliance flow

use std::sync::Arc;

use rust_decimal_macros::dec;

use bibank_compliance::{
    AmlDecision, ApprovalLevel, ComplianceConfig, ComplianceEngine, ComplianceLedger, RiskScore,
};
use bibank_dsl::{
    account_age_lt, all_of, amount_gte, is_pep, is_watchlisted, rule, rule_set, RuleAction,
    RuleEvaluator, RuleSet,
};
use bibank_hooks::{
    HookContext, HookMetadata, HookRegistry, LargeTxHook, NewAccountHook, PepCheckHook,
    SanctionsHook, TransactionExecutor,
};

/// Create a standard test rule set
fn create_aml_ruleset() -> RuleSet {
    rule_set! {
        name: "AML_STANDARD",
        description: "Standard AML compliance rules",
        rules: [
            rule! {
                id: "SANCTIONS_CHECK",
                name: "Sanctions Watchlist Check",
                type: block,
                when: is_watchlisted!(),
                then: RuleAction::block("SANCTIONS", "User on sanctions watchlist"),
                priority: 10,
            },
            rule! {
                id: "PEP_BLOCK",
                name: "PEP Large Transaction Block",
                type: block,
                when: all_of!(is_pep!(), amount_gte!(dec!(50000))),
                then: RuleAction::block("PEP_LIMIT", "PEP transaction exceeds limit"),
                priority: 20,
            },
            rule! {
                id: "LARGE_TX_ALERT",
                name: "Large Transaction Alert",
                type: flag,
                when: amount_gte!(dec!(10000)),
                then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large transaction"),
                priority: 50,
            },
            rule! {
                id: "NEW_ACCOUNT_LARGE_TX",
                name: "New Account Large Transaction",
                type: flag,
                when: all_of!(account_age_lt!(7), amount_gte!(dec!(5000))),
                then: RuleAction::flag(RiskScore::High, ApprovalLevel::L2, "New account large tx"),
                priority: 60,
            },
        ],
    }
}

fn create_test_registry() -> HookRegistry {
    let mut registry = HookRegistry::new();
    let sanctions = SanctionsHook::new(10);
    sanctions.add_to_watchlist("blocked-user");
    sanctions.add_to_watchlist("sanctioned-entity");
    registry.register_pre_hook(Arc::new(sanctions));
    registry.register_pre_hook(Arc::new(PepCheckHook::new(20, dec!(50000))));
    registry.register_post_hook(Arc::new(NewAccountHook::new(30, 7, dec!(5000))));
    registry.register_post_hook(Arc::new(LargeTxHook::new(40, dec!(10000))));
    registry
}

#[test]
fn test_dsl_ruleset_clean_transaction() {
    let ruleset = create_aml_ruleset();
    let ctx = HookContext::new("tx-001", "clean-user", "DEPOSIT", dec!(1000), "USDT")
        .with_account_age(30);
    let result = RuleEvaluator::eval_ruleset(&ruleset, &ctx);
    assert!(result.triggered_rules.is_empty());
    assert!(result.decision.is_approved());
}

#[test]
fn test_dsl_ruleset_large_transaction() {
    let ruleset = create_aml_ruleset();
    let ctx =
        HookContext::new("tx-002", "user-1", "DEPOSIT", dec!(15000), "USDT").with_account_age(30);
    let result = RuleEvaluator::eval_ruleset(&ruleset, &ctx);
    assert!(result.triggered_rules.contains(&"LARGE_TX_ALERT".to_string()));
    assert!(result.decision.is_flagged());
}

#[test]
fn test_dsl_ruleset_watchlisted_user() {
    let ruleset = create_aml_ruleset();
    let mut ctx =
        HookContext::new("tx-003", "bad-actor", "DEPOSIT", dec!(100), "USDT").with_account_age(30);
    ctx.metadata.is_watchlisted = true;
    let result = RuleEvaluator::eval_ruleset(&ruleset, &ctx);
    assert!(result.triggered_rules.contains(&"SANCTIONS_CHECK".to_string()));
    assert!(result.decision.is_blocked());
}

#[test]
fn test_dsl_decision_aggregation() {
    let ruleset = create_aml_ruleset();
    let ctx =
        HookContext::new("tx-006", "new-user", "DEPOSIT", dec!(15000), "USDT").with_account_age(3);
    let result = RuleEvaluator::eval_ruleset(&ruleset, &ctx);
    assert!(result.triggered_rules.contains(&"LARGE_TX_ALERT".to_string()));
    assert!(result.triggered_rules.contains(&"NEW_ACCOUNT_LARGE_TX".to_string()));
    assert!(result.decision.is_flagged());
    if let AmlDecision::Flagged {
        required_approval, ..
    } = result.decision
    {
        assert_eq!(required_approval, ApprovalLevel::L2);
    }
}

#[tokio::test]
async fn test_executor_clean_approved() {
    let executor = TransactionExecutor::new(Arc::new(create_test_registry()));
    let ctx = HookContext::new("tx-001", "clean-user", "DEPOSIT", dec!(1000), "USDT")
        .with_account_age(30);
    let result = executor.execute(&ctx).await.unwrap();
    assert!(result.committed);
    assert!(result.decision.is_approved());
    assert!(!result.lock_applied);
}

#[tokio::test]
async fn test_executor_sanctions_blocked() {
    let executor = TransactionExecutor::new(Arc::new(create_test_registry()));
    let ctx = HookContext::new("tx-002", "blocked-user", "DEPOSIT", dec!(100), "USDT")
        .with_account_age(30);
    let result = executor.execute(&ctx).await.unwrap();
    assert!(!result.committed);
    assert!(result.decision.is_blocked());
}

#[tokio::test]
async fn test_executor_large_tx_flagged() {
    let executor = TransactionExecutor::new(Arc::new(create_test_registry()));
    let ctx =
        HookContext::new("tx-004", "user-1", "DEPOSIT", dec!(25000), "USDT").with_account_age(30);
    let result = executor.execute(&ctx).await.unwrap();
    assert!(result.committed);
    assert!(result.decision.is_flagged());
    assert!(result.lock_applied);
}

#[tokio::test]
async fn test_executor_batch_mixed() {
    let executor = TransactionExecutor::new(Arc::new(create_test_registry()));
    let contexts = vec![
        HookContext::new("tx-001", "user-1", "DEPOSIT", dec!(100), "USDT").with_account_age(30),
        HookContext::new("tx-002", "blocked-user", "DEPOSIT", dec!(100), "USDT")
            .with_account_age(30),
        HookContext::new("tx-003", "user-3", "DEPOSIT", dec!(50000), "USDT").with_account_age(30),
    ];
    let results = executor.execute_batch(&contexts).await;
    assert_eq!(results.len(), 3);
    assert!(results[0].as_ref().unwrap().decision.is_approved());
    assert!(results[1].as_ref().unwrap().decision.is_blocked());
    assert!(results[2].as_ref().unwrap().decision.is_flagged());
}

#[test]
fn test_compliance_engine_large_tx() {
    let ledger = ComplianceLedger::in_memory();
    let config = ComplianceConfig::default();
    let mut engine = ComplianceEngine::new(config, ledger);
    let tx_ctx = bibank_compliance::engine::TransactionContext {
        correlation_id: "tx-001".to_string(),
        user_id: "user-1".to_string(),
        intent: "DEPOSIT".to_string(),
        amount: dec!(15000),
        asset: "USDT".to_string(),
        account_age_days: Some(30),
    };
    let result = engine.check_transaction(&tx_ctx).unwrap();
    assert!(result.decision.is_flagged());
    assert!(result.rules_triggered.contains(&"LARGE_TX_ALERT".to_string()));
}

#[tokio::test]
async fn test_e2e_deposit_approved() {
    let executor = TransactionExecutor::new(Arc::new(create_test_registry()));
    let ctx = HookContext::new("dep-001", "customer-123", "DEPOSIT", dec!(5000), "USDT")
        .with_account_age(60)
        .with_kyc_level(2);
    let result = executor.execute(&ctx).await.unwrap();
    assert!(result.committed);
    assert!(result.decision.is_approved());
}

#[tokio::test]
async fn test_e2e_withdrawal_flagged() {
    let executor = TransactionExecutor::new(Arc::new(create_test_registry()));
    let ctx = HookContext::new("wd-001", "customer-456", "WITHDRAWAL", dec!(20000), "USDT")
        .with_account_age(90)
        .with_kyc_level(3);
    let result = executor.execute(&ctx).await.unwrap();
    assert!(result.committed);
    assert!(result.decision.is_flagged());
    assert!(result.lock_applied);
}

#[tokio::test]
async fn test_e2e_new_customer_flow() {
    let executor = TransactionExecutor::new(Arc::new(create_test_registry()));
    // Small deposit - approved
    let ctx1 = HookContext::new("dep-new-001", "new-customer", "DEPOSIT", dec!(2000), "USDT")
        .with_account_age(1);
    let result1 = executor.execute(&ctx1).await.unwrap();
    assert!(result1.committed);
    assert!(result1.decision.is_approved());
    // Large deposit - flagged
    let ctx2 = HookContext::new("dep-new-002", "new-customer", "DEPOSIT", dec!(8000), "USDT")
        .with_account_age(1);
    let result2 = executor.execute(&ctx2).await.unwrap();
    assert!(result2.committed);
    assert!(result2.decision.is_flagged());
    assert!(result2.lock_applied);
}
