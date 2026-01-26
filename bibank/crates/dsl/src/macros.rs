//! Rule and RuleSet macros for declarative compliance rule definition
//!
//! # Example
//!
//! ```ignore
//! use bibank_dsl::{rule, rule_set};
//!
//! let sanctions_rule = rule! {
//!     id: "SANCTIONS_CHECK",
//!     name: "Sanctions Watchlist Check",
//!     type: block,
//!     when: is_watchlisted,
//!     then: block("SANCTIONS", "User on sanctions watchlist"),
//!     priority: 10,
//! };
//!
//! let large_tx_rule = rule! {
//!     id: "LARGE_TX_ALERT",
//!     type: flag,
//!     when: amount >= 10_000,
//!     then: flag(Medium, L1, "Large transaction detected"),
//! };
//!
//! let ruleset = rule_set! {
//!     name: "AML_BASIC",
//!     description: "Basic AML compliance rules",
//!     rules: [sanctions_rule, large_tx_rule],
//! };
//! ```

/// Create a compliance rule definition
///
/// # Syntax
///
/// ```ignore
/// rule! {
///     id: "RULE_ID",                          // Required: unique identifier
///     name: "Human readable name",            // Optional: defaults to id
///     description: "What this rule does",     // Optional
///     type: block | flag,                     // Required: rule type
///     when: <condition>,                      // Required: trigger condition
///     then: <action>,                         // Required: action to take
///     priority: 100,                          // Optional: lower = runs first
///     enabled: true,                          // Optional: defaults to true
/// }
/// ```
///
/// # Conditions
///
/// - `amount >= <value>` - Amount greater than or equal
/// - `amount < <value>` - Amount less than
/// - `amount in <min>..<max>` - Amount in range
/// - `account_age < <days>` - Account younger than days
/// - `account_age >= <days>` - Account older than days
/// - `is_watchlisted` - User on watchlist
/// - `is_pep` - User is PEP
/// - `tx_count >= <n> in <minutes>m` - Transaction count in window
/// - `volume >= <amount> in <minutes>m` - Volume in window
/// - `all(<cond1>, <cond2>, ...)` - All conditions must match
/// - `any(<cond1>, <cond2>, ...)` - Any condition must match
///
/// # Actions
///
/// - `block("<code>", "<reason>")` - Block the transaction
/// - `flag(<RiskScore>, <ApprovalLevel>, "<reason>")` - Flag for review
/// - `approve` - Approve (no action)
#[macro_export]
macro_rules! rule {
    // Full syntax with all fields
    (
        id: $id:expr,
        $(name: $name:expr,)?
        $(description: $desc:expr,)?
        type: block,
        when: $cond:expr,
        then: $action:expr
        $(, priority: $priority:expr)?
        $(, enabled: $enabled:expr)?
        $(,)?
    ) => {{
        $crate::types::RuleDefinition::builder($id)
            $(.name($name))?
            $(.description($desc))?
            .block_rule()
            .when($cond)
            .then($action)
            $(.priority($priority))?
            $(.enabled($enabled))?
            .build()
    }};

    // Flag rule
    (
        id: $id:expr,
        $(name: $name:expr,)?
        $(description: $desc:expr,)?
        type: flag,
        when: $cond:expr,
        then: $action:expr
        $(, priority: $priority:expr)?
        $(, enabled: $enabled:expr)?
        $(,)?
    ) => {{
        $crate::types::RuleDefinition::builder($id)
            $(.name($name))?
            $(.description($desc))?
            .flag_rule()
            .when($cond)
            .then($action)
            $(.priority($priority))?
            $(.enabled($enabled))?
            .build()
    }};
}

/// Create a rule set containing multiple rules
///
/// # Syntax
///
/// ```ignore
/// rule_set! {
///     name: "RULESET_NAME",
///     description: "What this ruleset does",  // Optional
///     rules: [rule1, rule2, ...],
/// }
/// ```
#[macro_export]
macro_rules! rule_set {
    (
        name: $name:expr,
        $(description: $desc:expr,)?
        rules: [$($rule:expr),* $(,)?]
        $(,)?
    ) => {{
        let mut ruleset = $crate::types::RuleSet::new($name);
        $(ruleset = ruleset.with_description($desc);)?
        $(ruleset = ruleset.add_rule($rule);)*
        ruleset
    }};
}

/// Shorthand for creating a block action
#[macro_export]
macro_rules! block_action {
    ($code:expr, $reason:expr) => {
        $crate::types::RuleAction::block($code, $reason)
    };
}

/// Shorthand for creating a flag action
#[macro_export]
macro_rules! flag_action {
    ($risk:ident, $level:ident, $reason:expr) => {
        $crate::types::RuleAction::flag(
            $crate::RiskScore::$risk,
            $crate::ApprovalLevel::$level,
            $reason,
        )
    };
}

/// Shorthand for amount >= condition
#[macro_export]
macro_rules! amount_gte {
    ($threshold:expr) => {
        $crate::types::Condition::amount_gte($threshold)
    };
}

/// Shorthand for amount < condition
#[macro_export]
macro_rules! amount_lt {
    ($threshold:expr) => {
        $crate::types::Condition::amount_lt($threshold)
    };
}

/// Shorthand for account_age < days condition
#[macro_export]
macro_rules! account_age_lt {
    ($days:expr) => {
        $crate::types::Condition::account_age_lt($days)
    };
}

/// Shorthand for is_watchlisted condition
#[macro_export]
macro_rules! is_watchlisted {
    () => {
        $crate::types::Condition::is_watchlisted()
    };
}

/// Shorthand for is_pep condition
#[macro_export]
macro_rules! is_pep {
    () => {
        $crate::types::Condition::is_pep()
    };
}

/// Shorthand for all conditions (AND)
#[macro_export]
macro_rules! all_of {
    ($($cond:expr),+ $(,)?) => {
        $crate::types::Condition::all(vec![$($cond),+])
    };
}

/// Shorthand for any conditions (OR)
#[macro_export]
macro_rules! any_of {
    ($($cond:expr),+ $(,)?) => {
        $crate::types::Condition::any(vec![$($cond),+])
    };
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Condition, RuleAction, RuleType};
    use bibank_compliance::{ApprovalLevel, RiskScore};
    use rust_decimal_macros::dec;

    #[test]
    fn test_rule_macro_block() {
        let rule = rule! {
            id: "SANCTIONS",
            name: "Sanctions Check",
            type: block,
            when: Condition::is_watchlisted(),
            then: RuleAction::block("SANCTIONS", "User on watchlist"),
            priority: 10,
        };

        assert_eq!(rule.id, "SANCTIONS");
        assert_eq!(rule.name, "Sanctions Check");
        assert_eq!(rule.rule_type, RuleType::Block);
        assert_eq!(rule.priority, 10);
    }

    #[test]
    fn test_rule_macro_flag() {
        let rule = rule! {
            id: "LARGE_TX",
            type: flag,
            when: Condition::amount_gte(dec!(10000)),
            then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large tx"),
        };

        assert_eq!(rule.id, "LARGE_TX");
        assert_eq!(rule.rule_type, RuleType::Flag);
        assert!(rule.enabled);
    }

    #[test]
    fn test_rule_set_macro() {
        let sanctions = rule! {
            id: "SANCTIONS",
            type: block,
            when: Condition::is_watchlisted(),
            then: RuleAction::block("SANCTIONS", "Watchlisted"),
        };

        let large_tx = rule! {
            id: "LARGE_TX",
            type: flag,
            when: Condition::amount_gte(dec!(10000)),
            then: RuleAction::flag(RiskScore::Medium, ApprovalLevel::L1, "Large"),
        };

        let ruleset = rule_set! {
            name: "AML_BASIC",
            description: "Basic AML rules",
            rules: [sanctions, large_tx],
        };

        assert_eq!(ruleset.name, "AML_BASIC");
        assert_eq!(ruleset.rules.len(), 2);
    }

    #[test]
    fn test_shorthand_macros() {
        let cond = amount_gte!(dec!(5000));
        assert!(matches!(cond, Condition::AmountGte { .. }));

        let cond2 = account_age_lt!(7);
        assert!(matches!(cond2, Condition::AccountAgeLt { days: 7 }));

        let cond3 = is_watchlisted!();
        assert!(matches!(cond3, Condition::IsWatchlisted));
    }

    #[test]
    fn test_all_of_macro() {
        let cond = all_of!(
            amount_gte!(dec!(5000)),
            account_age_lt!(7),
        );

        if let Condition::All { conditions } = cond {
            assert_eq!(conditions.len(), 2);
        } else {
            panic!("Expected All condition");
        }
    }

    #[test]
    fn test_any_of_macro() {
        let cond = any_of!(
            is_watchlisted!(),
            is_pep!(),
        );

        if let Condition::Any { conditions } = cond {
            assert_eq!(conditions.len(), 2);
        } else {
            panic!("Expected Any condition");
        }
    }

    #[test]
    fn test_complex_rule_with_macros() {
        let rule = rule! {
            id: "NEW_ACCOUNT_LARGE_TX",
            name: "New Account Large Transaction",
            description: "Flag large transactions from new accounts",
            type: flag,
            when: all_of!(
                amount_gte!(dec!(5000)),
                account_age_lt!(7)
            ),
            then: RuleAction::flag(
                RiskScore::High,
                ApprovalLevel::L2,
                "New account with large transaction"
            ),
            priority: 30,
        };

        assert_eq!(rule.id, "NEW_ACCOUNT_LARGE_TX");
        assert_eq!(rule.priority, 30);

        if let Condition::All { conditions } = &rule.condition {
            assert_eq!(conditions.len(), 2);
        }
    }
}
