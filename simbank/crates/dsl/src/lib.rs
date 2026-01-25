//! # Simbank DSL
//!
//! English DSL macros for Banking scenarios.
//!
//! ## Macros
//!
//! - [`banking_scenario!`] - Unified macro for defining workflows by stakeholder
//! - [`rule!`] - Business rules definition for AML and limits
//!
//! ## Example
//!
//! ```rust,ignore
//! use simbank_dsl::{banking_scenario, rule};
//!
//! banking_scenario! {
//!     Customer "Alice" {
//!         deposit 100 USDT to Funding;
//!         transfer 50 USDT from Funding to Spot;
//!     }
//!
//!     Employee "Bob" {
//!         receive_salary 5000 USD;
//!     }
//!
//!     Auditor "Deloitte" {
//!         scan from "2026-01-01" flags ["large_amount"];
//!     }
//! }
//!
//! rule! {
//!     name: "Large Transaction"
//!     when amount > 10000 USD
//!     then flag_aml "large_amount"
//! }
//! ```

pub mod scenario;
pub mod rules;

pub use scenario::{
    Scenario, ScenarioBuilder, StakeholderBlock, Operation,
    CustomerOp, EmployeeOp, AuditorOp, ShareholderOp, ManagerOp,
};
pub use rules::{Rule, RuleCondition, RuleAction, RuleBuilder};

// Re-export core types for DSL users
pub use simbank_core::{WalletType, PersonType, AmlFlag};
pub use rust_decimal::Decimal;

/// Main DSL macro for defining banking scenarios.
///
/// # Syntax
///
/// ```text
/// banking_scenario! {
///     <Stakeholder> "<name>" {
///         <operation>;
///         <operation>;
///     }
/// }
/// ```
///
/// ## Stakeholder Types
///
/// - `Customer` - deposit, withdraw, transfer
/// - `Employee` - receive_salary, buy_insurance
/// - `Shareholder` - receive_dividend
/// - `Manager` - pay_salary, pay_bonus, pay_dividend
/// - `Auditor` - scan, report
///
/// # Example
///
/// ```rust
/// use simbank_dsl::banking_scenario;
///
/// let scenario = banking_scenario! {
///     Customer "Alice" {
///         deposit 100 USDT to Funding;
///         transfer 50 USDT from Funding to Spot;
///         withdraw 20 USDT from Funding;
///     }
/// };
///
/// assert_eq!(scenario.blocks.len(), 1);
/// ```
#[macro_export]
macro_rules! banking_scenario {
    // Entry point - collect all stakeholder blocks
    (
        $(
            $stakeholder:ident $name:literal {
                $($op:tt)*
            }
        )*
    ) => {{
        let mut builder = $crate::ScenarioBuilder::new();
        $(
            builder = $crate::banking_scenario!(@block builder, $stakeholder, $name, $($op)*);
        )*
        builder.build()
    }};

    // --- Customer Operations ---
    (@block $builder:expr, Customer, $name:literal, $($op:tt)*) => {{
        let mut ops = Vec::new();
        $crate::banking_scenario!(@customer_ops ops, $($op)*);
        $builder.customer($name, ops)
    }};

    // Customer: deposit <amount> <currency> to <wallet>;
    (@customer_ops $ops:expr, deposit $amount:literal $currency:ident to Spot; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Deposit {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            to_wallet: $crate::WalletType::Spot,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};
    (@customer_ops $ops:expr, deposit $amount:literal $currency:ident to Funding; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Deposit {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            to_wallet: $crate::WalletType::Funding,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};
    (@customer_ops $ops:expr, deposit $amount:literal $currency:ident to Margin; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Deposit {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            to_wallet: $crate::WalletType::Margin,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};

    // Customer: withdraw <amount> <currency> from <wallet>;
    (@customer_ops $ops:expr, withdraw $amount:literal $currency:ident from Spot; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Withdraw {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            from_wallet: $crate::WalletType::Spot,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};
    (@customer_ops $ops:expr, withdraw $amount:literal $currency:ident from Funding; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Withdraw {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            from_wallet: $crate::WalletType::Funding,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};
    (@customer_ops $ops:expr, withdraw $amount:literal $currency:ident from Margin; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Withdraw {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            from_wallet: $crate::WalletType::Margin,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};

    // Customer: transfer <amount> <currency> from <wallet> to <wallet>;
    (@customer_ops $ops:expr, transfer $amount:literal $currency:ident from Funding to Spot; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Transfer {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            from_wallet: $crate::WalletType::Funding,
            to_wallet: $crate::WalletType::Spot,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};
    (@customer_ops $ops:expr, transfer $amount:literal $currency:ident from Spot to Funding; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Transfer {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            from_wallet: $crate::WalletType::Spot,
            to_wallet: $crate::WalletType::Funding,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};
    (@customer_ops $ops:expr, transfer $amount:literal $currency:ident from Spot to Margin; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Transfer {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            from_wallet: $crate::WalletType::Spot,
            to_wallet: $crate::WalletType::Margin,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};
    (@customer_ops $ops:expr, transfer $amount:literal $currency:ident from Margin to Spot; $($rest:tt)*) => {{
        $ops.push($crate::CustomerOp::Transfer {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            from_wallet: $crate::WalletType::Margin,
            to_wallet: $crate::WalletType::Spot,
        });
        $crate::banking_scenario!(@customer_ops $ops, $($rest)*);
    }};

    // Customer: end of operations
    (@customer_ops $ops:expr,) => {};

    // --- Employee Operations ---
    (@block $builder:expr, Employee, $name:literal, $($op:tt)*) => {{
        let mut ops = Vec::new();
        $crate::banking_scenario!(@employee_ops ops, $($op)*);
        $builder.employee($name, ops)
    }};

    // Employee: receive_salary <amount> <currency>;
    (@employee_ops $ops:expr, receive_salary $amount:literal $currency:ident; $($rest:tt)*) => {{
        $ops.push($crate::EmployeeOp::ReceiveSalary {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
        });
        $crate::banking_scenario!(@employee_ops $ops, $($rest)*);
    }};

    // Employee: buy_insurance <plan> for <amount> <currency>;
    (@employee_ops $ops:expr, buy_insurance $plan:literal for $amount:literal $currency:ident; $($rest:tt)*) => {{
        $ops.push($crate::EmployeeOp::BuyInsurance {
            plan: $plan.to_string(),
            cost: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
        });
        $crate::banking_scenario!(@employee_ops $ops, $($rest)*);
    }};

    // Employee: end of operations
    (@employee_ops $ops:expr,) => {};

    // --- Shareholder Operations ---
    (@block $builder:expr, Shareholder, $name:literal, $($op:tt)*) => {{
        let mut ops = Vec::new();
        $crate::banking_scenario!(@shareholder_ops ops, $($op)*);
        $builder.shareholder($name, ops)
    }};

    // Shareholder: receive_dividend <amount> <currency>;
    (@shareholder_ops $ops:expr, receive_dividend $amount:literal $currency:ident; $($rest:tt)*) => {{
        $ops.push($crate::ShareholderOp::ReceiveDividend {
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
        });
        $crate::banking_scenario!(@shareholder_ops $ops, $($rest)*);
    }};

    // Shareholder: end of operations
    (@shareholder_ops $ops:expr,) => {};

    // --- Manager Operations ---
    (@block $builder:expr, Manager, $name:literal, $($op:tt)*) => {{
        let mut ops = Vec::new();
        $crate::banking_scenario!(@manager_ops ops, $($op)*);
        $builder.manager($name, ops)
    }};

    // Manager: pay_salary to <employee> amount <amount> <currency>;
    (@manager_ops $ops:expr, pay_salary to $employee:literal amount $amount:literal $currency:ident; $($rest:tt)*) => {{
        $ops.push($crate::ManagerOp::PaySalary {
            employee_account: $employee.to_string(),
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
        });
        $crate::banking_scenario!(@manager_ops $ops, $($rest)*);
    }};

    // Manager: pay_bonus to <employee> amount <amount> <currency> reason <reason>;
    (@manager_ops $ops:expr, pay_bonus to $employee:literal amount $amount:literal $currency:ident reason $reason:literal; $($rest:tt)*) => {{
        $ops.push($crate::ManagerOp::PayBonus {
            employee_account: $employee.to_string(),
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
            reason: $reason.to_string(),
        });
        $crate::banking_scenario!(@manager_ops $ops, $($rest)*);
    }};

    // Manager: pay_dividend to <shareholder> amount <amount> <currency>;
    (@manager_ops $ops:expr, pay_dividend to $shareholder:literal amount $amount:literal $currency:ident; $($rest:tt)*) => {{
        $ops.push($crate::ManagerOp::PayDividend {
            shareholder_account: $shareholder.to_string(),
            amount: rust_decimal_macros::dec!($amount),
            currency: stringify!($currency).to_string(),
        });
        $crate::banking_scenario!(@manager_ops $ops, $($rest)*);
    }};

    // Manager: end of operations
    (@manager_ops $ops:expr,) => {};

    // --- Auditor Operations ---
    (@block $builder:expr, Auditor, $name:literal, $($op:tt)*) => {{
        let mut ops = Vec::new();
        $crate::banking_scenario!(@auditor_ops ops, $($op)*);
        $builder.auditor($name, ops)
    }};

    // Auditor: scan from <date> flags [<flags>];
    (@auditor_ops $ops:expr, scan from $from:literal flags [$($flag:literal),*]; $($rest:tt)*) => {{
        $ops.push($crate::AuditorOp::Scan {
            from_date: Some($from.to_string()),
            to_date: None,
            flags: vec![$($flag.to_string()),*],
        });
        $crate::banking_scenario!(@auditor_ops $ops, $($rest)*);
    }};

    // Auditor: scan from <date> to <date> flags [<flags>];
    (@auditor_ops $ops:expr, scan from $from:literal to $to:literal flags [$($flag:literal),*]; $($rest:tt)*) => {{
        $ops.push($crate::AuditorOp::Scan {
            from_date: Some($from.to_string()),
            to_date: Some($to.to_string()),
            flags: vec![$($flag.to_string()),*],
        });
        $crate::banking_scenario!(@auditor_ops $ops, $($rest)*);
    }};

    // Auditor: report <format>;
    (@auditor_ops $ops:expr, report $format:ident; $($rest:tt)*) => {{
        $ops.push($crate::AuditorOp::Report {
            format: stringify!($format).to_string(),
        });
        $crate::banking_scenario!(@auditor_ops $ops, $($rest)*);
    }};

    // Auditor: end of operations
    (@auditor_ops $ops:expr,) => {};
}

/// Business rule definition macro.
///
/// # Syntax
///
/// ```text
/// rule! {
///     name: "<rule_name>"
///     when <condition>
///     then <action>
/// }
/// ```
///
/// ## Conditions
///
/// - `amount > <value> <currency>` - Amount threshold
/// - `amount >= <value> <currency>` - Amount threshold (inclusive)
/// - `location in [<countries>]` - High-risk country check
///
/// ## Actions
///
/// - `flag_aml "<flag>"` - Add AML flag to transaction
/// - `require_approval` - Require manager approval
/// - `block` - Block the transaction
///
/// # Example
///
/// ```rust
/// use simbank_dsl::rule;
///
/// let aml_rule = rule! {
///     name: "Large Transaction"
///     when amount > 10000 USD
///     then flag_aml "large_amount"
/// };
///
/// assert_eq!(aml_rule.name, "Large Transaction");
/// ```
#[macro_export]
macro_rules! rule {
    // Amount > threshold
    (
        name: $name:literal
        when amount > $threshold:literal $currency:ident
        then $action:ident $($action_args:tt)*
    ) => {{
        $crate::RuleBuilder::new($name)
            .when($crate::RuleCondition::AmountGreaterThan {
                threshold: rust_decimal_macros::dec!($threshold),
                currency: stringify!($currency).to_string(),
            })
            .then($crate::rule!(@action $action $($action_args)*))
            .build()
    }};

    // Amount >= threshold
    (
        name: $name:literal
        when amount >= $threshold:literal $currency:ident
        then $action:ident $($action_args:tt)*
    ) => {{
        $crate::RuleBuilder::new($name)
            .when($crate::RuleCondition::AmountGreaterOrEqual {
                threshold: rust_decimal_macros::dec!($threshold),
                currency: stringify!($currency).to_string(),
            })
            .then($crate::rule!(@action $action $($action_args)*))
            .build()
    }};

    // Location in countries
    (
        name: $name:literal
        when location in [$($country:literal),*]
        then $action:ident $($action_args:tt)*
    ) => {{
        $crate::RuleBuilder::new($name)
            .when($crate::RuleCondition::LocationIn {
                countries: vec![$($country.to_string()),*],
            })
            .then($crate::rule!(@action $action $($action_args)*))
            .build()
    }};

    // Action: flag_aml
    (@action flag_aml $flag:literal) => {
        $crate::RuleAction::FlagAml($flag.to_string())
    };

    // Action: require_approval
    (@action require_approval) => {
        $crate::RuleAction::RequireApproval
    };

    // Action: block
    (@action block) => {
        $crate::RuleAction::Block
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customer_scenario() {
        let scenario = banking_scenario! {
            Customer "Alice" {
                deposit 100 USDT to Funding;
                transfer 50 USDT from Funding to Spot;
                withdraw 20 USDT from Funding;
            }
        };

        assert_eq!(scenario.blocks.len(), 1);
        if let StakeholderBlock::Customer { name, operations } = &scenario.blocks[0] {
            assert_eq!(name, "Alice");
            assert_eq!(operations.len(), 3);
        } else {
            panic!("Expected Customer block");
        }
    }

    #[test]
    fn test_employee_scenario() {
        let scenario = banking_scenario! {
            Employee "Bob" {
                receive_salary 5000 USD;
                buy_insurance "Health Premium" for 200 USD;
            }
        };

        assert_eq!(scenario.blocks.len(), 1);
        if let StakeholderBlock::Employee { name, operations } = &scenario.blocks[0] {
            assert_eq!(name, "Bob");
            assert_eq!(operations.len(), 2);
        } else {
            panic!("Expected Employee block");
        }
    }

    #[test]
    fn test_auditor_scenario() {
        let scenario = banking_scenario! {
            Auditor "Deloitte" {
                scan from "2026-01-01" flags ["large_amount", "high_risk_country"];
                report Markdown;
            }
        };

        assert_eq!(scenario.blocks.len(), 1);
        if let StakeholderBlock::Auditor { name, operations } = &scenario.blocks[0] {
            assert_eq!(name, "Deloitte");
            assert_eq!(operations.len(), 2);
        } else {
            panic!("Expected Auditor block");
        }
    }

    #[test]
    fn test_multi_stakeholder_scenario() {
        let scenario = banking_scenario! {
            Customer "Alice" {
                deposit 100 USDT to Funding;
            }
            Manager "CEO" {
                pay_salary to "ACC_EMP_001" amount 5000 USD;
                pay_bonus to "ACC_EMP_001" amount 1000 USD reason "Q4 Performance";
            }
            Auditor "PwC" {
                scan from "2026-01-01" flags ["large_amount"];
            }
        };

        assert_eq!(scenario.blocks.len(), 3);
    }

    #[test]
    fn test_aml_rule() {
        let rule = rule! {
            name: "Large Transaction"
            when amount > 10000 USD
            then flag_aml "large_amount"
        };

        assert_eq!(rule.name, "Large Transaction");
        assert!(matches!(rule.condition, RuleCondition::AmountGreaterThan { .. }));
        assert!(matches!(rule.action, RuleAction::FlagAml(_)));
    }

    #[test]
    fn test_location_rule() {
        let rule = rule! {
            name: "High Risk Country"
            when location in ["KP", "IR", "SY"]
            then block
        };

        assert_eq!(rule.name, "High Risk Country");
        if let RuleCondition::LocationIn { countries } = &rule.condition {
            assert_eq!(countries.len(), 3);
            assert!(countries.contains(&"KP".to_string()));
        } else {
            panic!("Expected LocationIn condition");
        }
        assert!(matches!(rule.action, RuleAction::Block));
    }

    #[test]
    fn test_approval_rule() {
        let rule = rule! {
            name: "Large Withdrawal"
            when amount >= 50000 USD
            then require_approval
        };

        assert_eq!(rule.name, "Large Withdrawal");
        assert!(matches!(rule.action, RuleAction::RequireApproval));
    }
}
