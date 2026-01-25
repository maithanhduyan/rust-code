//! Business rules for AML and transaction limits
//!
//! These types represent business rules that can be evaluated
//! against transactions for compliance checks.

use rust_decimal::Decimal;
use simbank_core::AmlFlag;

/// A business rule with a condition and action
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
}

impl Rule {
    pub fn new(name: &str, condition: RuleCondition, action: RuleAction) -> Self {
        Self {
            name: name.to_string(),
            condition,
            action,
        }
    }

    /// Check if the rule condition matches the given transaction context
    pub fn matches(&self, ctx: &TransactionContext) -> bool {
        self.condition.evaluate(ctx)
    }

    /// Get the action to take if the rule matches
    pub fn action(&self) -> &RuleAction {
        &self.action
    }
}

/// Builder for constructing rules
#[derive(Debug)]
pub struct RuleBuilder {
    name: String,
    condition: Option<RuleCondition>,
    action: Option<RuleAction>,
}

impl RuleBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            condition: None,
            action: None,
        }
    }

    pub fn when(mut self, condition: RuleCondition) -> Self {
        self.condition = Some(condition);
        self
    }

    pub fn then(mut self, action: RuleAction) -> Self {
        self.action = Some(action);
        self
    }

    pub fn build(self) -> Rule {
        Rule {
            name: self.name,
            condition: self.condition.expect("Rule must have a condition"),
            action: self.action.expect("Rule must have an action"),
        }
    }
}

/// Conditions that can be evaluated against transactions
#[derive(Debug, Clone)]
pub enum RuleCondition {
    /// Amount is greater than threshold
    AmountGreaterThan {
        threshold: Decimal,
        currency: String,
    },
    /// Amount is greater than or equal to threshold
    AmountGreaterOrEqual {
        threshold: Decimal,
        currency: String,
    },
    /// Location is in the list of countries
    LocationIn {
        countries: Vec<String>,
    },
    /// Transaction type matches
    TransactionType {
        tx_type: String,
    },
    /// Combined conditions (AND)
    And(Box<RuleCondition>, Box<RuleCondition>),
    /// Combined conditions (OR)
    Or(Box<RuleCondition>, Box<RuleCondition>),
}

impl RuleCondition {
    /// Evaluate the condition against a transaction context
    pub fn evaluate(&self, ctx: &TransactionContext) -> bool {
        match self {
            RuleCondition::AmountGreaterThan { threshold, currency } => {
                ctx.currency.as_ref().map(|c| c == currency).unwrap_or(false)
                    && ctx.amount.map(|a| a > *threshold).unwrap_or(false)
            }
            RuleCondition::AmountGreaterOrEqual { threshold, currency } => {
                ctx.currency.as_ref().map(|c| c == currency).unwrap_or(false)
                    && ctx.amount.map(|a| a >= *threshold).unwrap_or(false)
            }
            RuleCondition::LocationIn { countries } => {
                ctx.location
                    .as_ref()
                    .map(|loc| countries.contains(loc))
                    .unwrap_or(false)
            }
            RuleCondition::TransactionType { tx_type } => {
                ctx.tx_type.as_ref().map(|t| t == tx_type).unwrap_or(false)
            }
            RuleCondition::And(left, right) => {
                left.evaluate(ctx) && right.evaluate(ctx)
            }
            RuleCondition::Or(left, right) => {
                left.evaluate(ctx) || right.evaluate(ctx)
            }
        }
    }

    /// Combine with another condition using AND
    pub fn and(self, other: RuleCondition) -> RuleCondition {
        RuleCondition::And(Box::new(self), Box::new(other))
    }

    /// Combine with another condition using OR
    pub fn or(self, other: RuleCondition) -> RuleCondition {
        RuleCondition::Or(Box::new(self), Box::new(other))
    }
}

/// Actions to take when a rule matches
#[derive(Debug, Clone)]
pub enum RuleAction {
    /// Flag the transaction for AML review
    FlagAml(String),
    /// Require manager approval
    RequireApproval,
    /// Block the transaction
    Block,
    /// Send notification
    Notify(String),
    /// Multiple actions
    Multiple(Vec<RuleAction>),
}

impl RuleAction {
    /// Convert action to AML flag if applicable
    pub fn to_aml_flag(&self) -> Option<AmlFlag> {
        match self {
            RuleAction::FlagAml(flag) => match flag.as_str() {
                "large_amount" => Some(AmlFlag::LargeAmount),
                "near_threshold" => Some(AmlFlag::NearThreshold),
                "unusual_pattern" => Some(AmlFlag::UnusualPattern),
                "high_risk_country" => Some(AmlFlag::HighRiskCountry),
                "cross_border" => Some(AmlFlag::CrossBorder),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Context for evaluating rules against a transaction
#[derive(Debug, Clone, Default)]
pub struct TransactionContext {
    pub amount: Option<Decimal>,
    pub currency: Option<String>,
    pub tx_type: Option<String>,
    pub location: Option<String>,
    pub ip_address: Option<String>,
    pub actor_id: Option<String>,
    pub account_id: Option<String>,
}

impl TransactionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_amount(mut self, amount: Decimal, currency: &str) -> Self {
        self.amount = Some(amount);
        self.currency = Some(currency.to_string());
        self
    }

    pub fn with_location(mut self, location: &str) -> Self {
        self.location = Some(location.to_string());
        self
    }

    pub fn with_tx_type(mut self, tx_type: &str) -> Self {
        self.tx_type = Some(tx_type.to_string());
        self
    }

    pub fn with_actor(mut self, actor_id: &str) -> Self {
        self.actor_id = Some(actor_id.to_string());
        self
    }

    pub fn with_account(mut self, account_id: &str) -> Self {
        self.account_id = Some(account_id.to_string());
        self
    }
}

/// A collection of rules that can be evaluated together
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Evaluate all rules against the context and return matching actions
    pub fn evaluate(&self, ctx: &TransactionContext) -> Vec<&RuleAction> {
        self.rules
            .iter()
            .filter(|rule| rule.matches(ctx))
            .map(|rule| rule.action())
            .collect()
    }

    /// Check if any rule would block the transaction
    pub fn is_blocked(&self, ctx: &TransactionContext) -> bool {
        self.evaluate(ctx)
            .iter()
            .any(|action| matches!(action, RuleAction::Block))
    }

    /// Check if any rule requires approval
    pub fn requires_approval(&self, ctx: &TransactionContext) -> bool {
        self.evaluate(ctx)
            .iter()
            .any(|action| matches!(action, RuleAction::RequireApproval))
    }

    /// Get all AML flags that should be applied
    pub fn get_aml_flags(&self, ctx: &TransactionContext) -> Vec<AmlFlag> {
        self.evaluate(ctx)
            .iter()
            .filter_map(|action| action.to_aml_flag())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_amount_condition() {
        let condition = RuleCondition::AmountGreaterThan {
            threshold: dec!(10000),
            currency: "USD".to_string(),
        };

        let ctx_match = TransactionContext::new()
            .with_amount(dec!(15000), "USD");
        assert!(condition.evaluate(&ctx_match));

        let ctx_no_match = TransactionContext::new()
            .with_amount(dec!(5000), "USD");
        assert!(!condition.evaluate(&ctx_no_match));

        let ctx_wrong_currency = TransactionContext::new()
            .with_amount(dec!(15000), "EUR");
        assert!(!condition.evaluate(&ctx_wrong_currency));
    }

    #[test]
    fn test_location_condition() {
        let condition = RuleCondition::LocationIn {
            countries: vec!["KP".to_string(), "IR".to_string(), "SY".to_string()],
        };

        let ctx_match = TransactionContext::new()
            .with_location("KP");
        assert!(condition.evaluate(&ctx_match));

        let ctx_no_match = TransactionContext::new()
            .with_location("US");
        assert!(!condition.evaluate(&ctx_no_match));
    }

    #[test]
    fn test_combined_conditions() {
        let large_amount = RuleCondition::AmountGreaterThan {
            threshold: dec!(10000),
            currency: "USD".to_string(),
        };
        let high_risk = RuleCondition::LocationIn {
            countries: vec!["KP".to_string()],
        };

        let combined = large_amount.and(high_risk);

        // Both conditions must match
        let ctx_both = TransactionContext::new()
            .with_amount(dec!(15000), "USD")
            .with_location("KP");
        assert!(combined.evaluate(&ctx_both));

        // Only amount matches
        let ctx_amount_only = TransactionContext::new()
            .with_amount(dec!(15000), "USD")
            .with_location("US");
        assert!(!combined.evaluate(&ctx_amount_only));
    }

    #[test]
    fn test_rule_evaluation() {
        let rule = Rule::new(
            "Large Transaction",
            RuleCondition::AmountGreaterThan {
                threshold: dec!(10000),
                currency: "USD".to_string(),
            },
            RuleAction::FlagAml("large_amount".to_string()),
        );

        let ctx = TransactionContext::new()
            .with_amount(dec!(15000), "USD");

        assert!(rule.matches(&ctx));
        assert!(matches!(rule.action(), RuleAction::FlagAml(_)));
    }

    #[test]
    fn test_ruleset() {
        let ruleset = RuleSet::new()
            .add(Rule::new(
                "Large Transaction",
                RuleCondition::AmountGreaterThan {
                    threshold: dec!(10000),
                    currency: "USD".to_string(),
                },
                RuleAction::FlagAml("large_amount".to_string()),
            ))
            .add(Rule::new(
                "High Risk Country",
                RuleCondition::LocationIn {
                    countries: vec!["KP".to_string(), "IR".to_string()],
                },
                RuleAction::Block,
            ));

        // Large transaction from safe country - flagged but not blocked
        let ctx_large = TransactionContext::new()
            .with_amount(dec!(15000), "USD")
            .with_location("US");
        assert!(!ruleset.is_blocked(&ctx_large));
        assert_eq!(ruleset.get_aml_flags(&ctx_large).len(), 1);

        // Transaction from high-risk country - blocked
        let ctx_risky = TransactionContext::new()
            .with_amount(dec!(100), "USD")
            .with_location("KP");
        assert!(ruleset.is_blocked(&ctx_risky));
    }

    #[test]
    fn test_aml_flag_conversion() {
        let action = RuleAction::FlagAml("large_amount".to_string());
        assert_eq!(action.to_aml_flag(), Some(AmlFlag::LargeAmount));

        let action = RuleAction::FlagAml("high_risk_country".to_string());
        assert_eq!(action.to_aml_flag(), Some(AmlFlag::HighRiskCountry));

        let action = RuleAction::Block;
        assert_eq!(action.to_aml_flag(), None);
    }
}
