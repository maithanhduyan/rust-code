//! Scenario types for banking_scenario! macro
//!
//! These types represent the parsed scenario structure that can be
//! executed against the business layer.

use rust_decimal::Decimal;
use simbank_core::WalletType;

/// A complete banking scenario with multiple stakeholder blocks
#[derive(Debug, Clone)]
pub struct Scenario {
    pub blocks: Vec<StakeholderBlock>,
}

impl Scenario {
    pub fn new(blocks: Vec<StakeholderBlock>) -> Self {
        Self { blocks }
    }

    /// Get all customer blocks
    pub fn customers(&self) -> impl Iterator<Item = (&str, &Vec<CustomerOp>)> {
        self.blocks.iter().filter_map(|b| {
            if let StakeholderBlock::Customer { name, operations } = b {
                Some((name.as_str(), operations))
            } else {
                None
            }
        })
    }

    /// Get all employee blocks
    pub fn employees(&self) -> impl Iterator<Item = (&str, &Vec<EmployeeOp>)> {
        self.blocks.iter().filter_map(|b| {
            if let StakeholderBlock::Employee { name, operations } = b {
                Some((name.as_str(), operations))
            } else {
                None
            }
        })
    }

    /// Get all auditor blocks
    pub fn auditors(&self) -> impl Iterator<Item = (&str, &Vec<AuditorOp>)> {
        self.blocks.iter().filter_map(|b| {
            if let StakeholderBlock::Auditor { name, operations } = b {
                Some((name.as_str(), operations))
            } else {
                None
            }
        })
    }

    /// Get all shareholder blocks
    pub fn shareholders(&self) -> impl Iterator<Item = (&str, &Vec<ShareholderOp>)> {
        self.blocks.iter().filter_map(|b| {
            if let StakeholderBlock::Shareholder { name, operations } = b {
                Some((name.as_str(), operations))
            } else {
                None
            }
        })
    }

    /// Get all manager blocks
    pub fn managers(&self) -> impl Iterator<Item = (&str, &Vec<ManagerOp>)> {
        self.blocks.iter().filter_map(|b| {
            if let StakeholderBlock::Manager { name, operations } = b {
                Some((name.as_str(), operations))
            } else {
                None
            }
        })
    }
}

/// Builder for constructing scenarios
#[derive(Debug, Default)]
pub struct ScenarioBuilder {
    blocks: Vec<StakeholderBlock>,
}

impl ScenarioBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn customer(mut self, name: &str, operations: Vec<CustomerOp>) -> Self {
        self.blocks.push(StakeholderBlock::Customer {
            name: name.to_string(),
            operations,
        });
        self
    }

    pub fn employee(mut self, name: &str, operations: Vec<EmployeeOp>) -> Self {
        self.blocks.push(StakeholderBlock::Employee {
            name: name.to_string(),
            operations,
        });
        self
    }

    pub fn shareholder(mut self, name: &str, operations: Vec<ShareholderOp>) -> Self {
        self.blocks.push(StakeholderBlock::Shareholder {
            name: name.to_string(),
            operations,
        });
        self
    }

    pub fn manager(mut self, name: &str, operations: Vec<ManagerOp>) -> Self {
        self.blocks.push(StakeholderBlock::Manager {
            name: name.to_string(),
            operations,
        });
        self
    }

    pub fn auditor(mut self, name: &str, operations: Vec<AuditorOp>) -> Self {
        self.blocks.push(StakeholderBlock::Auditor {
            name: name.to_string(),
            operations,
        });
        self
    }

    pub fn build(self) -> Scenario {
        Scenario::new(self.blocks)
    }
}

/// A block of operations for a specific stakeholder
#[derive(Debug, Clone)]
pub enum StakeholderBlock {
    Customer {
        name: String,
        operations: Vec<CustomerOp>,
    },
    Employee {
        name: String,
        operations: Vec<EmployeeOp>,
    },
    Shareholder {
        name: String,
        operations: Vec<ShareholderOp>,
    },
    Manager {
        name: String,
        operations: Vec<ManagerOp>,
    },
    Auditor {
        name: String,
        operations: Vec<AuditorOp>,
    },
}

/// Generic operation trait
pub trait Operation {
    fn description(&self) -> String;
}

// ============================================================================
// Customer Operations
// ============================================================================

#[derive(Debug, Clone)]
pub enum CustomerOp {
    Deposit {
        amount: Decimal,
        currency: String,
        to_wallet: WalletType,
    },
    Withdraw {
        amount: Decimal,
        currency: String,
        from_wallet: WalletType,
    },
    Transfer {
        amount: Decimal,
        currency: String,
        from_wallet: WalletType,
        to_wallet: WalletType,
    },
}

impl Operation for CustomerOp {
    fn description(&self) -> String {
        match self {
            CustomerOp::Deposit { amount, currency, to_wallet } => {
                format!("Deposit {} {} to {:?}", amount, currency, to_wallet)
            }
            CustomerOp::Withdraw { amount, currency, from_wallet } => {
                format!("Withdraw {} {} from {:?}", amount, currency, from_wallet)
            }
            CustomerOp::Transfer { amount, currency, from_wallet, to_wallet } => {
                format!("Transfer {} {} from {:?} to {:?}", amount, currency, from_wallet, to_wallet)
            }
        }
    }
}

// ============================================================================
// Employee Operations
// ============================================================================

#[derive(Debug, Clone)]
pub enum EmployeeOp {
    ReceiveSalary {
        amount: Decimal,
        currency: String,
    },
    BuyInsurance {
        plan: String,
        cost: Decimal,
        currency: String,
    },
}

impl Operation for EmployeeOp {
    fn description(&self) -> String {
        match self {
            EmployeeOp::ReceiveSalary { amount, currency } => {
                format!("Receive salary {} {}", amount, currency)
            }
            EmployeeOp::BuyInsurance { plan, cost, currency } => {
                format!("Buy insurance '{}' for {} {}", plan, cost, currency)
            }
        }
    }
}

// ============================================================================
// Shareholder Operations
// ============================================================================

#[derive(Debug, Clone)]
pub enum ShareholderOp {
    ReceiveDividend {
        amount: Decimal,
        currency: String,
    },
}

impl Operation for ShareholderOp {
    fn description(&self) -> String {
        match self {
            ShareholderOp::ReceiveDividend { amount, currency } => {
                format!("Receive dividend {} {}", amount, currency)
            }
        }
    }
}

// ============================================================================
// Manager Operations
// ============================================================================

#[derive(Debug, Clone)]
pub enum ManagerOp {
    PaySalary {
        employee_account: String,
        amount: Decimal,
        currency: String,
    },
    PayBonus {
        employee_account: String,
        amount: Decimal,
        currency: String,
        reason: String,
    },
    PayDividend {
        shareholder_account: String,
        amount: Decimal,
        currency: String,
    },
}

impl Operation for ManagerOp {
    fn description(&self) -> String {
        match self {
            ManagerOp::PaySalary { employee_account, amount, currency } => {
                format!("Pay salary {} {} to {}", amount, currency, employee_account)
            }
            ManagerOp::PayBonus { employee_account, amount, currency, reason } => {
                format!("Pay bonus {} {} to {} ({})", amount, currency, employee_account, reason)
            }
            ManagerOp::PayDividend { shareholder_account, amount, currency } => {
                format!("Pay dividend {} {} to {}", amount, currency, shareholder_account)
            }
        }
    }
}

// ============================================================================
// Auditor Operations
// ============================================================================

#[derive(Debug, Clone)]
pub enum AuditorOp {
    Scan {
        from_date: Option<String>,
        to_date: Option<String>,
        flags: Vec<String>,
    },
    Report {
        format: String,
    },
}

impl Operation for AuditorOp {
    fn description(&self) -> String {
        match self {
            AuditorOp::Scan { from_date, to_date, flags } => {
                let date_range = match (from_date, to_date) {
                    (Some(from), Some(to)) => format!("from {} to {}", from, to),
                    (Some(from), None) => format!("from {}", from),
                    _ => "all time".to_string(),
                };
                format!("Scan transactions {} with flags {:?}", date_range, flags)
            }
            AuditorOp::Report { format } => {
                format!("Generate {} report", format)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_scenario_builder() {
        let scenario = ScenarioBuilder::new()
            .customer("Alice", vec![
                CustomerOp::Deposit {
                    amount: dec!(100),
                    currency: "USDT".to_string(),
                    to_wallet: WalletType::Funding,
                },
            ])
            .employee("Bob", vec![
                EmployeeOp::ReceiveSalary {
                    amount: dec!(5000),
                    currency: "USD".to_string(),
                },
            ])
            .build();

        assert_eq!(scenario.blocks.len(), 2);
    }

    #[test]
    fn test_operation_descriptions() {
        let deposit = CustomerOp::Deposit {
            amount: dec!(100),
            currency: "USDT".to_string(),
            to_wallet: WalletType::Funding,
        };
        assert!(deposit.description().contains("100"));
        assert!(deposit.description().contains("USDT"));

        let salary = EmployeeOp::ReceiveSalary {
            amount: dec!(5000),
            currency: "USD".to_string(),
        };
        assert!(salary.description().contains("5000"));
    }

    #[test]
    fn test_scenario_iterators() {
        let scenario = ScenarioBuilder::new()
            .customer("Alice", vec![])
            .customer("Bob", vec![])
            .employee("Charlie", vec![])
            .build();

        assert_eq!(scenario.customers().count(), 2);
        assert_eq!(scenario.employees().count(), 1);
        assert_eq!(scenario.auditors().count(), 0);
    }
}
