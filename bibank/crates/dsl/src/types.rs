//! Rule types for the DSL
//!
//! Defines the core types used by the rule! and rule_set! macros.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use bibank_compliance::{AmlDecision, ApprovalLevel, RiskScore};

/// Action to take when a rule triggers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleAction {
    /// Block the transaction (pre-validation)
    Block {
        /// Compliance code
        code: String,
        /// Human-readable reason
        reason: String,
    },
    /// Flag the transaction for review (post-commit)
    Flag {
        /// Risk score
        risk_score: RiskScore,
        /// Required approval level
        approval_level: ApprovalLevel,
        /// Reason for flagging
        reason: String,
    },
    /// Approve (no action needed)
    Approve,
}

impl RuleAction {
    /// Create a block action
    pub fn block(code: impl Into<String>, reason: impl Into<String>) -> Self {
        RuleAction::Block {
            code: code.into(),
            reason: reason.into(),
        }
    }

    /// Create a flag action
    pub fn flag(
        risk_score: RiskScore,
        approval_level: ApprovalLevel,
        reason: impl Into<String>,
    ) -> Self {
        RuleAction::Flag {
            risk_score,
            approval_level,
            reason: reason.into(),
        }
    }

    /// Check if this is a block action
    pub fn is_block(&self) -> bool {
        matches!(self, RuleAction::Block { .. })
    }

    /// Check if this is a flag action
    pub fn is_flag(&self) -> bool {
        matches!(self, RuleAction::Flag { .. })
    }

    /// Convert to AmlDecision
    pub fn to_decision(&self) -> AmlDecision {
        match self {
            RuleAction::Block { code, reason } => AmlDecision::blocked(reason, code),
            RuleAction::Flag {
                risk_score,
                approval_level,
                reason,
            } => AmlDecision::flagged(reason, *risk_score, *approval_level),
            RuleAction::Approve => AmlDecision::Approved,
        }
    }
}

/// Condition operators for rule matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Condition {
    /// Amount >= threshold
    AmountGte { threshold: Decimal },
    /// Amount < threshold
    AmountLt { threshold: Decimal },
    /// Amount in range [min, max)
    AmountInRange { min: Decimal, max: Decimal },
    /// Account age < days
    AccountAgeLt { days: i64 },
    /// Account age >= days
    AccountAgeGte { days: i64 },
    /// User is on watchlist
    IsWatchlisted,
    /// User is a PEP
    IsPep,
    /// Transaction count in window >= count
    TxCountGte { count: u32, window_minutes: u32 },
    /// Total volume in window >= threshold
    VolumeGte { threshold: Decimal, window_minutes: u32 },
    /// Custom condition with name
    Custom { name: String },
    /// All conditions must match (AND)
    All { conditions: Vec<Condition> },
    /// Any condition must match (OR)
    Any { conditions: Vec<Condition> },
}

impl Condition {
    /// Amount >= threshold
    pub fn amount_gte(threshold: Decimal) -> Self {
        Condition::AmountGte { threshold }
    }

    /// Amount < threshold
    pub fn amount_lt(threshold: Decimal) -> Self {
        Condition::AmountLt { threshold }
    }

    /// Amount in range [min, max)
    pub fn amount_in_range(min: Decimal, max: Decimal) -> Self {
        Condition::AmountInRange { min, max }
    }

    /// Account age < days
    pub fn account_age_lt(days: i64) -> Self {
        Condition::AccountAgeLt { days }
    }

    /// User is on watchlist
    pub fn is_watchlisted() -> Self {
        Condition::IsWatchlisted
    }

    /// User is a PEP
    pub fn is_pep() -> Self {
        Condition::IsPep
    }

    /// Transaction count >= count in window
    pub fn tx_count_gte(count: u32, window_minutes: u32) -> Self {
        Condition::TxCountGte {
            count,
            window_minutes,
        }
    }

    /// Volume >= threshold in window
    pub fn volume_gte(threshold: Decimal, window_minutes: u32) -> Self {
        Condition::VolumeGte {
            threshold,
            window_minutes,
        }
    }

    /// All conditions (AND)
    pub fn all(conditions: Vec<Condition>) -> Self {
        Condition::All { conditions }
    }

    /// Any condition (OR)
    pub fn any(conditions: Vec<Condition>) -> Self {
        Condition::Any { conditions }
    }
}

/// A single compliance rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDefinition {
    /// Unique rule ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Whether this is a BLOCK (pre-validation) or FLAG (post-commit) rule
    pub rule_type: RuleType,
    /// Condition that triggers this rule
    pub condition: Condition,
    /// Action to take when triggered
    pub action: RuleAction,
    /// Priority (lower = runs first)
    pub priority: u32,
    /// Whether this rule is enabled
    pub enabled: bool,
}

/// Type of rule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    /// Pre-validation rule (can BLOCK)
    Block,
    /// Post-commit rule (can FLAG)
    Flag,
}

impl RuleDefinition {
    /// Create a new rule definition builder
    pub fn builder(id: impl Into<String>) -> RuleBuilder {
        RuleBuilder::new(id)
    }
}

/// Builder for RuleDefinition
pub struct RuleBuilder {
    id: String,
    name: Option<String>,
    description: String,
    rule_type: RuleType,
    condition: Option<Condition>,
    action: Option<RuleAction>,
    priority: u32,
    enabled: bool,
}

impl RuleBuilder {
    /// Create a new builder
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            description: String::new(),
            rule_type: RuleType::Flag,
            condition: None,
            action: None,
            priority: 100,
            enabled: true,
        }
    }

    /// Set the rule name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set as a BLOCK rule
    pub fn block_rule(mut self) -> Self {
        self.rule_type = RuleType::Block;
        self
    }

    /// Set as a FLAG rule
    pub fn flag_rule(mut self) -> Self {
        self.rule_type = RuleType::Flag;
        self
    }

    /// Set the condition
    pub fn when(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Set the action
    pub fn then(mut self, action: RuleAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Set priority
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Build the rule definition
    pub fn build(self) -> RuleDefinition {
        RuleDefinition {
            id: self.id.clone(),
            name: self.name.unwrap_or_else(|| self.id.clone()),
            description: self.description,
            rule_type: self.rule_type,
            condition: self.condition.expect("condition is required"),
            action: self.action.expect("action is required"),
            priority: self.priority,
            enabled: self.enabled,
        }
    }
}

/// A set of rules
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    /// Name of this rule set
    pub name: String,
    /// Description
    pub description: String,
    /// Rules in this set
    pub rules: Vec<RuleDefinition>,
}

impl RuleSet {
    /// Create a new empty rule set
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            rules: Vec::new(),
        }
    }

    /// Add a description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add a rule
    pub fn add_rule(mut self, rule: RuleDefinition) -> Self {
        self.rules.push(rule);
        self
    }

    /// Add multiple rules
    pub fn add_rules(mut self, rules: Vec<RuleDefinition>) -> Self {
        self.rules.extend(rules);
        self
    }

    /// Get all BLOCK rules sorted by priority
    pub fn block_rules(&self) -> Vec<&RuleDefinition> {
        let mut rules: Vec<_> = self
            .rules
            .iter()
            .filter(|r| r.rule_type == RuleType::Block && r.enabled)
            .collect();
        rules.sort_by_key(|r| r.priority);
        rules
    }

    /// Get all FLAG rules sorted by priority
    pub fn flag_rules(&self) -> Vec<&RuleDefinition> {
        let mut rules: Vec<_> = self
            .rules
            .iter()
            .filter(|r| r.rule_type == RuleType::Flag && r.enabled)
            .collect();
        rules.sort_by_key(|r| r.priority);
        rules
    }

    /// Get rule by ID
    pub fn get_rule(&self, id: &str) -> Option<&RuleDefinition> {
        self.rules.iter().find(|r| r.id == id)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_rule_action_block() {
        let action = RuleAction::block("SANCTIONS", "User on watchlist");
        assert!(action.is_block());
        assert!(!action.is_flag());

        let decision = action.to_decision();
        assert!(decision.is_blocked());
    }

    #[test]
    fn test_rule_action_flag() {
        let action = RuleAction::flag(RiskScore::High, ApprovalLevel::L2, "Large transaction");
        assert!(action.is_flag());
        assert!(!action.is_block());

        let decision = action.to_decision();
        assert!(decision.is_flagged());
    }

    #[test]
    fn test_condition_amount() {
        let cond = Condition::amount_gte(dec!(10000));
        assert!(matches!(cond, Condition::AmountGte { threshold } if threshold == dec!(10000)));
    }

    #[test]
    fn test_condition_all() {
        let cond = Condition::all(vec![
            Condition::amount_gte(dec!(5000)),
            Condition::account_age_lt(7),
        ]);

        if let Condition::All { conditions } = cond {
            assert_eq!(conditions.len(), 2);
        } else {
            panic!("Expected All condition");
        }
    }

    #[test]
    fn test_rule_builder() {
        let rule = RuleDefinition::builder("LARGE_TX")
            .name("Large Transaction Alert")
            .description("Flag transactions >= 10,000")
            .flag_rule()
            .when(Condition::amount_gte(dec!(10000)))
            .then(RuleAction::flag(
                RiskScore::Medium,
                ApprovalLevel::L1,
                "Large transaction detected",
            ))
            .priority(50)
            .build();

        assert_eq!(rule.id, "LARGE_TX");
        assert_eq!(rule.name, "Large Transaction Alert");
        assert_eq!(rule.rule_type, RuleType::Flag);
        assert_eq!(rule.priority, 50);
        assert!(rule.enabled);
    }

    #[test]
    fn test_rule_set() {
        let ruleset = RuleSet::new("AML_BASIC")
            .with_description("Basic AML rules")
            .add_rule(
                RuleDefinition::builder("SANCTIONS")
                    .block_rule()
                    .when(Condition::is_watchlisted())
                    .then(RuleAction::block("SANCTIONS", "User on watchlist"))
                    .priority(10)
                    .build(),
            )
            .add_rule(
                RuleDefinition::builder("LARGE_TX")
                    .flag_rule()
                    .when(Condition::amount_gte(dec!(10000)))
                    .then(RuleAction::flag(
                        RiskScore::Medium,
                        ApprovalLevel::L1,
                        "Large transaction",
                    ))
                    .priority(50)
                    .build(),
            );

        assert_eq!(ruleset.name, "AML_BASIC");
        assert_eq!(ruleset.rules.len(), 2);
        assert_eq!(ruleset.block_rules().len(), 1);
        assert_eq!(ruleset.flag_rules().len(), 1);
    }

    #[test]
    fn test_rule_set_get_by_id() {
        let ruleset = RuleSet::new("TEST")
            .add_rule(
                RuleDefinition::builder("RULE_1")
                    .flag_rule()
                    .when(Condition::amount_gte(dec!(100)))
                    .then(RuleAction::Approve)
                    .build(),
            );

        assert!(ruleset.get_rule("RULE_1").is_some());
        assert!(ruleset.get_rule("RULE_2").is_none());
    }

    #[test]
    fn test_rule_serialization() {
        let rule = RuleDefinition::builder("TEST")
            .when(Condition::amount_gte(dec!(1000)))
            .then(RuleAction::Approve)
            .build();

        let json = serde_json::to_string(&rule).unwrap();
        let parsed: RuleDefinition = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, rule.id);
    }
}
