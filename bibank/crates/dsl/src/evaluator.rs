//! Rule evaluator - evaluates rules against transaction context

use rust_decimal::Decimal;

use bibank_compliance::{AmlDecision, CheckResult};
use bibank_hooks::HookContext;

use crate::types::{Condition, RuleAction, RuleDefinition, RuleSet, RuleType};

/// Result of evaluating a single rule
#[derive(Debug, Clone)]
pub struct RuleEvalResult {
    /// Rule ID
    pub rule_id: String,
    /// Whether the rule triggered
    pub triggered: bool,
    /// The action (if triggered)
    pub action: Option<RuleAction>,
}

/// Result of evaluating a rule set
#[derive(Debug, Clone)]
pub struct RuleSetEvalResult {
    /// Individual rule results
    pub results: Vec<RuleEvalResult>,
    /// Rules that triggered
    pub triggered_rules: Vec<String>,
    /// Final aggregated decision
    pub decision: AmlDecision,
}

/// Rule evaluator
pub struct RuleEvaluator;

impl RuleEvaluator {
    /// Evaluate a single condition against a context
    pub fn eval_condition(condition: &Condition, ctx: &HookContext) -> bool {
        match condition {
            Condition::AmountGte { threshold } => ctx.amount >= *threshold,
            Condition::AmountLt { threshold } => ctx.amount < *threshold,
            Condition::AmountInRange { min, max } => ctx.amount >= *min && ctx.amount < *max,
            Condition::AccountAgeLt { days } => {
                ctx.metadata.account_age_days.unwrap_or(365) < *days
            }
            Condition::AccountAgeGte { days } => {
                ctx.metadata.account_age_days.unwrap_or(365) >= *days
            }
            Condition::IsWatchlisted => ctx.metadata.is_watchlisted,
            Condition::IsPep => ctx.metadata.is_pep,
            Condition::TxCountGte { count, .. } => {
                // Would need velocity state - simplified for now
                // In real implementation, would query ComplianceState
                false
            }
            Condition::VolumeGte { threshold, .. } => {
                // Would need velocity state - simplified for now
                false
            }
            Condition::Custom { .. } => {
                // Custom conditions need external handler
                false
            }
            Condition::All { conditions } => {
                conditions.iter().all(|c| Self::eval_condition(c, ctx))
            }
            Condition::Any { conditions } => {
                conditions.iter().any(|c| Self::eval_condition(c, ctx))
            }
        }
    }

    /// Evaluate a single rule
    pub fn eval_rule(rule: &RuleDefinition, ctx: &HookContext) -> RuleEvalResult {
        if !rule.enabled {
            return RuleEvalResult {
                rule_id: rule.id.clone(),
                triggered: false,
                action: None,
            };
        }

        let triggered = Self::eval_condition(&rule.condition, ctx);

        RuleEvalResult {
            rule_id: rule.id.clone(),
            triggered,
            action: if triggered { Some(rule.action.clone()) } else { None },
        }
    }

    /// Evaluate all rules in a rule set
    pub fn eval_ruleset(ruleset: &RuleSet, ctx: &HookContext) -> RuleSetEvalResult {
        let mut results = Vec::new();
        let mut triggered_rules = Vec::new();
        let mut decisions = Vec::new();

        // Evaluate all rules
        for rule in &ruleset.rules {
            let result = Self::eval_rule(rule, ctx);
            if result.triggered {
                triggered_rules.push(rule.id.clone());
                if let Some(action) = &result.action {
                    decisions.push(action.to_decision());
                }
            }
            results.push(result);
        }

        // Aggregate decisions using max()
        let final_decision = decisions
            .into_iter()
            .max()
            .unwrap_or(AmlDecision::Approved);

        RuleSetEvalResult {
            results,
            triggered_rules,
            decision: final_decision,
        }
    }

    /// Evaluate only BLOCK rules (for pre-validation)
    pub fn eval_block_rules(ruleset: &RuleSet, ctx: &HookContext) -> Option<RuleEvalResult> {
        for rule in ruleset.block_rules() {
            let result = Self::eval_rule(rule, ctx);
            if result.triggered {
                return Some(result);
            }
        }
        None
    }

    /// Evaluate only FLAG rules (for post-commit)
    pub fn eval_flag_rules(ruleset: &RuleSet, ctx: &HookContext) -> RuleSetEvalResult {
        let mut results = Vec::new();
        let mut triggered_rules = Vec::new();
        let mut decisions = Vec::new();

        for rule in ruleset.flag_rules() {
            let result = Self::eval_rule(rule, ctx);
            if result.triggered {
                triggered_rules.push(rule.id.clone());
                if let Some(action) = &result.action {
                    decisions.push(action.to_decision());
                }
            }
            results.push(result);
        }

        let final_decision = decisions
            .into_iter()
            .max()
            .unwrap_or(AmlDecision::Approved);

        RuleSetEvalResult {
            results,
            triggered_rules,
            decision: final_decision,
        }
    }

    /// Convert rule set evaluation to CheckResult
    pub fn to_check_result(eval_result: &RuleSetEvalResult) -> CheckResult {
        CheckResult {
            decision: eval_result.decision.clone(),
            rules_triggered: eval_result.triggered_rules.clone(),
            risk_score: None, // Would extract from flagged decision
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RuleAction;
    use crate::{account_age_lt, all_of, amount_gte, is_watchlisted, rule, rule_set};
    use bibank_compliance::{ApprovalLevel, RiskScore};
    use rust_decimal_macros::dec;

    fn create_test_context(amount: Decimal, watchlisted: bool, age_days: i64) -> HookContext {
        let mut ctx = HookContext::new("corr-1", "user-1", "DEPOSIT", amount, "USDT")
            .with_account_age(age_days);
        if watchlisted {
            ctx.metadata.is_watchlisted = true;
        }
        ctx
    }

    #[test]
    fn test_eval_amount_gte() {
        let ctx = create_test_context(dec!(15000), false, 30);
        assert!(RuleEvaluator::eval_condition(
            &Condition::amount_gte(dec!(10000)),
            &ctx
        ));
        assert!(!RuleEvaluator::eval_condition(
            &Condition::amount_gte(dec!(20000)),
            &ctx
        ));
    }

    #[test]
    fn test_eval_amount_lt() {
        let ctx = create_test_context(dec!(5000), false, 30);
        assert!(RuleEvaluator::eval_condition(
            &Condition::amount_lt(dec!(10000)),
            &ctx
        ));
        assert!(!RuleEvaluator::eval_condition(
            &Condition::amount_lt(dec!(5000)),
            &ctx
        ));
    }

    #[test]
    fn test_eval_watchlisted() {
        let ctx_clean = create_test_context(dec!(1000), false, 30);
        let ctx_bad = create_test_context(dec!(1000), true, 30);

        assert!(!RuleEvaluator::eval_condition(
            &Condition::is_watchlisted(),
            &ctx_clean
        ));
        assert!(RuleEvaluator::eval_condition(
            &Condition::is_watchlisted(),
            &ctx_bad
        ));
    }

    #[test]
    fn test_eval_account_age() {
        let ctx_new = create_test_context(dec!(1000), false, 3);
        let ctx_old = create_test_context(dec!(1000), false, 30);

        assert!(RuleEvaluator::eval_condition(
            &Condition::account_age_lt(7),
            &ctx_new
        ));
        assert!(!RuleEvaluator::eval_condition(
            &Condition::account_age_lt(7),
            &ctx_old
        ));
    }

    #[test]
    fn test_eval_all_condition() {
        let cond = Condition::all(vec![
            Condition::amount_gte(dec!(5000)),
            Condition::account_age_lt(7),
        ]);

        let ctx_match = create_test_context(dec!(10000), false, 3);
        let ctx_no_match = create_test_context(dec!(10000), false, 30);

        assert!(RuleEvaluator::eval_condition(&cond, &ctx_match));
        assert!(!RuleEvaluator::eval_condition(&cond, &ctx_no_match));
    }

    #[test]
    fn test_eval_any_condition() {
        let cond = Condition::any(vec![
            Condition::is_watchlisted(),
            Condition::amount_gte(dec!(50000)),
        ]);

        let ctx_watchlisted = create_test_context(dec!(100), true, 30);
        let ctx_large = create_test_context(dec!(60000), false, 30);
        let ctx_neither = create_test_context(dec!(100), false, 30);

        assert!(RuleEvaluator::eval_condition(&cond, &ctx_watchlisted));
        assert!(RuleEvaluator::eval_condition(&cond, &ctx_large));
        assert!(!RuleEvaluator::eval_condition(&cond, &ctx_neither));
    }

    #[test]
    fn test_eval_rule() {
        let rule = rule! {
            id: "LARGE_TX",
            type: flag,
            when: amount_gte!(dec!(10000)),
            then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large"),
        };

        let ctx_small = create_test_context(dec!(5000), false, 30);
        let ctx_large = create_test_context(dec!(15000), false, 30);

        let result_small = RuleEvaluator::eval_rule(&rule, &ctx_small);
        assert!(!result_small.triggered);

        let result_large = RuleEvaluator::eval_rule(&rule, &ctx_large);
        assert!(result_large.triggered);
        assert!(result_large.action.is_some());
    }

    #[test]
    fn test_eval_ruleset() {
        let ruleset = rule_set! {
            name: "TEST",
            rules: [
                rule! {
                    id: "SANCTIONS",
                    type: block,
                    when: is_watchlisted!(),
                    then: RuleAction::block("SANCTIONS", "Watchlisted"),
                    priority: 10,
                },
                rule! {
                    id: "LARGE_TX",
                    type: flag,
                    when: amount_gte!(dec!(10000)),
                    then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large"),
                    priority: 50,
                },
            ],
        };

        // Clean small transaction
        let ctx1 = create_test_context(dec!(1000), false, 30);
        let result1 = RuleEvaluator::eval_ruleset(&ruleset, &ctx1);
        assert!(result1.triggered_rules.is_empty());
        assert!(result1.decision.is_approved());

        // Large transaction
        let ctx2 = create_test_context(dec!(15000), false, 30);
        let result2 = RuleEvaluator::eval_ruleset(&ruleset, &ctx2);
        assert!(result2.triggered_rules.contains(&"LARGE_TX".to_string()));
        assert!(result2.decision.is_flagged());

        // Watchlisted user
        let ctx3 = create_test_context(dec!(1000), true, 30);
        let result3 = RuleEvaluator::eval_ruleset(&ruleset, &ctx3);
        assert!(result3.triggered_rules.contains(&"SANCTIONS".to_string()));
        assert!(result3.decision.is_blocked());
    }

    #[test]
    fn test_eval_block_rules_only() {
        let ruleset = rule_set! {
            name: "TEST",
            rules: [
                rule! {
                    id: "SANCTIONS",
                    type: block,
                    when: is_watchlisted!(),
                    then: RuleAction::block("SANCTIONS", "Watchlisted"),
                },
                rule! {
                    id: "LARGE_TX",
                    type: flag,
                    when: amount_gte!(dec!(10000)),
                    then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large"),
                },
            ],
        };

        let ctx_clean = create_test_context(dec!(15000), false, 30);
        let result = RuleEvaluator::eval_block_rules(&ruleset, &ctx_clean);
        assert!(result.is_none()); // No BLOCK rules triggered

        let ctx_bad = create_test_context(dec!(100), true, 30);
        let result2 = RuleEvaluator::eval_block_rules(&ruleset, &ctx_bad);
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().rule_id, "SANCTIONS");
    }

    #[test]
    fn test_decision_aggregation() {
        let ruleset = rule_set! {
            name: "TEST",
            rules: [
                rule! {
                    id: "RULE_1",
                    type: flag,
                    when: amount_gte!(dec!(5000)),
                    then: RuleAction::flag(RiskScore::Low, ApprovalLevel::L1, "Flag 1"),
                },
                rule! {
                    id: "RULE_2",
                    type: flag,
                    when: amount_gte!(dec!(10000)),
                    then: RuleAction::flag(RiskScore::High, ApprovalLevel::L3, "Flag 2"),
                },
            ],
        };

        // Both rules trigger, should get max (Higher risk/approval)
        let ctx = create_test_context(dec!(15000), false, 30);
        let result = RuleEvaluator::eval_ruleset(&ruleset, &ctx);

        assert_eq!(result.triggered_rules.len(), 2);
        assert!(result.decision.is_flagged());
        // Max should be L3 with High risk
        if let AmlDecision::Flagged {
            required_approval,
            risk_score,
            ..
        } = &result.decision
        {
            assert_eq!(*required_approval, ApprovalLevel::L3);
            assert_eq!(*risk_score, RiskScore::High);
        }
    }
}
