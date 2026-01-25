---
date: 2026-01-25 20:52:47 
---

# Cấu trúc Dự án như sau:

```
./simbank
├── Cargo.toml
├── crates
│   ├── business
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── auditor.rs
│   │       ├── customer.rs
│   │       ├── employee.rs
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       ├── management.rs
│   │       ├── services.rs
│   │       └── shareholder.rs
│   ├── core
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── account.rs
│   │       ├── error.rs
│   │       ├── event.rs
│   │       ├── lib.rs
│   │       ├── money.rs
│   │       ├── person.rs
│   │       └── wallet.rs
│   ├── dsl
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── lib.rs
│   │       ├── rules.rs
│   │       └── scenario.rs
│   ├── persistence
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── error.rs
│   │       ├── events
│   │       │   ├── mod.rs
│   │       │   ├── replay.rs
│   │       │   └── store.rs
│   │       ├── lib.rs
│   │       └── sqlite
│   │           ├── mod.rs
│   │           ├── repos.rs
│   │           └── schema.rs
│   └── reports
│       ├── Cargo.toml
│       └── src
│           ├── aml_report.rs
│           ├── exporters.rs
│           └── lib.rs
├── examples
│   └── .gitkeep
└── migrations
    └── 20260125_init.sql
```

# Danh sách chi tiết các file:

## File ./simbank\crates\business\src\auditor.rs:
```rust
//! Auditor operations - AML detection rules
//!
//! AuditorService implements AML (Anti-Money Laundering) detection rules.

use crate::error::{BusinessError, BusinessResult};
use crate::services::ServiceContext;
use rust_decimal::Decimal;
use simbank_core::{AmlFlag, Event, EventType, PersonType};
use simbank_persistence::{AmlReport, EventFilter, EventReader, PersonRepo};

/// AML thresholds for detection
pub struct AmlThresholds {
    /// Amount threshold for "large_amount" flag
    pub large_amount: Decimal,
    /// Threshold for "near_threshold" (structuring detection)
    pub near_threshold_min: Decimal,
    pub near_threshold_max: Decimal,
    /// High-risk countries (ISO codes)
    pub high_risk_countries: Vec<String>,
}

impl Default for AmlThresholds {
    fn default() -> Self {
        Self {
            large_amount: Decimal::new(10000, 0), // $10,000
            near_threshold_min: Decimal::new(9000, 0),
            near_threshold_max: Decimal::new(9999, 0),
            high_risk_countries: vec![
                "KP".to_string(), // North Korea
                "IR".to_string(), // Iran
                "SY".to_string(), // Syria
                "CU".to_string(), // Cuba
            ],
        }
    }
}

/// Auditor Service - AML detection and reporting
pub struct AuditorService<'a> {
    ctx: &'a ServiceContext,
    thresholds: AmlThresholds,
}

impl<'a> AuditorService<'a> {
    pub fn new(ctx: &'a ServiceContext) -> Self {
        Self {
            ctx,
            thresholds: AmlThresholds::default(),
        }
    }

    pub fn with_thresholds(mut self, thresholds: AmlThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Verify auditor has permission
    pub async fn verify_auditor(&self, auditor_id: &str) -> BusinessResult<()> {
        let person = PersonRepo::get_by_id(self.ctx.pool(), auditor_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(auditor_id.to_string()))?;

        if person.person_type != "auditor" {
            return Err(BusinessError::not_permitted(&person.person_type, "audit").into());
        }

        Ok(())
    }

    /// Check if an event should be flagged for AML
    pub fn check_aml_flags(&self, event: &Event) -> Vec<AmlFlag> {
        let mut flags = Vec::new();

        // Check amount thresholds
        if let Some(amount) = event.amount {
            // Large amount
            if amount >= self.thresholds.large_amount {
                flags.push(AmlFlag::LargeAmount);
            }

            // Near threshold (potential structuring)
            if amount >= self.thresholds.near_threshold_min
                && amount <= self.thresholds.near_threshold_max
            {
                flags.push(AmlFlag::NearThreshold);
            }
        }

        // Check location
        if let Some(ref location) = event.metadata.location {
            if self.thresholds.high_risk_countries.contains(location) {
                flags.push(AmlFlag::HighRiskCountry);
            }
        }

        flags
    }

    /// Scan events for AML flags
    pub async fn scan_transactions(
        &self,
        auditor_id: &str,
        from_date: Option<&str>,
        to_date: Option<&str>,
        flag_filter: Option<Vec<AmlFlag>>,
    ) -> BusinessResult<AmlReport> {
        // Verify auditor permission
        self.verify_auditor(auditor_id).await?;

        // Read events
        let reader = EventReader::new(self.ctx.events().base_path());
        let events = match (from_date, to_date) {
            (Some(from), Some(to)) => reader.read_range(from, to)?,
            _ => reader.read_all()?,
        };

        // Apply filter if specified
        let events = if let Some(flags) = flag_filter {
            EventFilter::new().aml_flags(flags).apply(events)
        } else {
            events
        };

        // Generate report
        let report = AmlReport::generate(&events);

        // Log audit access event
        let event_id = self.ctx.next_event_id();
        let audit_event = Event::new(
            event_id,
            EventType::AuditAccess,
            auditor_id.to_string(),
            PersonType::Auditor,
            "SYSTEM".to_string(),
        )
        .with_description(&format!(
            "AML scan: {} events analyzed, {} flagged",
            report.total_events, report.flagged_events
        ));

        self.ctx.events().append(&audit_event)?;

        Ok(report)
    }

    /// Get flagged events only
    pub async fn get_flagged_events(
        &self,
        auditor_id: &str,
    ) -> BusinessResult<Vec<Event>> {
        self.verify_auditor(auditor_id).await?;

        let reader = EventReader::new(self.ctx.events().base_path());
        let events = reader.read_all()?;

        let flagged = EventFilter::new().flagged_only().apply(events);

        Ok(flagged)
    }

    /// Get high-value transactions
    pub async fn get_high_value_transactions(
        &self,
        auditor_id: &str,
        min_amount: Decimal,
    ) -> BusinessResult<Vec<Event>> {
        self.verify_auditor(auditor_id).await?;

        let reader = EventReader::new(self.ctx.events().base_path());
        let events = reader.read_all()?;

        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|e| e.amount.map_or(false, |a| a >= min_amount))
            .collect();

        Ok(filtered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_aml_thresholds_default() {
        let thresholds = AmlThresholds::default();
        assert_eq!(thresholds.large_amount, dec!(10000));
        assert_eq!(thresholds.near_threshold_min, dec!(9000));
        assert_eq!(thresholds.near_threshold_max, dec!(9999));
        assert!(thresholds.high_risk_countries.contains(&"KP".to_string()));
        assert!(thresholds.high_risk_countries.contains(&"IR".to_string()));
    }

    #[test]
    fn test_aml_check_large_amount() {
        // Test check_aml_flags standalone logic
        let thresholds = AmlThresholds::default();

        // Create a large amount event
        let event = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");

        // Manually check flags
        let mut flags = Vec::new();
        if let Some(amount) = event.amount {
            if amount >= thresholds.large_amount {
                flags.push(AmlFlag::LargeAmount);
            }
        }

        assert!(flags.contains(&AmlFlag::LargeAmount));
    }

    #[test]
    fn test_aml_check_near_threshold() {
        let thresholds = AmlThresholds::default();
        let event = Event::deposit("EVT_002", "CUST_001", "ACC_001", dec!(9500), "USD");

        let mut flags = Vec::new();
        if let Some(amount) = event.amount {
            if amount >= thresholds.near_threshold_min && amount <= thresholds.near_threshold_max {
                flags.push(AmlFlag::NearThreshold);
            }
        }

        assert!(flags.contains(&AmlFlag::NearThreshold));
    }
}

```

## File ./simbank\crates\business\src\customer.rs:
```rust
//! Customer operations - deposit, withdraw, transfer
//!
//! CustomerService implements the main transaction operations for customers.

use crate::error::{BusinessError, BusinessResult};
use crate::services::{ServiceContext, TransactionResult};
use anyhow::Context;
use chrono::Utc;
use rust_decimal::Decimal;
use simbank_core::{Event, WalletType};
use simbank_persistence::{AccountRepo, BalanceRepo, TransactionRepo, TransactionRow, WalletRepo};
use std::sync::atomic::{AtomicU64, Ordering};

/// Transaction ID counter (in production, use DB sequence)
static TXN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_txn_id() -> String {
    let id = TXN_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("TXN_{:06}", id)
}

/// Customer Service - handles deposit, withdraw, transfer operations
pub struct CustomerService<'a> {
    ctx: &'a ServiceContext,
}

impl<'a> CustomerService<'a> {
    pub fn new(ctx: &'a ServiceContext) -> Self {
        Self { ctx }
    }

    /// Deposit funds to customer's Funding wallet
    pub async fn deposit(
        &self,
        actor_id: &str,
        account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> BusinessResult<TransactionResult> {
        // Validate amount
        if amount <= Decimal::ZERO {
            return Err(BusinessError::InvalidAmount(format!(
                "Deposit amount must be positive: {}",
                amount
            ))
            .into());
        }

        // Get account and verify status
        let account = AccountRepo::get_by_id(self.ctx.pool(), account_id)
            .await
            .map_err(|_| BusinessError::AccountNotFound(account_id.to_string()))?;

        if account.status != "active" {
            return Err(BusinessError::account_not_active(account_id, &account.status).into());
        }

        // Get Funding wallet
        let wallet = WalletRepo::get_by_account_and_type(
            self.ctx.pool(),
            account_id,
            WalletType::Funding,
        )
        .await
        .context("Failed to get Funding wallet")?
        .ok_or_else(|| BusinessError::WalletNotFound(format!("{}-Funding", account_id)))?;

        // Generate IDs
        let txn_id = next_txn_id();
        let event_id = self.ctx.next_event_id();

        // Create event
        let event = Event::deposit(&event_id, actor_id, account_id, amount, currency);

        // Dual-write: update balance and append event
        let pool = self.ctx.pool();

        // Update balance
        BalanceRepo::credit(pool, &wallet.id, currency, amount)
            .await
            .context("Failed to credit balance")?;

        // Record transaction
        let tx_row = TransactionRow {
            id: txn_id.clone(),
            account_id: account_id.to_string(),
            wallet_id: wallet.id.clone(),
            tx_type: "deposit".to_string(),
            amount: amount.to_string(),
            currency_code: currency.to_string(),
            description: Some(format!("Deposit {} {}", amount, currency)),
            created_at: Utc::now(),
        };
        TransactionRepo::insert(pool, &tx_row)
            .await
            .context("Failed to record transaction")?;

        // Append event
        self.ctx.events().append(&event)?;

        Ok(TransactionResult::new(&txn_id, &event_id, amount, currency)
            .with_to_wallet(WalletType::Funding))
    }

    /// Withdraw funds from customer's Funding wallet
    pub async fn withdraw(
        &self,
        actor_id: &str,
        account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> BusinessResult<TransactionResult> {
        // Validate amount
        if amount <= Decimal::ZERO {
            return Err(BusinessError::InvalidAmount(format!(
                "Withdrawal amount must be positive: {}",
                amount
            ))
            .into());
        }

        // Get account and verify status
        let account = AccountRepo::get_by_id(self.ctx.pool(), account_id)
            .await
            .map_err(|_| BusinessError::AccountNotFound(account_id.to_string()))?;

        if account.status != "active" {
            return Err(BusinessError::account_not_active(account_id, &account.status).into());
        }

        // Get Funding wallet
        let wallet = WalletRepo::get_by_account_and_type(
            self.ctx.pool(),
            account_id,
            WalletType::Funding,
        )
        .await
        .context("Failed to get Funding wallet")?
        .ok_or_else(|| BusinessError::WalletNotFound(format!("{}-Funding", account_id)))?;

        // Check balance
        let balance = BalanceRepo::get(self.ctx.pool(), &wallet.id, currency)
            .await
            .ok()
            .flatten();

        let available = balance
            .as_ref()
            .and_then(|b| b.available.parse::<Decimal>().ok())
            .unwrap_or(Decimal::ZERO);

        if available < amount {
            return Err(BusinessError::insufficient_balance(amount, available).into());
        }

        // Generate IDs
        let txn_id = next_txn_id();
        let event_id = self.ctx.next_event_id();

        // Create event
        let event = Event::withdrawal(&event_id, actor_id, account_id, amount, currency);

        // Dual-write
        let pool = self.ctx.pool();

        // Debit balance
        BalanceRepo::debit(pool, &wallet.id, currency, amount)
            .await
            .context("Failed to debit balance")?;

        // Record transaction
        let tx_row = TransactionRow {
            id: txn_id.clone(),
            account_id: account_id.to_string(),
            wallet_id: wallet.id.clone(),
            tx_type: "withdrawal".to_string(),
            amount: amount.to_string(),
            currency_code: currency.to_string(),
            description: Some(format!("Withdraw {} {}", amount, currency)),
            created_at: Utc::now(),
        };
        TransactionRepo::insert(pool, &tx_row)
            .await
            .context("Failed to record transaction")?;

        // Append event
        self.ctx.events().append(&event)?;

        Ok(TransactionResult::new(&txn_id, &event_id, amount, currency)
            .with_from_wallet(WalletType::Funding))
    }

    /// Internal transfer between wallets (free, instant)
    pub async fn transfer(
        &self,
        actor_id: &str,
        account_id: &str,
        from_wallet_type: WalletType,
        to_wallet_type: WalletType,
        amount: Decimal,
        currency: &str,
    ) -> BusinessResult<TransactionResult> {
        // Validate
        if amount <= Decimal::ZERO {
            return Err(BusinessError::InvalidAmount(format!(
                "Transfer amount must be positive: {}",
                amount
            ))
            .into());
        }

        if from_wallet_type == to_wallet_type {
            return Err(BusinessError::InvalidAmount(
                "Cannot transfer to the same wallet".to_string(),
            )
            .into());
        }

        // Get account
        let account = AccountRepo::get_by_id(self.ctx.pool(), account_id)
            .await
            .map_err(|_| BusinessError::AccountNotFound(account_id.to_string()))?;

        if account.status != "active" {
            return Err(BusinessError::account_not_active(account_id, &account.status).into());
        }

        // Get wallets
        let from_wallet = WalletRepo::get_by_account_and_type(
            self.ctx.pool(),
            account_id,
            from_wallet_type.clone(),
        )
        .await
        .context("Failed to get source wallet")?
        .ok_or_else(|| {
            BusinessError::WalletNotFound(format!("{}-{:?}", account_id, from_wallet_type))
        })?;

        let to_wallet = WalletRepo::get_by_account_and_type(
            self.ctx.pool(),
            account_id,
            to_wallet_type.clone(),
        )
        .await
        .context("Failed to get destination wallet")?
        .ok_or_else(|| {
            BusinessError::WalletNotFound(format!("{}-{:?}", account_id, to_wallet_type))
        })?;

        // Check balance
        let balance = BalanceRepo::get(self.ctx.pool(), &from_wallet.id, currency)
            .await
            .ok()
            .flatten();

        let available = balance
            .as_ref()
            .and_then(|b| b.available.parse::<Decimal>().ok())
            .unwrap_or(Decimal::ZERO);

        if available < amount {
            return Err(BusinessError::insufficient_balance(amount, available).into());
        }

        // Generate IDs
        let txn_id = next_txn_id();
        let event_id = self.ctx.next_event_id();

        // Create event
        let event = Event::internal_transfer(
            &event_id,
            actor_id,
            account_id,
            from_wallet_type.clone(),
            to_wallet_type.clone(),
            amount,
            currency,
        );

        // Dual-write
        let pool = self.ctx.pool();

        // Debit source
        BalanceRepo::debit(pool, &from_wallet.id, currency, amount)
            .await
            .context("Failed to debit source wallet")?;

        // Credit destination
        BalanceRepo::credit(pool, &to_wallet.id, currency, amount)
            .await
            .context("Failed to credit destination wallet")?;

        // Record transaction
        let tx_row = TransactionRow {
            id: txn_id.clone(),
            account_id: account_id.to_string(),
            wallet_id: from_wallet.id.clone(),
            tx_type: "internal_transfer".to_string(),
            amount: amount.to_string(),
            currency_code: currency.to_string(),
            description: Some(format!(
                "Transfer {} {} from {:?} to {:?}",
                amount, currency, from_wallet_type, to_wallet_type
            )),
            created_at: Utc::now(),
        };
        TransactionRepo::insert(pool, &tx_row)
            .await
            .context("Failed to record transaction")?;

        // Append event
        self.ctx.events().append(&event)?;

        Ok(TransactionResult::new(&txn_id, &event_id, amount, currency)
            .with_wallets(from_wallet_type, to_wallet_type))
    }
}

```

## File ./simbank\crates\business\src\employee.rs:
```rust
//! Employee operations - salary, insurance
//!
//! EmployeeService handles payroll and benefits for bank employees.

use crate::error::{BusinessError, BusinessResult};
use crate::services::{ServiceContext, TransactionResult};
use anyhow::Context;
use chrono::Utc;
use rust_decimal::Decimal;
use simbank_core::{Event, EventType, PersonType, WalletType};
use simbank_persistence::{
    AccountRepo, BalanceRepo, PersonRepo, TransactionRepo, TransactionRow, WalletRepo,
};
use std::sync::atomic::{AtomicU64, Ordering};

/// Transaction ID counter
static TXN_COUNTER: AtomicU64 = AtomicU64::new(1000);

fn next_txn_id() -> String {
    let id = TXN_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("TXN_{:06}", id)
}

/// Employee Service - handles salary payments and insurance
pub struct EmployeeService<'a> {
    ctx: &'a ServiceContext,
}

impl<'a> EmployeeService<'a> {
    pub fn new(ctx: &'a ServiceContext) -> Self {
        Self { ctx }
    }

    /// Pay salary to employee's Funding wallet
    pub async fn pay_salary(
        &self,
        manager_id: &str,
        employee_account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> BusinessResult<TransactionResult> {
        // Validate amount
        if amount <= Decimal::ZERO {
            return Err(BusinessError::InvalidAmount(format!(
                "Salary must be positive: {}",
                amount
            ))
            .into());
        }

        // Verify manager exists and is a Manager
        let manager = PersonRepo::get_by_id(self.ctx.pool(), manager_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(manager_id.to_string()))?;

        if manager.person_type != "manager" {
            return Err(BusinessError::not_permitted(&manager.person_type, "pay_salary").into());
        }

        // Get employee account
        let account = AccountRepo::get_by_id(self.ctx.pool(), employee_account_id)
            .await
            .map_err(|_| BusinessError::AccountNotFound(employee_account_id.to_string()))?;

        if account.status != "active" {
            return Err(
                BusinessError::account_not_active(employee_account_id, &account.status).into(),
            );
        }

        // Verify employee is indeed an employee
        let employee = PersonRepo::get_by_id(self.ctx.pool(), &account.person_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(account.person_id.clone()))?;

        if employee.person_type != "employee" {
            return Err(BusinessError::not_permitted(
                &employee.person_type,
                "receive_salary",
            )
            .into());
        }

        // Get Funding wallet
        let wallet = WalletRepo::get_by_account_and_type(
            self.ctx.pool(),
            employee_account_id,
            WalletType::Funding,
        )
        .await
        .context("Failed to get employee Funding wallet")?
        .ok_or_else(|| {
            BusinessError::WalletNotFound(format!("{}-Funding", employee_account_id))
        })?;

        // Generate IDs
        let txn_id = next_txn_id();
        let event_id = self.ctx.next_event_id();

        // Create event
        let event = Event::new(
            event_id.clone(),
            EventType::SalaryPayment,
            manager_id.to_string(),
            PersonType::Manager,
            employee_account_id.to_string(),
        )
        .with_to_wallet(WalletType::Funding)
        .with_amount(amount, currency)
        .with_description(&format!("Salary payment to {}", employee.name));

        // Dual-write
        let pool = self.ctx.pool();

        // Credit employee balance
        BalanceRepo::credit(pool, &wallet.id, currency, amount)
            .await
            .context("Failed to credit salary")?;

        // Record transaction
        let tx_row = TransactionRow {
            id: txn_id.clone(),
            account_id: employee_account_id.to_string(),
            wallet_id: wallet.id.clone(),
            tx_type: "salary_payment".to_string(),
            amount: amount.to_string(),
            currency_code: currency.to_string(),
            description: Some(format!("Salary: {} {}", amount, currency)),
            created_at: Utc::now(),
        };
        TransactionRepo::insert(pool, &tx_row)
            .await
            .context("Failed to record salary transaction")?;

        // Append event
        self.ctx.events().append(&event)?;

        Ok(TransactionResult::new(&txn_id, &event_id, amount, currency)
            .with_to_wallet(WalletType::Funding))
    }

    /// Purchase insurance for employee (deducts from their Funding wallet)
    pub async fn purchase_insurance(
        &self,
        employee_id: &str,
        account_id: &str,
        plan_name: &str,
        cost: Decimal,
        currency: &str,
    ) -> BusinessResult<TransactionResult> {
        // Validate cost
        if cost <= Decimal::ZERO {
            return Err(BusinessError::InvalidAmount(format!(
                "Insurance cost must be positive: {}",
                cost
            ))
            .into());
        }

        // Get account
        let account = AccountRepo::get_by_id(self.ctx.pool(), account_id)
            .await
            .map_err(|_| BusinessError::AccountNotFound(account_id.to_string()))?;

        if account.status != "active" {
            return Err(BusinessError::account_not_active(account_id, &account.status).into());
        }

        // Verify is employee
        let person = PersonRepo::get_by_id(self.ctx.pool(), &account.person_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(account.person_id.clone()))?;

        if person.person_type != "employee" {
            return Err(
                BusinessError::not_permitted(&person.person_type, "purchase_insurance").into(),
            );
        }

        // Get Funding wallet
        let wallet = WalletRepo::get_by_account_and_type(
            self.ctx.pool(),
            account_id,
            WalletType::Funding,
        )
        .await
        .context("Failed to get Funding wallet")?
        .ok_or_else(|| BusinessError::WalletNotFound(format!("{}-Funding", account_id)))?;

        // Check balance
        let balance = BalanceRepo::get(self.ctx.pool(), &wallet.id, currency)
            .await
            .ok()
            .flatten();

        let available = balance
            .as_ref()
            .and_then(|b| b.available.parse::<Decimal>().ok())
            .unwrap_or(Decimal::ZERO);

        if available < cost {
            return Err(BusinessError::insufficient_balance(cost, available).into());
        }

        // Generate IDs
        let txn_id = next_txn_id();
        let event_id = self.ctx.next_event_id();

        // Create event
        let event = Event::new(
            event_id.clone(),
            EventType::InsurancePurchase,
            employee_id.to_string(),
            PersonType::Employee,
            account_id.to_string(),
        )
        .with_from_wallet(WalletType::Funding)
        .with_amount(cost, currency)
        .with_description(&format!("Insurance plan: {}", plan_name));

        // Dual-write
        let pool = self.ctx.pool();

        // Debit employee balance
        BalanceRepo::debit(pool, &wallet.id, currency, cost)
            .await
            .context("Failed to debit for insurance")?;

        // Record transaction
        let tx_row = TransactionRow {
            id: txn_id.clone(),
            account_id: account_id.to_string(),
            wallet_id: wallet.id.clone(),
            tx_type: "insurance_purchase".to_string(),
            amount: cost.to_string(),
            currency_code: currency.to_string(),
            description: Some(format!("Insurance: {} - {} {}", plan_name, cost, currency)),
            created_at: Utc::now(),
        };
        TransactionRepo::insert(pool, &tx_row)
            .await
            .context("Failed to record insurance transaction")?;

        // Append event
        self.ctx.events().append(&event)?;

        Ok(TransactionResult::new(&txn_id, &event_id, cost, currency)
            .with_from_wallet(WalletType::Funding))
    }
}

```

## File ./simbank\crates\business\src\error.rs:
```rust
//! Business layer errors
//!
//! Uses anyhow for error aggregation with custom error types.

use rust_decimal::Decimal;
use thiserror::Error;

/// Business operation errors
#[derive(Debug, Error)]
pub enum BusinessError {
    // === Validation errors ===
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
    },

    #[error("Currency mismatch: expected {expected}, got {actual}")]
    CurrencyMismatch { expected: String, actual: String },

    // === Permission errors ===
    #[error("Operation not permitted for {person_type}: {operation}")]
    OperationNotPermitted {
        person_type: String,
        operation: String,
    },

    #[error("Account not active: {account_id} (status: {status})")]
    AccountNotActive { account_id: String, status: String },

    #[error("Wallet not active: {wallet_id} (status: {status})")]
    WalletNotActive { wallet_id: String, status: String },

    // === Not found errors ===
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Wallet not found: {0}")]
    WalletNotFound(String),

    #[error("Person not found: {0}")]
    PersonNotFound(String),

    #[error("Currency not found: {0}")]
    CurrencyNotFound(String),

    // === AML errors ===
    #[error("Transaction requires approval: {reason}")]
    RequiresApproval { reason: String },

    #[error("Transaction blocked by AML: {reason}")]
    AmlBlocked { reason: String },

    // === Wrapped errors ===
    #[error("Persistence error: {0}")]
    Persistence(#[from] simbank_persistence::PersistenceError),

    #[error("Core error: {0}")]
    Core(#[from] simbank_core::CoreError),
}

/// Result type alias for business operations
pub type BusinessResult<T> = anyhow::Result<T>;

impl BusinessError {
    /// Create insufficient balance error
    pub fn insufficient_balance(required: Decimal, available: Decimal) -> Self {
        Self::InsufficientBalance {
            required,
            available,
        }
    }

    /// Create operation not permitted error
    pub fn not_permitted(person_type: &str, operation: &str) -> Self {
        Self::OperationNotPermitted {
            person_type: person_type.to_string(),
            operation: operation.to_string(),
        }
    }

    /// Create account not active error
    pub fn account_not_active(account_id: &str, status: &str) -> Self {
        Self::AccountNotActive {
            account_id: account_id.to_string(),
            status: status.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_insufficient_balance_error() {
        let err = BusinessError::insufficient_balance(dec!(100), dec!(50));
        assert!(err.to_string().contains("required 100"));
        assert!(err.to_string().contains("available 50"));
    }

    #[test]
    fn test_not_permitted_error() {
        let err = BusinessError::not_permitted("Auditor", "deposit");
        assert!(err.to_string().contains("Auditor"));
        assert!(err.to_string().contains("deposit"));
    }
}

```

## File ./simbank\crates\business\src\lib.rs:
```rust
//! # Simbank Business
//!
//! Business logic layer - Customer, Employee, Auditor operations.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Business Layer                           │
//! │  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐  │
//! │  │ Customer  │ │ Employee  │ │Shareholder│ │  Auditor  │  │
//! │  │  Service  │ │  Service  │ │  Service  │ │  Service  │  │
//! │  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘  │
//! │        │             │             │             │         │
//! │        └─────────────┴─────────────┴─────────────┘         │
//! │                          │                                  │
//! │                  ServiceContext                             │
//! │                    (Pool + Events)                          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use simbank_business::{ServiceContext, CustomerService};
//!
//! let ctx = ServiceContext::new(&db);
//! let customer_svc = CustomerService::new(&ctx);
//!
//! // Deposit 100 USDT to customer's Funding wallet
//! let result = customer_svc.deposit("CUST_001", "ACC_001", dec!(100), "USDT").await?;
//! ```

pub mod auditor;
pub mod customer;
pub mod employee;
pub mod error;
pub mod management;
pub mod services;
pub mod shareholder;

// Re-export commonly used types
pub use auditor::{AmlThresholds, AuditorService};
pub use customer::CustomerService;
pub use employee::EmployeeService;
pub use error::{BusinessError, BusinessResult};
pub use management::ManagementService;
pub use services::{AccountCreationResult, ServiceContext, TransactionResult};
pub use shareholder::ShareholderService;

```

## File ./simbank\crates\business\src\management.rs:
```rust
//! Management operations - bonus approval
//!
//! ManagementService handles bonus payments and approvals.

use crate::error::{BusinessError, BusinessResult};
use crate::services::{ServiceContext, TransactionResult};
use anyhow::Context;
use chrono::Utc;
use rust_decimal::Decimal;
use simbank_core::{Event, EventType, PersonType, WalletType};
use simbank_persistence::{
    AccountRepo, BalanceRepo, PersonRepo, TransactionRepo, TransactionRow, WalletRepo,
};
use std::sync::atomic::{AtomicU64, Ordering};

static TXN_COUNTER: AtomicU64 = AtomicU64::new(3000);

fn next_txn_id() -> String {
    let id = TXN_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("TXN_{:06}", id)
}

/// Management Service - handles bonus payments
pub struct ManagementService<'a> {
    ctx: &'a ServiceContext,
}

impl<'a> ManagementService<'a> {
    pub fn new(ctx: &'a ServiceContext) -> Self {
        Self { ctx }
    }

    /// Pay bonus to an employee
    pub async fn pay_bonus(
        &self,
        manager_id: &str,
        employee_account_id: &str,
        amount: Decimal,
        currency: &str,
        reason: &str,
    ) -> BusinessResult<TransactionResult> {
        // Validate
        if amount <= Decimal::ZERO {
            return Err(
                BusinessError::InvalidAmount(format!("Bonus must be positive: {}", amount)).into(),
            );
        }

        // Verify manager
        let manager = PersonRepo::get_by_id(self.ctx.pool(), manager_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(manager_id.to_string()))?;

        if manager.person_type != "manager" {
            return Err(BusinessError::not_permitted(&manager.person_type, "pay_bonus").into());
        }

        // Get employee account
        let account = AccountRepo::get_by_id(self.ctx.pool(), employee_account_id)
            .await
            .map_err(|_| BusinessError::AccountNotFound(employee_account_id.to_string()))?;

        // Verify is employee
        let employee = PersonRepo::get_by_id(self.ctx.pool(), &account.person_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(account.person_id.clone()))?;

        if employee.person_type != "employee" {
            return Err(
                BusinessError::not_permitted(&employee.person_type, "receive_bonus").into(),
            );
        }

        // Get Funding wallet
        let wallet = WalletRepo::get_by_account_and_type(
            self.ctx.pool(),
            employee_account_id,
            WalletType::Funding,
        )
        .await
        .context("Failed to get employee Funding wallet")?
        .ok_or_else(|| {
            BusinessError::WalletNotFound(format!("{}-Funding", employee_account_id))
        })?;

        // Generate IDs
        let txn_id = next_txn_id();
        let event_id = self.ctx.next_event_id();

        // Create event
        let event = Event::new(
            event_id.clone(),
            EventType::BonusPayment,
            manager_id.to_string(),
            PersonType::Manager,
            employee_account_id.to_string(),
        )
        .with_to_wallet(WalletType::Funding)
        .with_amount(amount, currency)
        .with_description(&format!("Bonus: {} - {}", reason, employee.name));

        // Dual-write
        let pool = self.ctx.pool();

        BalanceRepo::credit(pool, &wallet.id, currency, amount)
            .await
            .context("Failed to credit bonus")?;

        let tx_row = TransactionRow {
            id: txn_id.clone(),
            account_id: employee_account_id.to_string(),
            wallet_id: wallet.id.clone(),
            tx_type: "bonus_payment".to_string(),
            amount: amount.to_string(),
            currency_code: currency.to_string(),
            description: Some(format!("Bonus: {} - {} {}", reason, amount, currency)),
            created_at: Utc::now(),
        };
        TransactionRepo::insert(pool, &tx_row)
            .await
            .context("Failed to record bonus transaction")?;

        self.ctx.events().append(&event)?;

        Ok(TransactionResult::new(&txn_id, &event_id, amount, currency)
            .with_to_wallet(WalletType::Funding))
    }
}

```

## File ./simbank\crates\business\src\services.rs:
```rust
//! Service traits and implementations
//!
//! Defines the core service interfaces for business operations.

use crate::error::BusinessResult;
use rust_decimal::Decimal;
use simbank_core::{Account, Event, Person, WalletType};
use simbank_persistence::{Database, EventStore};
use sqlx::SqlitePool;
use std::sync::Arc;

/// Context for business operations - contains database access
pub struct ServiceContext {
    pool: SqlitePool,
    events: Arc<EventStore>,
}

impl ServiceContext {
    /// Create new service context from database
    pub fn new(db: &Database) -> Self {
        Self {
            pool: db.pool().clone(),
            events: Arc::new(EventStore::new(db.events().base_path()).expect("EventStore")),
        }
    }

    /// Create from pool and event store directly
    pub fn from_parts(pool: SqlitePool, events: Arc<EventStore>) -> Self {
        Self { pool, events }
    }

    /// Get database pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get event store
    pub fn events(&self) -> &EventStore {
        &self.events
    }

    /// Generate next event ID
    pub fn next_event_id(&self) -> String {
        self.events.next_event_id()
    }

    /// Dual-write helper: write to DB and append event
    pub async fn dual_write<F, Fut>(&self, event: &Event, db_op: F) -> BusinessResult<()>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = BusinessResult<()>>,
    {
        // Execute DB operation first
        db_op().await?;

        // Then append event (if DB succeeded)
        self.events.append(event)?;

        Ok(())
    }
}

/// Transaction result from business operations
#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub transaction_id: String,
    pub event_id: String,
    pub amount: Decimal,
    pub currency: String,
    pub from_wallet: Option<WalletType>,
    pub to_wallet: Option<WalletType>,
}

impl TransactionResult {
    pub fn new(
        transaction_id: &str,
        event_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> Self {
        Self {
            transaction_id: transaction_id.to_string(),
            event_id: event_id.to_string(),
            amount,
            currency: currency.to_string(),
            from_wallet: None,
            to_wallet: None,
        }
    }

    pub fn with_to_wallet(mut self, wallet: WalletType) -> Self {
        self.to_wallet = Some(wallet);
        self
    }

    pub fn with_from_wallet(mut self, wallet: WalletType) -> Self {
        self.from_wallet = Some(wallet);
        self
    }

    pub fn with_wallets(mut self, from: WalletType, to: WalletType) -> Self {
        self.from_wallet = Some(from);
        self.to_wallet = Some(to);
        self
    }
}

/// Account creation result
#[derive(Debug, Clone)]
pub struct AccountCreationResult {
    pub person: Person,
    pub account: Account,
    pub event_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_transaction_result() {
        let result = TransactionResult::new("TXN_001", "EVT_001", dec!(100), "USDT")
            .with_to_wallet(WalletType::Funding);

        assert_eq!(result.transaction_id, "TXN_001");
        assert_eq!(result.to_wallet, Some(WalletType::Funding));
    }
}

```

## File ./simbank\crates\business\src\shareholder.rs:
```rust
//! Shareholder operations - dividend
//!
//! ShareholderService handles dividend payments to shareholders.

use crate::error::{BusinessError, BusinessResult};
use crate::services::{ServiceContext, TransactionResult};
use anyhow::Context;
use chrono::Utc;
use rust_decimal::Decimal;
use simbank_core::{Event, EventType, PersonType, WalletType};
use simbank_persistence::{
    AccountRepo, BalanceRepo, PersonRepo, TransactionRepo, TransactionRow, WalletRepo,
};
use std::sync::atomic::{AtomicU64, Ordering};

static TXN_COUNTER: AtomicU64 = AtomicU64::new(2000);

fn next_txn_id() -> String {
    let id = TXN_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("TXN_{:06}", id)
}

/// Shareholder Service - handles dividend payments
pub struct ShareholderService<'a> {
    ctx: &'a ServiceContext,
}

impl<'a> ShareholderService<'a> {
    pub fn new(ctx: &'a ServiceContext) -> Self {
        Self { ctx }
    }

    /// Pay dividend to shareholder's Funding wallet
    pub async fn pay_dividend(
        &self,
        manager_id: &str,
        shareholder_account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> BusinessResult<TransactionResult> {
        // Validate amount
        if amount <= Decimal::ZERO {
            return Err(BusinessError::InvalidAmount(format!(
                "Dividend must be positive: {}",
                amount
            ))
            .into());
        }

        // Verify manager
        let manager = PersonRepo::get_by_id(self.ctx.pool(), manager_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(manager_id.to_string()))?;

        if manager.person_type != "manager" {
            return Err(BusinessError::not_permitted(&manager.person_type, "pay_dividend").into());
        }

        // Get shareholder account
        let account = AccountRepo::get_by_id(self.ctx.pool(), shareholder_account_id)
            .await
            .map_err(|_| BusinessError::AccountNotFound(shareholder_account_id.to_string()))?;

        // Verify is shareholder
        let shareholder = PersonRepo::get_by_id(self.ctx.pool(), &account.person_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(account.person_id.clone()))?;

        if shareholder.person_type != "shareholder" {
            return Err(
                BusinessError::not_permitted(&shareholder.person_type, "receive_dividend").into(),
            );
        }

        // Get Funding wallet
        let wallet = WalletRepo::get_by_account_and_type(
            self.ctx.pool(),
            shareholder_account_id,
            WalletType::Funding,
        )
        .await
        .context("Failed to get shareholder Funding wallet")?
        .ok_or_else(|| {
            BusinessError::WalletNotFound(format!("{}-Funding", shareholder_account_id))
        })?;

        // Generate IDs
        let txn_id = next_txn_id();
        let event_id = self.ctx.next_event_id();

        // Create event
        let event = Event::new(
            event_id.clone(),
            EventType::DividendPayment,
            manager_id.to_string(),
            PersonType::Manager,
            shareholder_account_id.to_string(),
        )
        .with_to_wallet(WalletType::Funding)
        .with_amount(amount, currency)
        .with_description(&format!("Dividend payment to {}", shareholder.name));

        // Dual-write
        let pool = self.ctx.pool();

        BalanceRepo::credit(pool, &wallet.id, currency, amount)
            .await
            .context("Failed to credit dividend")?;

        let tx_row = TransactionRow {
            id: txn_id.clone(),
            account_id: shareholder_account_id.to_string(),
            wallet_id: wallet.id.clone(),
            tx_type: "dividend_payment".to_string(),
            amount: amount.to_string(),
            currency_code: currency.to_string(),
            description: Some(format!("Dividend: {} {}", amount, currency)),
            created_at: Utc::now(),
        };
        TransactionRepo::insert(pool, &tx_row)
            .await
            .context("Failed to record dividend transaction")?;

        self.ctx.events().append(&event)?;

        Ok(TransactionResult::new(&txn_id, &event_id, amount, currency)
            .with_to_wallet(WalletType::Funding))
    }
}

```

## File ./simbank\crates\core\src\account.rs:
```rust
//! # Account Module
//!
//! Định nghĩa Account - đại diện cho tài khoản của người dùng.
//! Mỗi Account có quan hệ 1:1 với Person và chứa nhiều Wallets.

use crate::person::Person;
use crate::wallet::{Wallet, WalletType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Trạng thái của Account
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    /// Tài khoản hoạt động bình thường
    Active,
    /// Tài khoản bị đóng băng (nghi ngờ gian lận, vi phạm)
    Frozen,
    /// Tài khoản đã đóng
    Closed,
}

impl AccountStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountStatus::Active => "active",
            AccountStatus::Frozen => "frozen",
            AccountStatus::Closed => "closed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(AccountStatus::Active),
            "frozen" => Some(AccountStatus::Frozen),
            "closed" => Some(AccountStatus::Closed),
            _ => None,
        }
    }
}

impl fmt::Display for AccountStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tài khoản của người dùng.
///
/// Mỗi Account:
/// - Thuộc về một Person (1:1)
/// - Chứa nhiều Wallets (Spot, Funding, ...)
/// - Có trạng thái (Active, Frozen, Closed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// ID của account (ACC_001, ACC_002, ...)
    pub id: String,
    /// ID của person sở hữu account này
    pub person_id: String,
    /// Trạng thái account
    pub status: AccountStatus,
    /// Map từ WalletType -> Wallet
    pub wallets: HashMap<WalletType, Wallet>,
    /// Thời gian tạo
    pub created_at: DateTime<Utc>,
}

impl Account {
    /// Tạo Account mới với các wallets mặc định (Phase 1: Spot + Funding)
    pub fn new(id: String, person_id: String) -> Self {
        let mut account = Self {
            id: id.clone(),
            person_id,
            status: AccountStatus::Active,
            wallets: HashMap::new(),
            created_at: Utc::now(),
        };

        // Eager creation: Tạo sẵn các wallets cho Phase 1
        account.create_default_wallets();
        account
    }

    /// Tạo Account từ Person
    pub fn from_person(account_id: &str, person: &Person) -> Self {
        Self::new(account_id.to_string(), person.id.clone())
    }

    /// Tạo các wallets mặc định (Phase 1: Spot + Funding)
    fn create_default_wallets(&mut self) {
        let mut wallet_counter = 1;
        for wallet_type in WalletType::phase1_types() {
            let wallet_id = format!("WAL_{:03}", wallet_counter);
            let wallet = Wallet::new(wallet_id, self.id.clone(), wallet_type);
            self.wallets.insert(wallet_type, wallet);
            wallet_counter += 1;
        }
    }

    /// Lấy wallet theo loại
    pub fn get_wallet(&self, wallet_type: WalletType) -> Option<&Wallet> {
        self.wallets.get(&wallet_type)
    }

    /// Lấy mutable wallet theo loại
    pub fn get_wallet_mut(&mut self, wallet_type: WalletType) -> Option<&mut Wallet> {
        self.wallets.get_mut(&wallet_type)
    }

    /// Lấy Spot wallet
    pub fn spot(&self) -> Option<&Wallet> {
        self.get_wallet(WalletType::Spot)
    }

    /// Lấy Funding wallet
    pub fn funding(&self) -> Option<&Wallet> {
        self.get_wallet(WalletType::Funding)
    }

    /// Lấy mutable Spot wallet
    pub fn spot_mut(&mut self) -> Option<&mut Wallet> {
        self.get_wallet_mut(WalletType::Spot)
    }

    /// Lấy mutable Funding wallet
    pub fn funding_mut(&mut self) -> Option<&mut Wallet> {
        self.get_wallet_mut(WalletType::Funding)
    }

    /// Kiểm tra account có active không
    pub fn is_active(&self) -> bool {
        self.status == AccountStatus::Active
    }

    /// Freeze account
    pub fn freeze(&mut self) {
        self.status = AccountStatus::Frozen;
    }

    /// Activate account
    pub fn activate(&mut self) {
        self.status = AccountStatus::Active;
    }

    /// Close account
    pub fn close(&mut self) {
        self.status = AccountStatus::Closed;
    }

    /// Generate ID cho account mới
    pub fn generate_id(counter: u32) -> String {
        format!("ACC_{:03}", counter)
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Account {} (owner: {}, status: {}, wallets: {})",
            self.id,
            self.person_id,
            self.status,
            self.wallets.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::Currency;
    use rust_decimal_macros::dec;

    #[test]
    fn test_account_creation() {
        let account = Account::new("ACC_001".to_string(), "CUST_001".to_string());

        assert_eq!(account.id, "ACC_001");
        assert_eq!(account.person_id, "CUST_001");
        assert_eq!(account.status, AccountStatus::Active);
        assert!(account.is_active());

        // Should have 2 wallets (Spot + Funding) by default
        assert_eq!(account.wallets.len(), 2);
        assert!(account.spot().is_some());
        assert!(account.funding().is_some());
    }

    #[test]
    fn test_account_from_person() {
        let alice = Person::customer("CUST_001", "Alice");
        let account = Account::from_person("ACC_001", &alice);

        assert_eq!(account.person_id, "CUST_001");
    }

    #[test]
    fn test_account_wallet_operations() {
        let mut account = Account::new("ACC_001".to_string(), "CUST_001".to_string());

        // Credit to funding wallet
        if let Some(funding) = account.funding_mut() {
            funding.credit(Currency::usdt(), dec!(1000));
        }

        // Verify balance
        let balance = account
            .funding()
            .and_then(|w| w.get_balance("USDT"))
            .map(|b| b.available);

        assert_eq!(balance, Some(dec!(1000)));
    }

    #[test]
    fn test_account_status_transitions() {
        let mut account = Account::new("ACC_001".to_string(), "CUST_001".to_string());

        assert!(account.is_active());

        account.freeze();
        assert_eq!(account.status, AccountStatus::Frozen);
        assert!(!account.is_active());

        account.activate();
        assert!(account.is_active());

        account.close();
        assert_eq!(account.status, AccountStatus::Closed);
    }

    #[test]
    fn test_account_id_generation() {
        assert_eq!(Account::generate_id(1), "ACC_001");
        assert_eq!(Account::generate_id(42), "ACC_042");
        assert_eq!(Account::generate_id(999), "ACC_999");
    }
}

```

## File ./simbank\crates\core\src\error.rs:
```rust
//! # Error Module
//!
//! Định nghĩa các domain errors cho Simbank sử dụng thiserror.

use crate::wallet::WalletType;
use rust_decimal::Decimal;
use thiserror::Error;

/// Core domain errors.
///
/// Các lỗi nghiệp vụ cốt lõi, không liên quan đến infrastructure.
#[derive(Debug, Error)]
pub enum CoreError {
    // === Money errors ===
    #[error("Insufficient balance: need {needed}, available {available}")]
    InsufficientBalance { needed: Decimal, available: Decimal },

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Currency mismatch: expected {expected}, got {actual}")]
    CurrencyMismatch { expected: String, actual: String },

    #[error("Unknown currency: {0}")]
    UnknownCurrency(String),

    // === Account errors ===
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Account is frozen: {0}")]
    AccountFrozen(String),

    #[error("Account is closed: {0}")]
    AccountClosed(String),

    #[error("Account already exists: {0}")]
    AccountAlreadyExists(String),

    // === Wallet errors ===
    #[error("Wallet not found: {account_id} - {wallet_type}")]
    WalletNotFound {
        account_id: String,
        wallet_type: WalletType,
    },

    #[error("Wallet is frozen: {0}")]
    WalletFrozen(String),

    #[error("Invalid wallet type for operation: {0}")]
    InvalidWalletType(String),

    #[error("Cannot transfer to same wallet")]
    SameWalletTransfer,

    // === Person errors ===
    #[error("Person not found: {0}")]
    PersonNotFound(String),

    #[error("Person already exists: {0}")]
    PersonAlreadyExists(String),

    #[error("Invalid person type: {0}")]
    InvalidPersonType(String),

    // === Permission errors ===
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Operation requires approval")]
    RequiresApproval,

    // === Validation errors ===
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Invalid ID format: {0}")]
    InvalidIdFormat(String),

    // === AML errors ===
    #[error("Transaction blocked by AML: {0}")]
    AmlBlocked(String),

    #[error("Transaction flagged for review: {0}")]
    AmlFlagged(String),
}

/// Result type alias với CoreError
pub type CoreResult<T> = Result<T, CoreError>;

impl CoreError {
    /// Kiểm tra có phải lỗi insufficient balance không
    pub fn is_insufficient_balance(&self) -> bool {
        matches!(self, CoreError::InsufficientBalance { .. })
    }

    /// Kiểm tra có phải lỗi permission không
    pub fn is_permission_error(&self) -> bool {
        matches!(
            self,
            CoreError::PermissionDenied(_) | CoreError::RequiresApproval
        )
    }

    /// Kiểm tra có phải lỗi AML không
    pub fn is_aml_error(&self) -> bool {
        matches!(self, CoreError::AmlBlocked(_) | CoreError::AmlFlagged(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_error_display() {
        let err = CoreError::InsufficientBalance {
            needed: dec!(1000),
            available: dec!(500),
        };
        assert_eq!(
            err.to_string(),
            "Insufficient balance: need 1000, available 500"
        );

        let err = CoreError::AccountNotFound("ACC_001".to_string());
        assert_eq!(err.to_string(), "Account not found: ACC_001");
    }

    #[test]
    fn test_error_checks() {
        let err = CoreError::InsufficientBalance {
            needed: dec!(100),
            available: dec!(50),
        };
        assert!(err.is_insufficient_balance());

        let err = CoreError::PermissionDenied("test".to_string());
        assert!(err.is_permission_error());

        let err = CoreError::AmlBlocked("suspicious".to_string());
        assert!(err.is_aml_error());
    }

    #[test]
    fn test_wallet_not_found() {
        let err = CoreError::WalletNotFound {
            account_id: "ACC_001".to_string(),
            wallet_type: WalletType::Spot,
        };
        assert!(err.to_string().contains("ACC_001"));
        assert!(err.to_string().contains("spot"));
    }
}

```

## File ./simbank\crates\core\src\event.rs:
```rust
//! # Event Module
//!
//! Định nghĩa Event, EventType, và EventMetadata cho Event Sourcing.
//! Events được ghi vào JSONL files để phục vụ AML compliance và audit.

use crate::person::PersonType;
use crate::wallet::WalletType;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Loại sự kiện trong hệ thống.
///
/// Mỗi event type đại diện cho một hành động đã xảy ra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // === Account events ===
    /// Tạo account mới
    AccountCreated,
    /// Đóng băng account
    AccountFrozen,
    /// Mở khóa account
    AccountActivated,
    /// Đóng account
    AccountClosed,

    // === Transaction events ===
    /// Nạp tiền vào (external -> Funding)
    Deposit,
    /// Rút tiền ra (Funding -> external)
    Withdrawal,
    /// Chuyển tiền nội bộ giữa các wallets
    InternalTransfer,
    /// Giao dịch trade (Spot)
    Trade,

    // === Business events ===
    /// Thu phí (annual fee, transaction fee)
    Fee,
    /// Trả lương cho employee
    SalaryPayment,
    /// Mua bảo hiểm
    InsurancePurchase,
    /// Chi trả cổ tức
    DividendPayment,
    /// Thưởng (bonus)
    BonusPayment,

    // === Audit events ===
    /// Kiểm toán viên truy cập dữ liệu
    AuditAccess,
    /// Tạo báo cáo audit
    AuditReportGenerated,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::AccountCreated => "account_created",
            EventType::AccountFrozen => "account_frozen",
            EventType::AccountActivated => "account_activated",
            EventType::AccountClosed => "account_closed",
            EventType::Deposit => "deposit",
            EventType::Withdrawal => "withdrawal",
            EventType::InternalTransfer => "internal_transfer",
            EventType::Trade => "trade",
            EventType::Fee => "fee",
            EventType::SalaryPayment => "salary_payment",
            EventType::InsurancePurchase => "insurance_purchase",
            EventType::DividendPayment => "dividend_payment",
            EventType::BonusPayment => "bonus_payment",
            EventType::AuditAccess => "audit_access",
            EventType::AuditReportGenerated => "audit_report_generated",
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// AML (Anti-Money Laundering) flags.
///
/// Các flag được gắn vào event để đánh dấu giao dịch đáng ngờ.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmlFlag {
    /// Giao dịch lớn (> threshold)
    LargeAmount,
    /// Gần ngưỡng báo cáo (có thể là smurfing)
    NearThreshold,
    /// Pattern bất thường (nhiều giao dịch nhỏ liên tiếp)
    UnusualPattern,
    /// Giao dịch xuyên biên giới
    CrossBorder,
    /// Từ/đến quốc gia rủi ro cao
    HighRiskCountry,
    /// Tài khoản mới với giao dịch lớn
    NewAccountLargeTx,
    /// Rút tiền nhanh sau khi nạp
    RapidWithdrawal,
}

impl AmlFlag {
    pub fn as_str(&self) -> &'static str {
        match self {
            AmlFlag::LargeAmount => "large_amount",
            AmlFlag::NearThreshold => "near_threshold",
            AmlFlag::UnusualPattern => "unusual_pattern",
            AmlFlag::CrossBorder => "cross_border",
            AmlFlag::HighRiskCountry => "high_risk_country",
            AmlFlag::NewAccountLargeTx => "new_account_large_tx",
            AmlFlag::RapidWithdrawal => "rapid_withdrawal",
        }
    }
}

/// Metadata bổ sung cho event, phục vụ truy vết và AML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventMetadata {
    /// IP address của người thực hiện
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    /// Mã quốc gia ISO (VN, US, ...)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// ID thiết bị
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// User agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Dữ liệu tùy chỉnh (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

impl EventMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ip(mut self, ip: &str) -> Self {
        self.ip_address = Some(ip.to_string());
        self
    }

    pub fn with_location(mut self, location: &str) -> Self {
        self.location = Some(location.to_string());
        self
    }

    pub fn with_device(mut self, device_id: &str) -> Self {
        self.device_id = Some(device_id.to_string());
        self
    }
}

/// Event chính - đại diện cho một sự kiện đã xảy ra trong hệ thống.
///
/// Events là immutable, append-only, và được lưu vào JSONL files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// ID unique của event (EVT_001, EVT_002, ...)
    pub event_id: String,
    /// Thời điểm xảy ra
    pub timestamp: DateTime<Utc>,
    /// Loại event
    pub event_type: EventType,

    // === Actor ===
    /// ID của người thực hiện (CUST_001, EMP_001, ...)
    pub actor_id: String,
    /// Loại actor
    pub actor_role: PersonType,

    // === Target ===
    /// ID của account liên quan
    pub account_id: String,
    /// Wallet nguồn (None nếu deposit từ external)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_wallet: Option<WalletType>,
    /// Wallet đích (None nếu withdrawal ra external)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_wallet: Option<WalletType>,

    // === Amount ===
    /// Số tiền (dạng string để đảm bảo precision)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<Decimal>,
    /// Mã tiền tệ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    // === Description ===
    /// Mô tả giao dịch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // === AML ===
    /// Các flag AML
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aml_flags: Vec<AmlFlag>,

    // === Metadata ===
    /// Thông tin bổ sung
    #[serde(default)]
    pub metadata: EventMetadata,
}

impl Event {
    /// Tạo Event mới với thông tin cơ bản
    pub fn new(
        event_id: String,
        event_type: EventType,
        actor_id: String,
        actor_role: PersonType,
        account_id: String,
    ) -> Self {
        Self {
            event_id,
            timestamp: Utc::now(),
            event_type,
            actor_id,
            actor_role,
            account_id,
            from_wallet: None,
            to_wallet: None,
            amount: None,
            currency: None,
            description: None,
            aml_flags: Vec::new(),
            metadata: EventMetadata::default(),
        }
    }

    // === Builder methods ===

    pub fn with_from_wallet(mut self, wallet: WalletType) -> Self {
        self.from_wallet = Some(wallet);
        self
    }

    pub fn with_to_wallet(mut self, wallet: WalletType) -> Self {
        self.to_wallet = Some(wallet);
        self
    }

    pub fn with_amount(mut self, amount: Decimal, currency: &str) -> Self {
        self.amount = Some(amount);
        self.currency = Some(currency.to_string());
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_aml_flag(mut self, flag: AmlFlag) -> Self {
        self.aml_flags.push(flag);
        self
    }

    pub fn with_aml_flags(mut self, flags: Vec<AmlFlag>) -> Self {
        self.aml_flags = flags;
        self
    }

    pub fn with_metadata(mut self, metadata: EventMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    // === Factory methods ===

    /// Tạo Deposit event
    pub fn deposit(
        event_id: &str,
        actor_id: &str,
        account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> Self {
        Self::new(
            event_id.to_string(),
            EventType::Deposit,
            actor_id.to_string(),
            PersonType::Customer,
            account_id.to_string(),
        )
        .with_to_wallet(WalletType::Funding)
        .with_amount(amount, currency)
    }

    /// Tạo Withdrawal event
    pub fn withdrawal(
        event_id: &str,
        actor_id: &str,
        account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> Self {
        Self::new(
            event_id.to_string(),
            EventType::Withdrawal,
            actor_id.to_string(),
            PersonType::Customer,
            account_id.to_string(),
        )
        .with_from_wallet(WalletType::Funding)
        .with_amount(amount, currency)
    }

    /// Tạo InternalTransfer event
    pub fn internal_transfer(
        event_id: &str,
        actor_id: &str,
        account_id: &str,
        from: WalletType,
        to: WalletType,
        amount: Decimal,
        currency: &str,
    ) -> Self {
        Self::new(
            event_id.to_string(),
            EventType::InternalTransfer,
            actor_id.to_string(),
            PersonType::Customer,
            account_id.to_string(),
        )
        .with_from_wallet(from)
        .with_to_wallet(to)
        .with_amount(amount, currency)
    }

    /// Generate ID cho event mới
    pub fn generate_id(counter: u64) -> String {
        format!("EVT_{:06}", counter)
    }

    /// Kiểm tra event có AML flags không
    pub fn has_aml_flags(&self) -> bool {
        !self.aml_flags.is_empty()
    }

    /// Serialize event thành JSON string (cho JSONL)
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} by {} on {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.event_type,
            self.actor_id,
            self.account_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_event_deposit() {
        let event = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(1000), "USDT");

        assert_eq!(event.event_id, "EVT_001");
        assert_eq!(event.event_type, EventType::Deposit);
        assert_eq!(event.to_wallet, Some(WalletType::Funding));
        assert_eq!(event.amount, Some(dec!(1000)));
        assert_eq!(event.currency, Some("USDT".to_string()));
    }

    #[test]
    fn test_event_internal_transfer() {
        let event = Event::internal_transfer(
            "EVT_002",
            "CUST_001",
            "ACC_001",
            WalletType::Funding,
            WalletType::Spot,
            dec!(500),
            "USDT",
        );

        assert_eq!(event.event_type, EventType::InternalTransfer);
        assert_eq!(event.from_wallet, Some(WalletType::Funding));
        assert_eq!(event.to_wallet, Some(WalletType::Spot));
    }

    #[test]
    fn test_event_with_aml_flags() {
        let event = Event::deposit("EVT_003", "CUST_001", "ACC_001", dec!(15000), "USD")
            .with_aml_flag(AmlFlag::LargeAmount)
            .with_description("Large deposit");

        assert!(event.has_aml_flags());
        assert_eq!(event.aml_flags.len(), 1);
        assert_eq!(event.aml_flags[0], AmlFlag::LargeAmount);
    }

    #[test]
    fn test_event_with_metadata() {
        let metadata = EventMetadata::new()
            .with_ip("192.168.1.1")
            .with_location("VN")
            .with_device("mobile_ios");

        let event = Event::deposit("EVT_004", "CUST_001", "ACC_001", dec!(100), "USDT")
            .with_metadata(metadata);

        assert_eq!(event.metadata.ip_address, Some("192.168.1.1".to_string()));
        assert_eq!(event.metadata.location, Some("VN".to_string()));
    }

    #[test]
    fn test_event_to_json() {
        let event = Event::deposit("EVT_005", "CUST_001", "ACC_001", dec!(100), "USDT");
        let json = event.to_json().unwrap();

        assert!(json.contains("EVT_005"));
        assert!(json.contains("deposit"));
        assert!(json.contains("USDT"));
    }

    #[test]
    fn test_event_id_generation() {
        assert_eq!(Event::generate_id(1), "EVT_000001");
        assert_eq!(Event::generate_id(999999), "EVT_999999");
    }
}

```

## File ./simbank\crates\core\src\lib.rs:
```rust
//! # Simbank Core
//!
//! Thư viện chứa các domain types cốt lõi của Simbank.
//!
//! ## Modules
//! - `money`: Currency và Money với rust_decimal
//! - `wallet`: WalletType, Wallet, Balance
//! - `person`: PersonType, Person
//! - `account`: Account
//! - `event`: Event, EventType, EventMetadata, AmlFlag
//! - `error`: Domain errors

pub mod money;
pub mod wallet;
pub mod person;
pub mod account;
pub mod event;
pub mod error;

// Re-export commonly used types
pub use money::{Currency, Money};
pub use wallet::{WalletType, Wallet, Balance};
pub use person::{PersonType, Person};
pub use account::Account;
pub use event::{Event, EventType, EventMetadata, AmlFlag};
pub use error::CoreError;

```

## File ./simbank\crates\core\src\money.rs:
```rust
//! # Money Module
//!
//! Định nghĩa Currency và Money với rust_decimal để đảm bảo độ chính xác
//! cho cả fiat (VND, USD) và crypto (BTC, ETH với 8-18 decimals).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Đại diện cho một loại tiền tệ với số decimal động.
///
/// # Examples
/// ```
/// use simbank_core::Currency;
///
/// let usd = Currency::new("USD", "US Dollar", 2, "$");
/// let btc = Currency::new("BTC", "Bitcoin", 8, "₿");
/// let eth = Currency::new("ETH", "Ethereum", 18, "Ξ");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Currency {
    /// Mã tiền tệ (ISO 4217 cho fiat, symbol cho crypto)
    pub code: String,
    /// Tên đầy đủ
    pub name: String,
    /// Số chữ số thập phân (VND=0, USD=2, BTC=8, ETH=18)
    pub decimals: u8,
    /// Ký hiệu hiển thị
    pub symbol: String,
}

impl Currency {
    /// Tạo Currency mới
    pub fn new(code: &str, name: &str, decimals: u8, symbol: &str) -> Self {
        Self {
            code: code.to_uppercase(),
            name: name.to_string(),
            decimals,
            symbol: symbol.to_string(),
        }
    }

    // === Preset currencies ===

    /// Vietnamese Dong (0 decimals)
    pub fn vnd() -> Self {
        Self::new("VND", "Vietnamese Dong", 0, "₫")
    }

    /// US Dollar (2 decimals)
    pub fn usd() -> Self {
        Self::new("USD", "US Dollar", 2, "$")
    }

    /// Tether USDT (6 decimals)
    pub fn usdt() -> Self {
        Self::new("USDT", "Tether", 6, "₮")
    }

    /// USD Coin (6 decimals)
    pub fn usdc() -> Self {
        Self::new("USDC", "USD Coin", 6, "$")
    }

    /// Bitcoin (8 decimals)
    pub fn btc() -> Self {
        Self::new("BTC", "Bitcoin", 8, "₿")
    }

    /// Ethereum (18 decimals)
    pub fn eth() -> Self {
        Self::new("ETH", "Ethereum", 18, "Ξ")
    }

    /// Dogecoin (8 decimals)
    pub fn doge() -> Self {
        Self::new("DOGE", "Dogecoin", 8, "Ð")
    }

    /// Cardano (6 decimals)
    pub fn ada() -> Self {
        Self::new("ADA", "Cardano", 6, "₳")
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)
    }
}

/// Đại diện cho một số tiền với currency và amount.
///
/// Sử dụng `rust_decimal::Decimal` để đảm bảo độ chính xác tuyệt đối
/// cho các phép tính tài chính.
///
/// # Examples
/// ```
/// use simbank_core::{Money, Currency};
/// use rust_decimal_macros::dec;
///
/// let usd = Currency::usd();
/// let amount = Money::new(dec!(100.50), usd);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Số tiền (dạng Decimal, serialize thành String trong JSON)
    pub amount: Decimal,
    /// Loại tiền tệ
    pub currency: Currency,
}

impl Money {
    /// Tạo Money mới
    pub fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Tạo Money với amount = 0
    pub fn zero(currency: Currency) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }

    /// Tạo Money từ f64 (chỉ dùng cho test/demo, production nên dùng Decimal)
    pub fn from_f64(amount: f64, currency: Currency) -> Self {
        Self {
            amount: Decimal::try_from(amount).unwrap_or(Decimal::ZERO),
            currency,
        }
    }

    /// Kiểm tra có phải là số dương
    pub fn is_positive(&self) -> bool {
        self.amount > Decimal::ZERO
    }

    /// Kiểm tra có phải là 0
    pub fn is_zero(&self) -> bool {
        self.amount == Decimal::ZERO
    }

    /// Kiểm tra có phải là số âm
    pub fn is_negative(&self) -> bool {
        self.amount < Decimal::ZERO
    }

    /// Cộng hai Money cùng currency
    ///
    /// # Panics
    /// Panic nếu currency khác nhau
    pub fn add(&self, other: &Money) -> Money {
        assert_eq!(
            self.currency.code, other.currency.code,
            "Cannot add different currencies: {} vs {}",
            self.currency.code, other.currency.code
        );
        Money {
            amount: self.amount + other.amount,
            currency: self.currency.clone(),
        }
    }

    /// Trừ hai Money cùng currency
    ///
    /// # Panics
    /// Panic nếu currency khác nhau
    pub fn sub(&self, other: &Money) -> Money {
        assert_eq!(
            self.currency.code, other.currency.code,
            "Cannot subtract different currencies: {} vs {}",
            self.currency.code, other.currency.code
        );
        Money {
            amount: self.amount - other.amount,
            currency: self.currency.clone(),
        }
    }

    /// Nhân với một số
    pub fn mul(&self, multiplier: Decimal) -> Money {
        Money {
            amount: self.amount * multiplier,
            currency: self.currency.clone(),
        }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency.code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_currency_presets() {
        let usd = Currency::usd();
        assert_eq!(usd.code, "USD");
        assert_eq!(usd.decimals, 2);

        let btc = Currency::btc();
        assert_eq!(btc.code, "BTC");
        assert_eq!(btc.decimals, 8);

        let eth = Currency::eth();
        assert_eq!(eth.code, "ETH");
        assert_eq!(eth.decimals, 18);
    }

    #[test]
    fn test_money_add() {
        let usd = Currency::usd();
        let a = Money::new(dec!(100.50), usd.clone());
        let b = Money::new(dec!(50.25), usd);
        let result = a.add(&b);
        assert_eq!(result.amount, dec!(150.75));
    }

    #[test]
    fn test_money_sub() {
        let usd = Currency::usd();
        let a = Money::new(dec!(100.00), usd.clone());
        let b = Money::new(dec!(30.50), usd);
        let result = a.sub(&b);
        assert_eq!(result.amount, dec!(69.50));
    }

    #[test]
    #[should_panic(expected = "Cannot add different currencies")]
    fn test_money_add_different_currencies_panics() {
        let usd = Money::new(dec!(100), Currency::usd());
        let btc = Money::new(dec!(1), Currency::btc());
        usd.add(&btc);
    }

    #[test]
    fn test_money_display() {
        let money = Money::new(dec!(1234.56), Currency::usd());
        assert_eq!(format!("{}", money), "1234.56 USD");
    }

    #[test]
    fn test_high_precision_eth() {
        // ETH có 18 decimals - test precision
        let eth = Currency::eth();
        let wei = Money::new(dec!(0.000000000000000001), eth);
        assert!(wei.is_positive());
        assert_eq!(wei.amount, dec!(0.000000000000000001));
    }
}

```

## File ./simbank\crates\core\src\person.rs:
```rust
//! # Person Module
//!
//! Định nghĩa PersonType và Person cho các vai trò trong hệ thống.
//! - Customer: Khách hàng với đầy đủ wallets
//! - Employee: Nhân viên với Funding wallet
//! - Shareholder: Cổ đông nhận cổ tức
//! - Manager: Quản lý phê duyệt operations
//! - Auditor: Kiểm toán viên (Big 4) read-only

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Loại người dùng trong hệ thống.
///
/// Mỗi loại có quyền hạn và wallets khác nhau:
/// - Customer: Full wallets (Spot, Funding)
/// - Employee: Funding only (lương, bảo hiểm)
/// - Shareholder: Funding only (cổ tức)
/// - Manager: Không có wallet, chỉ có permissions
/// - Auditor: Không có wallet, read-only access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonType {
    /// Khách hàng - có đầy đủ wallets, có thể trade
    Customer,
    /// Nhân viên ngân hàng - nhận lương, mua bảo hiểm
    Employee,
    /// Cổ đông - nhận cổ tức
    Shareholder,
    /// Quản lý - phê duyệt bonus, xem reports
    Manager,
    /// Kiểm toán viên (Deloitte, PwC, EY, KPMG) - read-only
    Auditor,
}

impl PersonType {
    /// Trả về code string cho DB
    pub fn as_str(&self) -> &'static str {
        match self {
            PersonType::Customer => "customer",
            PersonType::Employee => "employee",
            PersonType::Shareholder => "shareholder",
            PersonType::Manager => "manager",
            PersonType::Auditor => "auditor",
        }
    }

    /// Parse từ string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "customer" => Some(PersonType::Customer),
            "employee" => Some(PersonType::Employee),
            "shareholder" => Some(PersonType::Shareholder),
            "manager" => Some(PersonType::Manager),
            "auditor" => Some(PersonType::Auditor),
            _ => None,
        }
    }

    /// Kiểm tra PersonType có cần account/wallets không
    pub fn has_account(&self) -> bool {
        matches!(
            self,
            PersonType::Customer | PersonType::Employee | PersonType::Shareholder
        )
    }

    /// Kiểm tra có quyền phê duyệt operations không
    pub fn can_approve(&self) -> bool {
        matches!(self, PersonType::Manager)
    }

    /// Kiểm tra có quyền audit/read events không
    pub fn can_audit(&self) -> bool {
        matches!(self, PersonType::Auditor | PersonType::Manager)
    }
}

impl fmt::Display for PersonType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Thông tin người dùng trong hệ thống.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    /// ID của person (CUST_001, EMP_001, AUDIT_001, ...)
    pub id: String,
    /// Loại người dùng
    pub person_type: PersonType,
    /// Tên đầy đủ
    pub name: String,
    /// Email (optional)
    pub email: Option<String>,
    /// Thời gian tạo
    pub created_at: DateTime<Utc>,
}

impl Person {
    /// Tạo Person mới
    pub fn new(id: String, person_type: PersonType, name: String) -> Self {
        Self {
            id,
            person_type,
            name,
            email: None,
            created_at: Utc::now(),
        }
    }

    /// Tạo Person với email
    pub fn with_email(mut self, email: String) -> Self {
        self.email = Some(email);
        self
    }

    /// Tạo Customer
    pub fn customer(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Customer, name.to_string())
    }

    /// Tạo Employee
    pub fn employee(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Employee, name.to_string())
    }

    /// Tạo Shareholder
    pub fn shareholder(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Shareholder, name.to_string())
    }

    /// Tạo Manager
    pub fn manager(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Manager, name.to_string())
    }

    /// Tạo Auditor (Big 4)
    pub fn auditor(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Auditor, name.to_string())
    }

    /// Kiểm tra có cần tạo account không
    pub fn needs_account(&self) -> bool {
        self.person_type.has_account()
    }

    /// Generate prefix cho ID dựa trên PersonType
    pub fn id_prefix(person_type: PersonType) -> &'static str {
        match person_type {
            PersonType::Customer => "CUST",
            PersonType::Employee => "EMP",
            PersonType::Shareholder => "SH",
            PersonType::Manager => "MGR",
            PersonType::Auditor => "AUDIT",
        }
    }
}

impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} - {})", self.name, self.id, self.person_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person_type_str() {
        assert_eq!(PersonType::Customer.as_str(), "customer");
        assert_eq!(PersonType::Auditor.as_str(), "auditor");
        assert_eq!(PersonType::from_str("CUSTOMER"), Some(PersonType::Customer));
        assert_eq!(PersonType::from_str("unknown"), None);
    }

    #[test]
    fn test_person_type_permissions() {
        assert!(PersonType::Customer.has_account());
        assert!(PersonType::Employee.has_account());
        assert!(PersonType::Shareholder.has_account());
        assert!(!PersonType::Manager.has_account());
        assert!(!PersonType::Auditor.has_account());

        assert!(PersonType::Manager.can_approve());
        assert!(!PersonType::Customer.can_approve());

        assert!(PersonType::Auditor.can_audit());
        assert!(PersonType::Manager.can_audit());
        assert!(!PersonType::Customer.can_audit());
    }

    #[test]
    fn test_person_creation() {
        let alice = Person::customer("CUST_001", "Alice");
        assert_eq!(alice.id, "CUST_001");
        assert_eq!(alice.person_type, PersonType::Customer);
        assert!(alice.needs_account());

        let deloitte = Person::auditor("AUDIT_001", "Deloitte");
        assert_eq!(deloitte.person_type, PersonType::Auditor);
        assert!(!deloitte.needs_account());
    }

    #[test]
    fn test_person_with_email() {
        let bob = Person::employee("EMP_001", "Bob").with_email("bob@simbank.com".to_string());

        assert_eq!(bob.email, Some("bob@simbank.com".to_string()));
    }

    #[test]
    fn test_person_display() {
        let person = Person::customer("CUST_001", "Alice");
        assert_eq!(format!("{}", person), "Alice (CUST_001 - customer)");
    }
}

```

## File ./simbank\crates\core\src\wallet.rs:
```rust
//! # Wallet Module
//!
//! Định nghĩa WalletType, Wallet, và Balance cho mô hình Exchange-style.
//! Mỗi Account có nhiều Wallets (Spot, Funding, Margin, Futures, Earn),
//! mỗi Wallet chứa nhiều loại tiền tệ.

use crate::money::{Currency, Money};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Loại ví trong hệ thống Exchange-style.
///
/// Phase 1: Chỉ implement Spot + Funding
/// Phase 2: Margin, Futures, Earn
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalletType {
    /// Ví giao dịch Spot
    Spot,
    /// Ví nạp/rút tiền
    Funding,
    /// Ví giao dịch ký quỹ (Phase 2)
    Margin,
    /// Ví hợp đồng tương lai (Phase 2)
    Futures,
    /// Ví staking/savings (Phase 2)
    Earn,
}

impl WalletType {
    /// Trả về code string cho DB
    pub fn as_str(&self) -> &'static str {
        match self {
            WalletType::Spot => "spot",
            WalletType::Funding => "funding",
            WalletType::Margin => "margin",
            WalletType::Futures => "futures",
            WalletType::Earn => "earn",
        }
    }

    /// Parse từ string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "spot" => Some(WalletType::Spot),
            "funding" => Some(WalletType::Funding),
            "margin" => Some(WalletType::Margin),
            "futures" => Some(WalletType::Futures),
            "earn" => Some(WalletType::Earn),
            _ => None,
        }
    }

    /// Các wallet types cho Phase 1
    pub fn phase1_types() -> Vec<WalletType> {
        vec![WalletType::Spot, WalletType::Funding]
    }

    /// Tất cả wallet types
    pub fn all_types() -> Vec<WalletType> {
        vec![
            WalletType::Spot,
            WalletType::Funding,
            WalletType::Margin,
            WalletType::Futures,
            WalletType::Earn,
        ]
    }
}

impl fmt::Display for WalletType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Số dư của một loại tiền trong wallet.
///
/// - `available`: Số dư khả dụng, có thể sử dụng ngay
/// - `locked`: Số dư bị khóa (đang trong order, staking, margin)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    /// Currency của balance này
    pub currency: Currency,
    /// Số dư khả dụng
    pub available: Decimal,
    /// Số dư bị khóa (Phase 2)
    pub locked: Decimal,
    /// Thời gian cập nhật cuối
    pub updated_at: DateTime<Utc>,
}

impl Balance {
    /// Tạo Balance mới với available = 0, locked = 0
    pub fn new(currency: Currency) -> Self {
        Self {
            currency,
            available: Decimal::ZERO,
            locked: Decimal::ZERO,
            updated_at: Utc::now(),
        }
    }

    /// Tạo Balance với số dư khởi tạo
    pub fn with_amount(currency: Currency, available: Decimal) -> Self {
        Self {
            currency,
            available,
            locked: Decimal::ZERO,
            updated_at: Utc::now(),
        }
    }

    /// Tổng số dư (available + locked)
    pub fn total(&self) -> Decimal {
        self.available + self.locked
    }

    /// Kiểm tra có đủ số dư available để thực hiện giao dịch
    pub fn can_spend(&self, amount: Decimal) -> bool {
        self.available >= amount
    }

    /// Cộng thêm vào available
    pub fn credit(&mut self, amount: Decimal) {
        self.available += amount;
        self.updated_at = Utc::now();
    }

    /// Trừ từ available
    ///
    /// # Returns
    /// - `Ok(())` nếu thành công
    /// - `Err(amount_needed)` nếu không đủ số dư
    pub fn debit(&mut self, amount: Decimal) -> Result<(), Decimal> {
        if self.available >= amount {
            self.available -= amount;
            self.updated_at = Utc::now();
            Ok(())
        } else {
            Err(amount - self.available)
        }
    }

    /// Chuyển sang Money object
    pub fn as_money(&self) -> Money {
        Money::new(self.available, self.currency.clone())
    }
}

impl fmt::Display for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.locked > Decimal::ZERO {
            write!(
                f,
                "{} {} (locked: {})",
                self.available, self.currency.code, self.locked
            )
        } else {
            write!(f, "{} {}", self.available, self.currency.code)
        }
    }
}

/// Ví của người dùng.
///
/// Mỗi ví thuộc một loại (Spot, Funding, ...) và chứa nhiều loại tiền tệ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    /// ID của wallet (WAL_001, WAL_002, ...)
    pub id: String,
    /// ID của account sở hữu
    pub account_id: String,
    /// Loại ví
    pub wallet_type: WalletType,
    /// Map từ currency code -> Balance
    pub balances: HashMap<String, Balance>,
    /// Trạng thái ví
    pub status: WalletStatus,
    /// Thời gian tạo
    pub created_at: DateTime<Utc>,
}

/// Trạng thái của ví
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalletStatus {
    Active,
    Frozen,
    Closed,
}

impl WalletStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WalletStatus::Active => "active",
            WalletStatus::Frozen => "frozen",
            WalletStatus::Closed => "closed",
        }
    }
}

impl Wallet {
    /// Tạo Wallet mới
    pub fn new(id: String, account_id: String, wallet_type: WalletType) -> Self {
        Self {
            id,
            account_id,
            wallet_type,
            balances: HashMap::new(),
            status: WalletStatus::Active,
            created_at: Utc::now(),
        }
    }

    /// Lấy balance của một currency
    pub fn get_balance(&self, currency_code: &str) -> Option<&Balance> {
        self.balances.get(currency_code)
    }

    /// Lấy hoặc tạo balance cho currency
    pub fn get_or_create_balance(&mut self, currency: Currency) -> &mut Balance {
        let code = currency.code.clone();
        self.balances
            .entry(code)
            .or_insert_with(|| Balance::new(currency))
    }

    /// Credit (cộng tiền) vào wallet
    pub fn credit(&mut self, currency: Currency, amount: Decimal) {
        let balance = self.get_or_create_balance(currency);
        balance.credit(amount);
    }

    /// Debit (trừ tiền) từ wallet
    ///
    /// # Returns
    /// - `Ok(())` nếu thành công
    /// - `Err(amount_needed)` nếu không đủ số dư
    pub fn debit(&mut self, currency_code: &str, amount: Decimal) -> Result<(), Decimal> {
        if let Some(balance) = self.balances.get_mut(currency_code) {
            balance.debit(amount)
        } else {
            Err(amount) // Không có balance = thiếu toàn bộ amount
        }
    }

    /// Kiểm tra ví có active không
    pub fn is_active(&self) -> bool {
        self.status == WalletStatus::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_wallet_type_str() {
        assert_eq!(WalletType::Spot.as_str(), "spot");
        assert_eq!(WalletType::Funding.as_str(), "funding");
        assert_eq!(WalletType::from_str("SPOT"), Some(WalletType::Spot));
        assert_eq!(WalletType::from_str("unknown"), None);
    }

    #[test]
    fn test_balance_operations() {
        let mut balance = Balance::new(Currency::usd());
        assert_eq!(balance.available, dec!(0));

        balance.credit(dec!(100));
        assert_eq!(balance.available, dec!(100));

        assert!(balance.can_spend(dec!(50)));
        assert!(!balance.can_spend(dec!(150)));

        assert!(balance.debit(dec!(30)).is_ok());
        assert_eq!(balance.available, dec!(70));

        assert!(balance.debit(dec!(100)).is_err());
    }

    #[test]
    fn test_wallet_multi_currency() {
        let mut wallet = Wallet::new(
            "WAL_001".to_string(),
            "ACC_001".to_string(),
            WalletType::Spot,
        );

        wallet.credit(Currency::usd(), dec!(100));
        wallet.credit(Currency::btc(), dec!(0.5));
        wallet.credit(Currency::usd(), dec!(50)); // Thêm vào USD có sẵn

        assert_eq!(wallet.get_balance("USD").unwrap().available, dec!(150));
        assert_eq!(wallet.get_balance("BTC").unwrap().available, dec!(0.5));
        assert!(wallet.get_balance("ETH").is_none());
    }

    #[test]
    fn test_wallet_debit() {
        let mut wallet = Wallet::new(
            "WAL_001".to_string(),
            "ACC_001".to_string(),
            WalletType::Funding,
        );

        wallet.credit(Currency::usdt(), dec!(1000));

        assert!(wallet.debit("USDT", dec!(300)).is_ok());
        assert_eq!(wallet.get_balance("USDT").unwrap().available, dec!(700));

        // Không đủ tiền
        let result = wallet.debit("USDT", dec!(1000));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), dec!(300)); // Thiếu 300

        // Currency không tồn tại
        let result = wallet.debit("BTC", dec!(1));
        assert!(result.is_err());
    }
}

```

## File ./simbank\crates\dsl\src\lib.rs:
```rust
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

```

## File ./simbank\crates\dsl\src\rules.rs:
```rust
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

```

## File ./simbank\crates\dsl\src\scenario.rs:
```rust
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

```

## File ./simbank\crates\persistence\src\error.rs:
```rust
//! # Persistence Errors
//!
//! Error types cho persistence layer, wrapping sqlx và IO errors.

use thiserror::Error;

/// Persistence layer errors
#[derive(Debug, Error)]
pub enum PersistenceError {
    // === Database errors ===
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Record not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("Record already exists: {entity} with id {id}")]
    AlreadyExists { entity: String, id: String },

    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("Unique constraint violation: {0}")]
    UniqueViolation(String),

    // === Event store errors ===
    #[error("Event store IO error: {0}")]
    EventStoreIo(#[from] std::io::Error),

    #[error("Event serialization error: {0}")]
    EventSerialization(#[from] serde_json::Error),

    #[error("Event file not found: {0}")]
    EventFileNotFound(String),

    // === Conversion errors ===
    #[error("Invalid decimal value: {0}")]
    InvalidDecimal(String),

    #[error("Invalid enum value: {field} = {value}")]
    InvalidEnumValue { field: String, value: String },

    // === Configuration errors ===
    #[error("Configuration error: {0}")]
    Configuration(String),

    // === Other errors ===
    #[error("{0}")]
    Other(String),
}

/// Result type alias cho PersistenceError
pub type PersistenceResult<T> = Result<T, PersistenceError>;

impl PersistenceError {
    /// Tạo NotFound error
    pub fn not_found(entity: &str, id: &str) -> Self {
        Self::NotFound {
            entity: entity.to_string(),
            id: id.to_string(),
        }
    }

    /// Tạo AlreadyExists error
    pub fn already_exists(entity: &str, id: &str) -> Self {
        Self::AlreadyExists {
            entity: entity.to_string(),
            id: id.to_string(),
        }
    }

    /// Kiểm tra có phải lỗi not found không
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }

    /// Kiểm tra có phải lỗi database không
    pub fn is_database_error(&self) -> bool {
        matches!(self, Self::Database(_))
    }
}

```

## File ./simbank\crates\persistence\src\lib.rs:
```rust
//! # Simbank Persistence
//!
//! Persistence layer cho Simbank - SQLite + JSONL Event Store.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      Database                               │
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────┐ │
//! │  │   SQLite    │    │    JSONL    │    │     Repos       │ │
//! │  │  (state)    │    │  (events)   │    │   (queries)     │ │
//! │  └─────────────┘    └─────────────┘    └─────────────────┘ │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use simbank_persistence::{Database, EventStore};
//!
//! // Initialize database
//! let db = Database::new("simbank.db", "data/events").await?;
//!
//! // Query via repos
//! let accounts = AccountRepo::get_all(db.pool()).await?;
//!
//! // Append events
//! db.events().append(&event)?;
//! ```

pub mod error;
pub mod events;
pub mod sqlite;

pub use error::{PersistenceError, PersistenceResult};
pub use events::{AmlReport, EventFilter, EventReader, EventStore};
pub use sqlite::{
    init_database, AccountRepo, BalanceRepo, CurrencyRepo, PersonRepo, TransactionRepo,
    WalletRepo,
};
pub use sqlite::schema::{
    AccountRow, BalanceRow, CurrencyRow, PersonRow, TransactionRow, WalletRow,
};

use sqlx::SqlitePool;
use std::path::Path;

/// Database facade - unified access to SQLite + Events
pub struct Database {
    pool: SqlitePool,
    event_store: EventStore,
}

impl Database {
    /// Create new database connection
    ///
    /// # Arguments
    /// * `db_url` - SQLite database URL (e.g., "sqlite:simbank.db?mode=rwc")
    /// * `events_path` - Path to JSONL events directory
    pub async fn new<Q: AsRef<Path>>(
        db_url: &str,
        events_path: Q,
    ) -> PersistenceResult<Self> {
        let pool = sqlite::create_pool(db_url).await?;
        let event_store = EventStore::new(events_path)?;

        Ok(Self { pool, event_store })
    }

    /// Initialize database with migrations and seed data
    pub async fn init_with_migrations<Q: AsRef<Path>>(
        db_url: &str,
        events_path: Q,
    ) -> PersistenceResult<Self> {
        let pool = init_database(db_url).await?;
        let event_store = EventStore::new(events_path)?;

        Ok(Self { pool, event_store })
    }

    /// Get SQLite connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get event store
    pub fn events(&self) -> &EventStore {
        &self.event_store
    }

    /// Event reader for replaying/auditing
    pub fn event_reader(&self) -> EventReader {
        EventReader::new(self.event_store.base_path())
    }
}

```

## File ./simbank\crates\persistence\src\events\mod.rs:
```rust
//! Event Sourcing module
//!
//! Ghi và đọc events từ JSONL files cho AML compliance.

pub mod replay;
pub mod store;

pub use replay::{AmlReport, EventFilter, EventReader};
pub use store::EventStore;

```

## File ./simbank\crates\persistence\src\events\replay.rs:
```rust
//! Event Replay - read events from JSONL files
//!
//! Đọc events từ JSONL files để replay, audit, và AML analysis.

use crate::error::{PersistenceError, PersistenceResult};
use chrono::NaiveDate;
use simbank_core::{AmlFlag, Event, EventType};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Event Reader - đọc events từ files JSONL
pub struct EventReader {
    base_path: PathBuf,
}

impl EventReader {
    /// Tạo reader mới
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Đọc tất cả events từ một file
    pub fn read_file(&self, file_path: &Path) -> PersistenceResult<Vec<Event>> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Event = serde_json::from_str(&line)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Đọc events theo ngày
    pub fn read_date(&self, date: &str) -> PersistenceResult<Vec<Event>> {
        let file_path = self.base_path.join(format!("{}.jsonl", date));
        if file_path.exists() {
            self.read_file(&file_path)
        } else {
            Ok(Vec::new())
        }
    }

    /// Đọc events trong khoảng thời gian
    pub fn read_range(&self, from: &str, to: &str) -> PersistenceResult<Vec<Event>> {
        let from_date = NaiveDate::parse_from_str(from, "%Y-%m-%d")
            .map_err(|e| PersistenceError::Other(format!("Invalid from date: {}", e)))?;
        let to_date = NaiveDate::parse_from_str(to, "%Y-%m-%d")
            .map_err(|e| PersistenceError::Other(format!("Invalid to date: {}", e)))?;

        let mut all_events = Vec::new();
        let mut current = from_date;

        while current <= to_date {
            let date_str = current.format("%Y-%m-%d").to_string();
            let events = self.read_date(&date_str)?;
            all_events.extend(events);
            current = current.succ_opt().unwrap_or(current);
        }

        Ok(all_events)
    }

    /// Đọc tất cả events
    pub fn read_all(&self) -> PersistenceResult<Vec<Event>> {
        let mut all_events = Vec::new();

        if !self.base_path.exists() {
            return Ok(all_events);
        }

        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.base_path)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "jsonl"))
            .collect();

        files.sort();

        for file_path in files {
            let events = self.read_file(&file_path)?;
            all_events.extend(events);
        }

        Ok(all_events)
    }
}

/// Event Filter - lọc events theo điều kiện
#[derive(Default)]
pub struct EventFilter {
    /// Lọc theo account ID
    pub account_id: Option<String>,
    /// Lọc theo actor ID (person who performed action)
    pub actor_id: Option<String>,
    /// Lọc theo event types
    pub event_types: Option<Vec<EventType>>,
    /// Lọc theo AML flags
    pub aml_flags: Option<Vec<AmlFlag>>,
    /// Chỉ lấy events có AML flag
    pub only_flagged: bool,
    /// Minimum amount
    pub min_amount: Option<rust_decimal::Decimal>,
    /// Maximum amount
    pub max_amount: Option<rust_decimal::Decimal>,
}

impl EventFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn account(mut self, account_id: &str) -> Self {
        self.account_id = Some(account_id.to_string());
        self
    }

    pub fn actor(mut self, actor_id: &str) -> Self {
        self.actor_id = Some(actor_id.to_string());
        self
    }

    pub fn event_types(mut self, types: Vec<EventType>) -> Self {
        self.event_types = Some(types);
        self
    }

    pub fn aml_flags(mut self, flags: Vec<AmlFlag>) -> Self {
        self.aml_flags = Some(flags);
        self
    }

    pub fn flagged_only(mut self) -> Self {
        self.only_flagged = true;
        self
    }

    pub fn amount_range(mut self, min: rust_decimal::Decimal, max: rust_decimal::Decimal) -> Self {
        self.min_amount = Some(min);
        self.max_amount = Some(max);
        self
    }

    /// Kiểm tra event có match filter không
    pub fn matches(&self, event: &Event) -> bool {
        // Account filter
        if let Some(ref acc_id) = self.account_id {
            if event.account_id != *acc_id {
                return false;
            }
        }

        // Actor filter
        if let Some(ref actor_id) = self.actor_id {
            if event.actor_id != *actor_id {
                return false;
            }
        }

        // Event type filter
        if let Some(ref types) = self.event_types {
            if !types.contains(&event.event_type) {
                return false;
            }
        }

        // AML flag filter
        if let Some(ref flags) = self.aml_flags {
            // Check if event has any of the specified flags
            let has_matching_flag = event.aml_flags.iter().any(|f| flags.contains(f));
            if !has_matching_flag {
                return false;
            }
        }

        // Only flagged filter
        if self.only_flagged && event.aml_flags.is_empty() {
            return false;
        }

        // Amount range filter
        if let Some(amount) = event.amount {
            if let Some(min) = self.min_amount {
                if amount < min {
                    return false;
                }
            }
            if let Some(max) = self.max_amount {
                if amount > max {
                    return false;
                }
            }
        }

        true
    }

    /// Apply filter to events
    pub fn apply(&self, events: Vec<Event>) -> Vec<Event> {
        events.into_iter().filter(|e| self.matches(e)).collect()
    }
}

/// AML Report - báo cáo cho Anti-Money Laundering
pub struct AmlReport {
    pub total_events: usize,
    pub flagged_events: usize,
    pub large_amount_count: usize,
    pub unusual_pattern_count: usize,
    pub high_risk_country_count: usize,
    pub events_by_flag: std::collections::HashMap<String, Vec<Event>>,
}

impl AmlReport {
    /// Tạo AML report từ events
    pub fn generate(events: &[Event]) -> Self {
        let mut report = Self {
            total_events: events.len(),
            flagged_events: 0,
            large_amount_count: 0,
            unusual_pattern_count: 0,
            high_risk_country_count: 0,
            events_by_flag: std::collections::HashMap::new(),
        };

        for event in events {
            if !event.aml_flags.is_empty() {
                report.flagged_events += 1;

                for flag in &event.aml_flags {
                    match flag {
                        AmlFlag::LargeAmount => report.large_amount_count += 1,
                        AmlFlag::UnusualPattern => report.unusual_pattern_count += 1,
                        AmlFlag::HighRiskCountry => report.high_risk_country_count += 1,
                        _ => {}
                    }

                    report
                        .events_by_flag
                        .entry(flag.as_str().to_string())
                        .or_insert_with(Vec::new)
                        .push(event.clone());
                }
            }
        }

        report
    }

    /// Summary text
    pub fn summary(&self) -> String {
        format!(
            "AML Report:\n\
             - Total events: {}\n\
             - Flagged events: {} ({:.1}%)\n\
             - Large amount: {}\n\
             - Unusual pattern: {}\n\
             - High risk country: {}",
            self.total_events,
            self.flagged_events,
            (self.flagged_events as f64 / self.total_events.max(1) as f64) * 100.0,
            self.large_amount_count,
            self.unusual_pattern_count,
            self.high_risk_country_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventStore;
    use rust_decimal_macros::dec;
    use tempfile::tempdir;

    #[test]
    fn test_event_reader() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path()).unwrap();

        // Write some events
        let event1 = Event::deposit(&store.next_event_id(), "CUST_001", "ACC_001", dec!(100), "USDT");
        let event2 = Event::withdrawal(&store.next_event_id(), "CUST_001", "ACC_001", dec!(50), "USDT");
        store.append(&event1).unwrap();
        store.append(&event2).unwrap();
        store.flush().unwrap();

        // Read back
        let reader = EventReader::new(dir.path());
        let events = reader.read_all().unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, EventType::Deposit);
        assert_eq!(events[1].event_type, EventType::Withdrawal);
    }

    #[test]
    fn test_event_filter() {
        let event1 = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(100), "USDT");
        let event2 = Event::deposit("EVT_002", "CUST_002", "ACC_002", dec!(200), "USDT");
        let event3 = Event::withdrawal("EVT_003", "CUST_001", "ACC_001", dec!(50), "USDT");

        let events = vec![event1, event2, event3];

        // Filter by account
        let filter = EventFilter::new().account("ACC_001");
        let filtered = filter.apply(events.clone());
        assert_eq!(filtered.len(), 2);

        // Filter by actor
        let filter = EventFilter::new().actor("CUST_002");
        let filtered = filter.apply(events.clone());
        assert_eq!(filtered.len(), 1);

        // Filter by event type
        let filter = EventFilter::new().event_types(vec![EventType::Withdrawal]);
        let filtered = filter.apply(events.clone());
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_aml_report() {
        let event1 = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(100000), "USDT")
            .with_aml_flag(AmlFlag::LargeAmount);

        let event2 = Event::deposit("EVT_002", "CUST_002", "ACC_002", dec!(200), "USDT")
            .with_aml_flag(AmlFlag::UnusualPattern);

        let event3 = Event::withdrawal("EVT_003", "CUST_001", "ACC_001", dec!(50), "USDT");

        let events = vec![event1, event2, event3];
        let report = AmlReport::generate(&events);

        assert_eq!(report.total_events, 3);
        assert_eq!(report.flagged_events, 2);
        assert_eq!(report.large_amount_count, 1);
        assert_eq!(report.unusual_pattern_count, 1);
    }
}

```

## File ./simbank\crates\persistence\src\events\store.rs:
```rust
//! JSONL Event Store - append-only writer
//!
//! Ghi events vào files JSONL theo ngày để phục vụ AML audit trail.

use crate::error::PersistenceResult;
use chrono::Utc;
use simbank_core::Event;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Event Store - ghi events vào files JSONL.
///
/// Files được tổ chức theo ngày: `data/events/2026-01-25.jsonl`
pub struct EventStore {
    /// Thư mục chứa event files
    base_path: PathBuf,
    /// Counter cho event ID
    event_counter: AtomicU64,
    /// Current file writer (thread-safe)
    current_writer: Mutex<Option<EventWriter>>,
}

struct EventWriter {
    date: String,
    writer: BufWriter<File>,
}

impl EventStore {
    /// Tạo EventStore mới
    ///
    /// # Arguments
    /// * `base_path` - Đường dẫn thư mục chứa events (e.g., "data/events")
    pub fn new<P: AsRef<Path>>(base_path: P) -> PersistenceResult<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Tạo thư mục nếu chưa có
        fs::create_dir_all(&base_path)?;

        // Đọc event counter từ existing files
        let event_counter = Self::load_event_counter(&base_path)?;

        Ok(Self {
            base_path,
            event_counter: AtomicU64::new(event_counter),
            current_writer: Mutex::new(None),
        })
    }

    /// Lấy base path
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Load event counter từ files hiện có
    fn load_event_counter(base_path: &Path) -> PersistenceResult<u64> {
        let mut max_id: u64 = 0;

        if let Ok(entries) = fs::read_dir(base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "jsonl") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        for line in content.lines() {
                            if let Ok(event) = serde_json::from_str::<Event>(line) {
                                // Parse event ID: EVT_000123 -> 123
                                if let Some(num_str) = event.event_id.strip_prefix("EVT_") {
                                    if let Ok(num) = num_str.parse::<u64>() {
                                        max_id = max_id.max(num);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(max_id + 1)
    }

    /// Lấy file path cho ngày hiện tại
    fn get_file_path(&self, date: &str) -> PathBuf {
        self.base_path.join(format!("{}.jsonl", date))
    }

    /// Lấy ngày hiện tại dạng string
    fn current_date() -> String {
        Utc::now().format("%Y-%m-%d").to_string()
    }

    /// Generate event ID mới
    pub fn next_event_id(&self) -> String {
        let id = self.event_counter.fetch_add(1, Ordering::SeqCst);
        format!("EVT_{:06}", id)
    }

    /// Ghi event vào store
    pub fn append(&self, event: &Event) -> PersistenceResult<()> {
        let date = Self::current_date();
        let json = serde_json::to_string(event)?;

        let mut guard = self.current_writer.lock().unwrap();

        // Kiểm tra cần tạo file mới không
        let needs_new_file = guard
            .as_ref()
            .map_or(true, |w| w.date != date);

        if needs_new_file {
            let path = self.get_file_path(&date);
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            let writer = BufWriter::new(file);
            *guard = Some(EventWriter {
                date: date.clone(),
                writer,
            });
        }

        // Ghi event
        if let Some(ref mut w) = *guard {
            writeln!(w.writer, "{}", json)?;
            w.writer.flush()?;
        }

        Ok(())
    }

    /// Ghi nhiều events
    pub fn append_batch(&self, events: &[Event]) -> PersistenceResult<()> {
        for event in events {
            self.append(event)?;
        }
        Ok(())
    }

    /// Lấy tất cả event files
    pub fn list_files(&self) -> PersistenceResult<Vec<PathBuf>> {
        let mut files = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "jsonl") {
                files.push(path);
            }
        }

        files.sort();
        Ok(files)
    }

    /// Lấy file path theo ngày
    pub fn get_file_for_date(&self, date: &str) -> Option<PathBuf> {
        let path = self.get_file_path(date);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Flush tất cả pending writes
    pub fn flush(&self) -> PersistenceResult<()> {
        let mut guard = self.current_writer.lock().unwrap();
        if let Some(ref mut w) = *guard {
            w.writer.flush()?;
        }
        Ok(())
    }
}

impl Drop for EventStore {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    #[allow(unused_imports)]
    use simbank_core::PersonType;
    use tempfile::tempdir;

    #[test]
    fn test_event_store_append() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path()).unwrap();

        let event_id = store.next_event_id();
        let event = Event::deposit(&event_id, "CUST_001", "ACC_001", dec!(100), "USDT");

        store.append(&event).unwrap();
        store.flush().unwrap();

        // Verify file exists
        let files = store.list_files().unwrap();
        assert_eq!(files.len(), 1);

        // Verify content
        let content = fs::read_to_string(&files[0]).unwrap();
        assert!(content.contains("EVT_000001"));
        assert!(content.contains("deposit"));
    }

    #[test]
    fn test_event_store_counter() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path()).unwrap();

        assert_eq!(store.next_event_id(), "EVT_000001");
        assert_eq!(store.next_event_id(), "EVT_000002");
        assert_eq!(store.next_event_id(), "EVT_000003");
    }

    #[test]
    fn test_event_store_reload_counter() {
        let dir = tempdir().unwrap();

        // First store
        {
            let store = EventStore::new(dir.path()).unwrap();
            let event_id = store.next_event_id();
            let event = Event::deposit(&event_id, "CUST_001", "ACC_001", dec!(100), "USDT");
            store.append(&event).unwrap();

            let event_id = store.next_event_id();
            let event = Event::deposit(&event_id, "CUST_001", "ACC_001", dec!(200), "USDT");
            store.append(&event).unwrap();
        }

        // Second store - should continue from 3
        {
            let store = EventStore::new(dir.path()).unwrap();
            assert_eq!(store.next_event_id(), "EVT_000003");
        }
    }
}
```

## File ./simbank\crates\persistence\src\sqlite\mod.rs:
```rust
//! SQLite persistence module
//!
//! Repository pattern cho SQLite database access.

pub mod repos;
pub mod schema;

pub use repos::{
    create_pool, init_database, run_migrations, AccountRepo, BalanceRepo, CurrencyRepo,
    PersonRepo, TransactionRepo, WalletRepo,
};
pub use schema::{AccountRow, BalanceRow, CurrencyRow, PersonRow, TransactionRow, WalletRow};

```

## File ./simbank\crates\persistence\src\sqlite\repos.rs:
```rust
//! Repository implementations cho SQLite
//!
//! CRUD operations cho tất cả các tables.

use crate::error::{PersistenceError, PersistenceResult};
use crate::sqlite::schema::*;
use rust_decimal::Decimal;
use simbank_core::{Account, Currency, Person, PersonType};
use simbank_core::wallet::{Wallet, WalletStatus, WalletType};
use sqlx::SqlitePool;
use std::str::FromStr;

// ============================================================================
// Currency Repository
// ============================================================================

/// Repository cho currencies table
pub struct CurrencyRepo;

impl CurrencyRepo {
    /// Lấy tất cả currencies
    pub async fn get_all(pool: &SqlitePool) -> PersistenceResult<Vec<CurrencyRow>> {
        let rows = sqlx::query_as::<_, CurrencyRow>("SELECT * FROM currencies")
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    /// Lấy currency theo code
    pub async fn get_by_code(pool: &SqlitePool, code: &str) -> PersistenceResult<CurrencyRow> {
        sqlx::query_as::<_, CurrencyRow>("SELECT * FROM currencies WHERE code = ?")
            .bind(code)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| PersistenceError::not_found("Currency", code))
    }

    /// Thêm currency mới
    pub async fn insert(pool: &SqlitePool, currency: &Currency) -> PersistenceResult<()> {
        sqlx::query(
            "INSERT INTO currencies (code, name, decimals, symbol) VALUES (?, ?, ?, ?)",
        )
        .bind(&currency.code)
        .bind(&currency.name)
        .bind(currency.decimals as i32)
        .bind(&currency.symbol)
        .execute(pool)
        .await?;
        Ok(())
    }
}

// ============================================================================
// Person Repository
// ============================================================================

/// Repository cho persons table
pub struct PersonRepo;

impl PersonRepo {
    /// Lấy person theo ID
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> PersistenceResult<PersonRow> {
        sqlx::query_as::<_, PersonRow>("SELECT * FROM persons WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| PersistenceError::not_found("Person", id))
    }

    /// Lấy tất cả persons theo type
    pub async fn get_by_type(
        pool: &SqlitePool,
        person_type: PersonType,
    ) -> PersistenceResult<Vec<PersonRow>> {
        let rows = sqlx::query_as::<_, PersonRow>(
            "SELECT * FROM persons WHERE person_type = ?",
        )
        .bind(person_type.as_str())
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Thêm person mới
    pub async fn insert(pool: &SqlitePool, person: &Person) -> PersistenceResult<()> {
        sqlx::query(
            "INSERT INTO persons (id, person_type, name, email, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&person.id)
        .bind(person.person_type.as_str())
        .bind(&person.name)
        .bind(&person.email)
        .bind(person.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Cập nhật person
    pub async fn update(pool: &SqlitePool, person: &Person) -> PersistenceResult<()> {
        let result = sqlx::query(
            "UPDATE persons SET name = ?, email = ? WHERE id = ?",
        )
        .bind(&person.name)
        .bind(&person.email)
        .bind(&person.id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(PersistenceError::not_found("Person", &person.id));
        }
        Ok(())
    }

    /// Xóa person
    pub async fn delete(pool: &SqlitePool, id: &str) -> PersistenceResult<()> {
        let result = sqlx::query("DELETE FROM persons WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(PersistenceError::not_found("Person", id));
        }
        Ok(())
    }
}

// ============================================================================
// Account Repository
// ============================================================================

/// Repository cho accounts table
pub struct AccountRepo;

impl AccountRepo {
    /// Lấy account theo ID
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> PersistenceResult<AccountRow> {
        sqlx::query_as::<_, AccountRow>("SELECT * FROM accounts WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| PersistenceError::not_found("Account", id))
    }

    /// Lấy account theo person_id
    pub async fn get_by_person_id(
        pool: &SqlitePool,
        person_id: &str,
    ) -> PersistenceResult<Option<AccountRow>> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT * FROM accounts WHERE person_id = ?",
        )
        .bind(person_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Thêm account mới
    pub async fn insert(pool: &SqlitePool, account: &Account) -> PersistenceResult<()> {
        sqlx::query(
            "INSERT INTO accounts (id, person_id, status, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&account.id)
        .bind(&account.person_id)
        .bind(account.status.as_str())
        .bind(account.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Cập nhật status
    pub async fn update_status(
        pool: &SqlitePool,
        id: &str,
        status: &str,
    ) -> PersistenceResult<()> {
        let result = sqlx::query("UPDATE accounts SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(PersistenceError::not_found("Account", id));
        }
        Ok(())
    }

    /// Lấy tất cả accounts
    pub async fn get_all(pool: &SqlitePool) -> PersistenceResult<Vec<AccountRow>> {
        let rows = sqlx::query_as::<_, AccountRow>("SELECT * FROM accounts")
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }
}

// ============================================================================
// Wallet Repository
// ============================================================================

/// Repository cho wallets table
pub struct WalletRepo;

impl WalletRepo {
    /// Lấy wallet theo ID
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> PersistenceResult<WalletRow> {
        sqlx::query_as::<_, WalletRow>("SELECT * FROM wallets WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| PersistenceError::not_found("Wallet", id))
    }

    /// Lấy tất cả wallets của account
    pub async fn get_by_account_id(
        pool: &SqlitePool,
        account_id: &str,
    ) -> PersistenceResult<Vec<WalletRow>> {
        let rows = sqlx::query_as::<_, WalletRow>(
            "SELECT * FROM wallets WHERE account_id = ?",
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Lấy wallet theo account và type
    pub async fn get_by_account_and_type(
        pool: &SqlitePool,
        account_id: &str,
        wallet_type: WalletType,
    ) -> PersistenceResult<Option<WalletRow>> {
        let row = sqlx::query_as::<_, WalletRow>(
            "SELECT * FROM wallets WHERE account_id = ? AND wallet_type = ?",
        )
        .bind(account_id)
        .bind(wallet_type.as_str())
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Thêm wallet mới
    pub async fn insert(pool: &SqlitePool, wallet: &Wallet) -> PersistenceResult<()> {
        sqlx::query(
            "INSERT INTO wallets (id, account_id, wallet_type, status, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&wallet.id)
        .bind(&wallet.account_id)
        .bind(wallet.wallet_type.as_str())
        .bind(wallet.status.as_str())
        .bind(wallet.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Cập nhật status
    pub async fn update_status(
        pool: &SqlitePool,
        id: &str,
        status: WalletStatus,
    ) -> PersistenceResult<()> {
        let result = sqlx::query("UPDATE wallets SET status = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(PersistenceError::not_found("Wallet", id));
        }
        Ok(())
    }
}

// ============================================================================
// Balance Repository
// ============================================================================

/// Repository cho balances table
pub struct BalanceRepo;

impl BalanceRepo {
    /// Lấy balance theo wallet và currency
    pub async fn get(
        pool: &SqlitePool,
        wallet_id: &str,
        currency_code: &str,
    ) -> PersistenceResult<Option<BalanceRow>> {
        let row = sqlx::query_as::<_, BalanceRow>(
            "SELECT * FROM balances WHERE wallet_id = ? AND currency_code = ?",
        )
        .bind(wallet_id)
        .bind(currency_code)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Lấy tất cả balances của wallet
    pub async fn get_by_wallet(
        pool: &SqlitePool,
        wallet_id: &str,
    ) -> PersistenceResult<Vec<BalanceRow>> {
        let rows = sqlx::query_as::<_, BalanceRow>(
            "SELECT * FROM balances WHERE wallet_id = ?",
        )
        .bind(wallet_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Upsert balance (insert hoặc update)
    pub async fn upsert(
        pool: &SqlitePool,
        wallet_id: &str,
        currency_code: &str,
        available: Decimal,
        locked: Decimal,
    ) -> PersistenceResult<()> {
        sqlx::query(
            r#"
            INSERT INTO balances (wallet_id, currency_code, available, locked, updated_at)
            VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(wallet_id, currency_code) DO UPDATE SET
                available = excluded.available,
                locked = excluded.locked,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(wallet_id)
        .bind(currency_code)
        .bind(available.to_string())
        .bind(locked.to_string())
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Credit (cộng tiền) vào available
    pub async fn credit(
        pool: &SqlitePool,
        wallet_id: &str,
        currency_code: &str,
        amount: Decimal,
    ) -> PersistenceResult<Decimal> {
        // Lấy balance hiện tại hoặc tạo mới
        let current = Self::get(pool, wallet_id, currency_code).await?;
        let current_available = current
            .map(|b| Decimal::from_str(&b.available).unwrap_or(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO);

        let new_available = current_available + amount;

        Self::upsert(pool, wallet_id, currency_code, new_available, Decimal::ZERO).await?;

        Ok(new_available)
    }

    /// Debit (trừ tiền) từ available
    pub async fn debit(
        pool: &SqlitePool,
        wallet_id: &str,
        currency_code: &str,
        amount: Decimal,
    ) -> PersistenceResult<Decimal> {
        let current = Self::get(pool, wallet_id, currency_code)
            .await?
            .ok_or_else(|| {
                PersistenceError::not_found("Balance", &format!("{}:{}", wallet_id, currency_code))
            })?;

        let current_available =
            Decimal::from_str(&current.available).map_err(|e| {
                PersistenceError::InvalidDecimal(e.to_string())
            })?;

        if current_available < amount {
            return Err(PersistenceError::Configuration(format!(
                "Insufficient balance: need {}, available {}",
                amount, current_available
            )));
        }

        let new_available = current_available - amount;
        let locked = Decimal::from_str(&current.locked).unwrap_or(Decimal::ZERO);

        Self::upsert(pool, wallet_id, currency_code, new_available, locked).await?;

        Ok(new_available)
    }
}

// ============================================================================
// Transaction Repository
// ============================================================================

/// Repository cho transactions table
pub struct TransactionRepo;

impl TransactionRepo {
    /// Thêm transaction mới
    pub async fn insert(pool: &SqlitePool, tx: &TransactionRow) -> PersistenceResult<()> {
        sqlx::query(
            r#"
            INSERT INTO transactions (id, account_id, wallet_id, tx_type, amount, currency_code, description, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&tx.id)
        .bind(&tx.account_id)
        .bind(&tx.wallet_id)
        .bind(&tx.tx_type)
        .bind(&tx.amount)
        .bind(&tx.currency_code)
        .bind(&tx.description)
        .bind(tx.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Lấy transactions theo account
    pub async fn get_by_account(
        pool: &SqlitePool,
        account_id: &str,
    ) -> PersistenceResult<Vec<TransactionRow>> {
        let rows = sqlx::query_as::<_, TransactionRow>(
            "SELECT * FROM transactions WHERE account_id = ? ORDER BY created_at DESC",
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Lấy transactions theo wallet
    pub async fn get_by_wallet(
        pool: &SqlitePool,
        wallet_id: &str,
    ) -> PersistenceResult<Vec<TransactionRow>> {
        let rows = sqlx::query_as::<_, TransactionRow>(
            "SELECT * FROM transactions WHERE wallet_id = ? ORDER BY created_at DESC",
        )
        .bind(wallet_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Lấy transaction theo ID
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> PersistenceResult<TransactionRow> {
        sqlx::query_as::<_, TransactionRow>("SELECT * FROM transactions WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| PersistenceError::not_found("Transaction", id))
    }

    /// Đếm transactions
    pub async fn count(pool: &SqlitePool) -> PersistenceResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}

// ============================================================================
// Database initialization
// ============================================================================

/// Khởi tạo database connection pool
pub async fn create_pool(database_url: &str) -> PersistenceResult<SqlitePool> {
    let pool = SqlitePool::connect(database_url).await?;
    Ok(pool)
}

/// Chạy migrations
pub async fn run_migrations(pool: &SqlitePool) -> PersistenceResult<()> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}

/// Tạo database mới với schema
pub async fn init_database(database_url: &str) -> PersistenceResult<SqlitePool> {
    // Tạo file nếu chưa có
    let pool = SqlitePool::connect_with(
        database_url.parse::<sqlx::sqlite::SqliteConnectOptions>()?
            .create_if_missing(true),
    )
    .await?;

    // Run migrations
    run_migrations(&pool).await?;

    Ok(pool)
}
```

## File ./simbank\crates\persistence\src\sqlite\schema.rs:
```rust
//! Database schema definitions
//!
//! Row types cho sqlx mapping từ SQLite tables.
//! Schema được định nghĩa trong migrations/20260125_init.sql

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Row type cho bảng `wallet_types`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct WalletTypeRow {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

/// Row type cho bảng `currencies`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct CurrencyRow {
    pub code: String,
    pub name: String,
    pub decimals: i32,
    pub symbol: Option<String>,
}

/// Row type cho bảng `persons`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PersonRow {
    pub id: String,
    pub person_type: String,
    pub name: String,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Row type cho bảng `accounts`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct AccountRow {
    pub id: String,
    pub person_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Row type cho bảng `wallets`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct WalletRow {
    pub id: String,
    pub account_id: String,
    pub wallet_type: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Row type cho bảng `balances`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct BalanceRow {
    pub wallet_id: String,
    pub currency_code: String,
    pub available: String, // Decimal stored as TEXT
    pub locked: String,    // Decimal stored as TEXT
    pub updated_at: DateTime<Utc>,
}

/// Row type cho bảng `transactions`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct TransactionRow {
    pub id: String,
    pub account_id: String,
    pub wallet_id: String,
    pub tx_type: String,
    pub amount: String, // Decimal stored as TEXT
    pub currency_code: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

// === Conversion implementations ===

impl From<&simbank_core::Currency> for CurrencyRow {
    fn from(currency: &simbank_core::Currency) -> Self {
        Self {
            code: currency.code.clone(),
            name: currency.name.clone(),
            decimals: currency.decimals as i32,
            symbol: Some(currency.symbol.clone()),
        }
    }
}

impl From<CurrencyRow> for simbank_core::Currency {
    fn from(row: CurrencyRow) -> Self {
        simbank_core::Currency::new(
            &row.code,
            &row.name,
            row.decimals as u8,
            row.symbol.as_deref().unwrap_or(""),
        )
    }
}

impl From<&simbank_core::Person> for PersonRow {
    fn from(person: &simbank_core::Person) -> Self {
        Self {
            id: person.id.clone(),
            person_type: person.person_type.as_str().to_string(),
            name: person.name.clone(),
            email: person.email.clone(),
            created_at: person.created_at,
        }
    }
}

impl From<&simbank_core::wallet::WalletType> for WalletTypeRow {
    fn from(wt: &simbank_core::wallet::WalletType) -> Self {
        let (name, desc) = match wt {
            simbank_core::wallet::WalletType::Spot => ("Spot Wallet", "For trading"),
            simbank_core::wallet::WalletType::Funding => ("Funding Wallet", "For deposit/withdraw"),
            simbank_core::wallet::WalletType::Margin => ("Margin Wallet", "For margin trading"),
            simbank_core::wallet::WalletType::Futures => ("Futures Wallet", "For futures contracts"),
            simbank_core::wallet::WalletType::Earn => ("Earn Wallet", "For staking/savings"),
        };
        Self {
            code: wt.as_str().to_string(),
            name: name.to_string(),
            description: Some(desc.to_string()),
        }
    }
}

```

## File ./simbank\crates\reports\src\aml_report.rs:
```rust
//! AML Report formatting for Big 4 compliance
//!
//! This module provides detailed AML (Anti-Money Laundering) report
//! generation suitable for regulatory compliance and audit purposes.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use simbank_core::{AmlFlag, Event};

use crate::exporters::ReportData;

// ============================================================================
// AML Report Data
// ============================================================================

/// AML Report with detailed analysis
#[derive(Debug, Clone)]
pub struct AmlReport {
    pub title: String,
    pub generated_at: DateTime<Utc>,
    pub total_events: usize,
    pub flagged_events: usize,
    pub large_amount_count: usize,
    pub near_threshold_count: usize,
    pub unusual_pattern_count: usize,
    pub high_risk_country_count: usize,
    pub events_by_flag: HashMap<AmlFlag, Vec<FlaggedEvent>>,
    pub risk_score: f64,
}

/// A flagged event for AML reporting
#[derive(Debug, Clone)]
pub struct FlaggedEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub account_id: String,
    pub amount: String,
    pub currency: String,
    pub flag: AmlFlag,
    pub risk_level: RiskLevel,
    pub description: String,
}

/// Risk level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "Low",
            RiskLevel::Medium => "Medium",
            RiskLevel::High => "High",
            RiskLevel::Critical => "Critical",
        }
    }

    pub fn from_flag(flag: &AmlFlag) -> Self {
        match flag {
            AmlFlag::LargeAmount => RiskLevel::High,
            AmlFlag::NearThreshold => RiskLevel::Medium,
            AmlFlag::UnusualPattern => RiskLevel::High,
            AmlFlag::HighRiskCountry => RiskLevel::Critical,
            AmlFlag::CrossBorder => RiskLevel::Medium,
            AmlFlag::NewAccountLargeTx => RiskLevel::High,
            AmlFlag::RapidWithdrawal => RiskLevel::High,
        }
    }
}

impl AmlReport {
    /// Create a new empty AML report
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            generated_at: Utc::now(),
            total_events: 0,
            flagged_events: 0,
            large_amount_count: 0,
            near_threshold_count: 0,
            unusual_pattern_count: 0,
            high_risk_country_count: 0,
            events_by_flag: HashMap::new(),
            risk_score: 0.0,
        }
    }

    /// Generate AML report from events
    pub fn generate(title: &str, events: &[Event]) -> Self {
        let mut report = Self::new(title);
        report.total_events = events.len();

        for event in events {
            for flag in &event.aml_flags {
                report.flagged_events += 1;

                match flag {
                    AmlFlag::LargeAmount => report.large_amount_count += 1,
                    AmlFlag::NearThreshold => report.near_threshold_count += 1,
                    AmlFlag::UnusualPattern => report.unusual_pattern_count += 1,
                    AmlFlag::HighRiskCountry => report.high_risk_country_count += 1,
                    // Other flags are counted in flagged_events but don't have dedicated counters
                    AmlFlag::CrossBorder | AmlFlag::NewAccountLargeTx | AmlFlag::RapidWithdrawal => {},
                }

                let flagged_event = FlaggedEvent {
                    event_id: event.event_id.clone(),
                    timestamp: event.timestamp,
                    event_type: event.event_type.as_str().to_string(),
                    account_id: event.account_id.clone(),
                    amount: event.amount.map(|a| a.to_string()).unwrap_or_default(),
                    currency: event.currency.clone().unwrap_or_default(),
                    flag: flag.clone(),
                    risk_level: RiskLevel::from_flag(flag),
                    description: event.description.clone().unwrap_or_default(),
                };

                report.events_by_flag
                    .entry(flag.clone())
                    .or_insert_with(Vec::new)
                    .push(flagged_event);
            }
        }

        report.calculate_risk_score();
        report
    }

    /// Calculate overall risk score (0-100)
    fn calculate_risk_score(&mut self) {
        if self.total_events == 0 {
            self.risk_score = 0.0;
            return;
        }

        // Weight factors for different risk types
        let weights = [
            (self.large_amount_count as f64, 3.0),      // High weight
            (self.near_threshold_count as f64, 2.0),    // Medium weight
            (self.unusual_pattern_count as f64, 3.5),   // High weight
            (self.high_risk_country_count as f64, 5.0), // Highest weight
        ];

        let weighted_sum: f64 = weights.iter().map(|(count, weight)| count * weight).sum();
        let max_possible = self.total_events as f64 * 5.0; // Max weight

        self.risk_score = (weighted_sum / max_possible * 100.0).min(100.0);
    }

    /// Get risk classification based on score
    pub fn risk_classification(&self) -> RiskLevel {
        match self.risk_score as u32 {
            0..=25 => RiskLevel::Low,
            26..=50 => RiskLevel::Medium,
            51..=75 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }

    /// Get summary text
    pub fn summary_text(&self) -> String {
        let mut summary = String::new();
        summary.push_str(&format!("=== {} ===\n\n", self.title));
        summary.push_str(&format!("Generated: {}\n", self.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        summary.push_str(&format!("Risk Score: {:.1}/100 ({})\n\n", self.risk_score, self.risk_classification().as_str()));

        summary.push_str("--- Statistics ---\n");
        summary.push_str(&format!("Total Events Analyzed: {}\n", self.total_events));
        summary.push_str(&format!("Flagged Events: {} ({:.1}%)\n",
            self.flagged_events,
            if self.total_events > 0 { self.flagged_events as f64 / self.total_events as f64 * 100.0 } else { 0.0 }
        ));
        summary.push_str(&format!("  - Large Amount (>$10,000): {}\n", self.large_amount_count));
        summary.push_str(&format!("  - Near Threshold ($9,000-$9,999): {}\n", self.near_threshold_count));
        summary.push_str(&format!("  - Unusual Pattern: {}\n", self.unusual_pattern_count));
        summary.push_str(&format!("  - High Risk Country: {}\n", self.high_risk_country_count));

        summary
    }

    /// Get flagged events sorted by risk level (highest first)
    pub fn flagged_events_sorted(&self) -> Vec<&FlaggedEvent> {
        let mut events: Vec<&FlaggedEvent> = self.events_by_flag
            .values()
            .flatten()
            .collect();

        events.sort_by(|a, b| {
            let a_score = match a.risk_level {
                RiskLevel::Critical => 4,
                RiskLevel::High => 3,
                RiskLevel::Medium => 2,
                RiskLevel::Low => 1,
            };
            let b_score = match b.risk_level {
                RiskLevel::Critical => 4,
                RiskLevel::High => 3,
                RiskLevel::Medium => 2,
                RiskLevel::Low => 1,
            };
            b_score.cmp(&a_score)
        });

        events
    }
}

impl ReportData for AmlReport {
    fn title(&self) -> &str {
        &self.title
    }

    fn headers(&self) -> Vec<String> {
        vec![
            "Event ID".to_string(),
            "Timestamp".to_string(),
            "Type".to_string(),
            "Account".to_string(),
            "Amount".to_string(),
            "Currency".to_string(),
            "Flag".to_string(),
            "Risk Level".to_string(),
            "Description".to_string(),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.flagged_events_sorted()
            .iter()
            .map(|e| {
                vec![
                    e.event_id.clone(),
                    e.timestamp.to_rfc3339(),
                    e.event_type.clone(),
                    e.account_id.clone(),
                    e.amount.clone(),
                    e.currency.clone(),
                    e.flag.as_str().to_string(),
                    e.risk_level.as_str().to_string(),
                    e.description.clone(),
                ]
            })
            .collect()
    }

    fn summary(&self) -> Vec<(String, String)> {
        vec![
            ("Total Events".to_string(), self.total_events.to_string()),
            ("Flagged Events".to_string(), self.flagged_events.to_string()),
            ("Large Amount".to_string(), self.large_amount_count.to_string()),
            ("Near Threshold".to_string(), self.near_threshold_count.to_string()),
            ("Unusual Pattern".to_string(), self.unusual_pattern_count.to_string()),
            ("High Risk Country".to_string(), self.high_risk_country_count.to_string()),
            ("Risk Score".to_string(), format!("{:.1}/100", self.risk_score)),
            ("Risk Level".to_string(), self.risk_classification().as_str().to_string()),
            ("Generated At".to_string(), self.generated_at.to_rfc3339()),
        ]
    }
}

// ============================================================================
// Velocity Report (for detecting structuring)
// ============================================================================

/// Velocity analysis for detecting rapid transactions
#[derive(Debug, Clone)]
pub struct VelocityReport {
    pub title: String,
    pub generated_at: DateTime<Utc>,
    pub analysis_window_hours: u32,
    pub accounts: Vec<VelocityAnalysis>,
}

#[derive(Debug, Clone)]
pub struct VelocityAnalysis {
    pub account_id: String,
    pub transaction_count: usize,
    pub total_amount: String,
    pub time_span_minutes: i64,
    pub avg_transaction_interval: String,
    pub risk_level: RiskLevel,
    pub transactions: Vec<String>, // Event IDs
}

impl VelocityReport {
    pub fn new(title: &str, window_hours: u32) -> Self {
        Self {
            title: title.to_string(),
            generated_at: Utc::now(),
            analysis_window_hours: window_hours,
            accounts: Vec::new(),
        }
    }

    pub fn generate(title: &str, events: &[Event], window_hours: u32) -> Self {
        let mut report = Self::new(title, window_hours);

        // Group events by account
        let mut by_account: HashMap<String, Vec<&Event>> = HashMap::new();
        for event in events {
            if event.amount.is_some() {
                by_account
                    .entry(event.account_id.clone())
                    .or_insert_with(Vec::new)
                    .push(event);
            }
        }

        // Analyze each account
        for (account_id, account_events) in by_account {
            if account_events.len() < 2 {
                continue;
            }

            // Sort by timestamp
            let mut sorted_events = account_events.clone();
            sorted_events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

            // Calculate metrics
            let total: rust_decimal::Decimal = sorted_events
                .iter()
                .filter_map(|e| e.amount)
                .sum();

            let first_ts = sorted_events.first().map(|e| e.timestamp).unwrap();
            let last_ts = sorted_events.last().map(|e| e.timestamp).unwrap();
            let time_span = (last_ts - first_ts).num_minutes();

            let avg_interval = if sorted_events.len() > 1 {
                time_span as f64 / (sorted_events.len() - 1) as f64
            } else {
                0.0
            };

            // Determine risk level based on velocity
            let risk_level = if avg_interval < 5.0 && sorted_events.len() > 5 {
                RiskLevel::Critical // Very rapid transactions
            } else if avg_interval < 30.0 && sorted_events.len() > 3 {
                RiskLevel::High
            } else if avg_interval < 60.0 && sorted_events.len() > 2 {
                RiskLevel::Medium
            } else {
                RiskLevel::Low
            };

            let analysis = VelocityAnalysis {
                account_id,
                transaction_count: sorted_events.len(),
                total_amount: total.to_string(),
                time_span_minutes: time_span,
                avg_transaction_interval: format!("{:.1} min", avg_interval),
                risk_level,
                transactions: sorted_events.iter().map(|e| e.event_id.clone()).collect(),
            };

            report.accounts.push(analysis);
        }

        // Sort by risk level
        report.accounts.sort_by(|a, b| {
            let a_score = match a.risk_level {
                RiskLevel::Critical => 4,
                RiskLevel::High => 3,
                RiskLevel::Medium => 2,
                RiskLevel::Low => 1,
            };
            let b_score = match b.risk_level {
                RiskLevel::Critical => 4,
                RiskLevel::High => 3,
                RiskLevel::Medium => 2,
                RiskLevel::Low => 1,
            };
            b_score.cmp(&a_score)
        });

        report
    }
}

impl ReportData for VelocityReport {
    fn title(&self) -> &str {
        &self.title
    }

    fn headers(&self) -> Vec<String> {
        vec![
            "Account".to_string(),
            "Tx Count".to_string(),
            "Total Amount".to_string(),
            "Time Span".to_string(),
            "Avg Interval".to_string(),
            "Risk Level".to_string(),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.accounts
            .iter()
            .map(|a| {
                vec![
                    a.account_id.clone(),
                    a.transaction_count.to_string(),
                    a.total_amount.clone(),
                    format!("{} min", a.time_span_minutes),
                    a.avg_transaction_interval.clone(),
                    a.risk_level.as_str().to_string(),
                ]
            })
            .collect()
    }

    fn summary(&self) -> Vec<(String, String)> {
        let high_risk_count = self.accounts.iter()
            .filter(|a| matches!(a.risk_level, RiskLevel::High | RiskLevel::Critical))
            .count();

        vec![
            ("Analysis Window".to_string(), format!("{} hours", self.analysis_window_hours)),
            ("Accounts Analyzed".to_string(), self.accounts.len().to_string()),
            ("High Risk Accounts".to_string(), high_risk_count.to_string()),
            ("Generated At".to_string(), self.generated_at.to_rfc3339()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_aml_report_empty() {
        let report = AmlReport::new("Empty Report");
        assert_eq!(report.total_events, 0);
        assert_eq!(report.risk_score, 0.0);
        assert_eq!(report.risk_classification(), RiskLevel::Low);
    }

    #[test]
    fn test_aml_report_generate() {
        let mut event1 = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");
        event1.aml_flags.push(AmlFlag::LargeAmount);

        let mut event2 = Event::deposit("EVT_002", "CUST_001", "ACC_001", dec!(9500), "USD");
        event2.aml_flags.push(AmlFlag::NearThreshold);

        let events = vec![event1, event2];
        let report = AmlReport::generate("Test AML Report", &events);

        assert_eq!(report.total_events, 2);
        assert_eq!(report.flagged_events, 2);
        assert_eq!(report.large_amount_count, 1);
        assert_eq!(report.near_threshold_count, 1);
        assert!(report.risk_score > 0.0);
    }

    #[test]
    fn test_risk_level_from_flag() {
        assert_eq!(RiskLevel::from_flag(&AmlFlag::LargeAmount), RiskLevel::High);
        assert_eq!(RiskLevel::from_flag(&AmlFlag::NearThreshold), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_flag(&AmlFlag::UnusualPattern), RiskLevel::High);
        assert_eq!(RiskLevel::from_flag(&AmlFlag::HighRiskCountry), RiskLevel::Critical);
    }

    #[test]
    fn test_flagged_events_sorted() {
        let mut event1 = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");
        event1.aml_flags.push(AmlFlag::NearThreshold); // Medium risk

        let mut event2 = Event::deposit("EVT_002", "CUST_001", "ACC_001", dec!(50000), "USD");
        event2.aml_flags.push(AmlFlag::HighRiskCountry); // Critical risk

        let events = vec![event1, event2];
        let report = AmlReport::generate("Test", &events);

        let sorted = report.flagged_events_sorted();
        assert!(!sorted.is_empty());
        // Critical should come first
        assert_eq!(sorted[0].risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_aml_report_summary() {
        let report = AmlReport::new("Summary Test");
        let summary = report.summary();

        assert!(summary.iter().any(|(k, _)| k == "Total Events"));
        assert!(summary.iter().any(|(k, _)| k == "Risk Score"));
    }

    #[test]
    fn test_aml_report_as_report_data() {
        let mut event = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");
        event.aml_flags.push(AmlFlag::LargeAmount);

        let events = vec![event];
        let report = AmlReport::generate("Test", &events);

        // Test ReportData trait implementation
        assert_eq!(report.title(), "Test");
        assert!(!report.headers().is_empty());
        assert!(!report.rows().is_empty());
        assert!(!report.summary().is_empty());
    }

    #[test]
    fn test_velocity_report_empty() {
        let report = VelocityReport::new("Empty Velocity", 24);
        assert_eq!(report.accounts.len(), 0);
        assert_eq!(report.analysis_window_hours, 24);
    }

    #[test]
    fn test_velocity_report_generate() {
        use chrono::Duration;

        let base_time = Utc::now();

        let mut events = vec![];
        for i in 0..5 {
            let mut event = Event::deposit(
                &format!("EVT_{:03}", i),
                "CUST_001",
                "ACC_001",
                dec!(100),
                "USD",
            );
            // Set timestamps 2 minutes apart
            event.timestamp = base_time + Duration::minutes(i * 2);
            events.push(event);
        }

        let report = VelocityReport::generate("Velocity Test", &events, 24);

        assert_eq!(report.accounts.len(), 1);
        assert_eq!(report.accounts[0].transaction_count, 5);
    }

    #[test]
    fn test_summary_text_format() {
        let mut event = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");
        event.aml_flags.push(AmlFlag::LargeAmount);

        let events = vec![event];
        let report = AmlReport::generate("Test Report", &events);

        let summary = report.summary_text();
        assert!(summary.contains("Test Report"));
        assert!(summary.contains("Risk Score"));
        assert!(summary.contains("Large Amount"));
    }
}

```

## File ./simbank\crates\reports\src\exporters.rs:
```rust
//! Report exporters - CSV, JSON, Markdown//!
//! This module provides different export formats for reports.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use simbank_core::Event;

/// Trait for exporting reports to different formats
pub trait ReportExporter {
    /// Export to the target format
    fn export(&self, report: &dyn ReportData) -> String;

    /// Get the file extension for this format
    fn extension(&self) -> &'static str;

    /// Get the MIME type for this format
    fn mime_type(&self) -> &'static str;
}

/// Trait for data that can be exported
pub trait ReportData {
    /// Get the report title
    fn title(&self) -> &str;

    /// Get column headers
    fn headers(&self) -> Vec<String>;

    /// Get data rows
    fn rows(&self) -> Vec<Vec<String>>;

    /// Get summary statistics as key-value pairs
    fn summary(&self) -> Vec<(String, String)>;
}

// ============================================================================
// CSV Exporter
// ============================================================================

/// CSV format exporter
pub struct CsvExporter {
    delimiter: char,
    include_header: bool,
}

impl Default for CsvExporter {
    fn default() -> Self {
        Self {
            delimiter: ',',
            include_header: true,
        }
    }
}

impl CsvExporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_delimiter(mut self, delimiter: char) -> Self {
        self.delimiter = delimiter;
        self
    }

    pub fn without_header(mut self) -> Self {
        self.include_header = false;
        self
    }

    fn escape_csv_field(&self, field: &str) -> String {
        if field.contains(self.delimiter) || field.contains('"') || field.contains('\n') {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }
}

impl ReportExporter for CsvExporter {
    fn export(&self, report: &dyn ReportData) -> String {
        let mut output = String::new();

        // Header
        if self.include_header {
            let headers: Vec<String> = report
                .headers()
                .iter()
                .map(|h| self.escape_csv_field(h))
                .collect();
            output.push_str(&headers.join(&self.delimiter.to_string()));
            output.push('\n');
        }

        // Data rows
        for row in report.rows() {
            let escaped: Vec<String> = row
                .iter()
                .map(|field| self.escape_csv_field(field))
                .collect();
            output.push_str(&escaped.join(&self.delimiter.to_string()));
            output.push('\n');
        }

        output
    }

    fn extension(&self) -> &'static str {
        "csv"
    }

    fn mime_type(&self) -> &'static str {
        "text/csv"
    }
}

// ============================================================================
// JSON Exporter
// ============================================================================

/// JSON format exporter
pub struct JsonExporter {
    pretty: bool,
}

impl Default for JsonExporter {
    fn default() -> Self {
        Self { pretty: true }
    }
}

impl JsonExporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compact(mut self) -> Self {
        self.pretty = false;
        self
    }
}

impl ReportExporter for JsonExporter {
    fn export(&self, report: &dyn ReportData) -> String {
        let headers = report.headers();
        let rows = report.rows();
        let summary = report.summary();

        // Build JSON structure
        let json_rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                for (i, header) in headers.iter().enumerate() {
                    let value = row.get(i).cloned().unwrap_or_default();
                    obj.insert(header.clone(), serde_json::Value::String(value));
                }
                serde_json::Value::Object(obj)
            })
            .collect();

        let summary_obj: serde_json::Map<String, serde_json::Value> = summary
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();

        let output = serde_json::json!({
            "title": report.title(),
            "summary": summary_obj,
            "data": json_rows,
        });

        if self.pretty {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            serde_json::to_string(&output).unwrap_or_default()
        }
    }

    fn extension(&self) -> &'static str {
        "json"
    }

    fn mime_type(&self) -> &'static str {
        "application/json"
    }
}

// ============================================================================
// Markdown Exporter
// ============================================================================

/// Markdown format exporter
pub struct MarkdownExporter {
    include_summary: bool,
    include_toc: bool,
}

impl Default for MarkdownExporter {
    fn default() -> Self {
        Self {
            include_summary: true,
            include_toc: false,
        }
    }
}

impl MarkdownExporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn without_summary(mut self) -> Self {
        self.include_summary = false;
        self
    }

    pub fn with_toc(mut self) -> Self {
        self.include_toc = true;
        self
    }
}

impl ReportExporter for MarkdownExporter {
    fn export(&self, report: &dyn ReportData) -> String {
        let mut output = String::new();

        // Title
        output.push_str(&format!("# {}\n\n", report.title()));

        // Table of Contents
        if self.include_toc {
            output.push_str("## Table of Contents\n\n");
            if self.include_summary {
                output.push_str("- [Summary](#summary)\n");
            }
            output.push_str("- [Data](#data)\n\n");
        }

        // Summary section
        if self.include_summary {
            output.push_str("## Summary\n\n");
            for (key, value) in report.summary() {
                output.push_str(&format!("- **{}**: {}\n", key, value));
            }
            output.push('\n');
        }

        // Data table
        output.push_str("## Data\n\n");

        let headers = report.headers();
        if !headers.is_empty() {
            // Header row
            output.push_str("| ");
            output.push_str(&headers.join(" | "));
            output.push_str(" |\n");

            // Separator row
            output.push_str("| ");
            output.push_str(&headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
            output.push_str(" |\n");

            // Data rows
            for row in report.rows() {
                output.push_str("| ");
                output.push_str(&row.join(" | "));
                output.push_str(" |\n");
            }
        }

        output
    }

    fn extension(&self) -> &'static str {
        "md"
    }

    fn mime_type(&self) -> &'static str {
        "text/markdown"
    }
}

// ============================================================================
// Transaction Report Data
// ============================================================================

/// Transaction report data
#[derive(Debug, Clone)]
pub struct TransactionReport {
    pub title: String,
    pub transactions: Vec<TransactionRow>,
    pub total_amount: Decimal,
    pub currency: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TransactionRow {
    pub id: String,
    pub timestamp: String,
    pub tx_type: String,
    pub amount: String,
    pub currency: String,
    pub account_id: String,
    pub wallet_type: String,
    pub description: String,
}

impl TransactionReport {
    pub fn new(title: &str, currency: &str) -> Self {
        Self {
            title: title.to_string(),
            transactions: Vec::new(),
            total_amount: Decimal::ZERO,
            currency: currency.to_string(),
            generated_at: Utc::now(),
        }
    }

    pub fn add_transaction(&mut self, row: TransactionRow) {
        if let Ok(amount) = row.amount.parse::<Decimal>() {
            self.total_amount += amount;
        }
        self.transactions.push(row);
    }

    pub fn from_events(title: &str, events: &[Event]) -> Self {
        let mut report = Self::new(title, "");

        for event in events {
            if let Some(amount) = event.amount {
                let row = TransactionRow {
                    id: event.event_id.clone(),
                    timestamp: event.timestamp.to_rfc3339(),
                    tx_type: event.event_type.as_str().to_string(),
                    amount: amount.to_string(),
                    currency: event.currency.clone().unwrap_or_default(),
                    account_id: event.account_id.clone(),
                    wallet_type: event.to_wallet
                        .as_ref()
                        .or(event.from_wallet.as_ref())
                        .map(|w| w.as_str().to_string())
                        .unwrap_or_default(),
                    description: event.description.clone().unwrap_or_default(),
                };
                report.add_transaction(row);
            }
        }

        report
    }
}

impl ReportData for TransactionReport {
    fn title(&self) -> &str {
        &self.title
    }

    fn headers(&self) -> Vec<String> {
        vec![
            "ID".to_string(),
            "Timestamp".to_string(),
            "Type".to_string(),
            "Amount".to_string(),
            "Currency".to_string(),
            "Account".to_string(),
            "Wallet".to_string(),
            "Description".to_string(),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.transactions
            .iter()
            .map(|t| {
                vec![
                    t.id.clone(),
                    t.timestamp.clone(),
                    t.tx_type.clone(),
                    t.amount.clone(),
                    t.currency.clone(),
                    t.account_id.clone(),
                    t.wallet_type.clone(),
                    t.description.clone(),
                ]
            })
            .collect()
    }

    fn summary(&self) -> Vec<(String, String)> {
        vec![
            ("Total Transactions".to_string(), self.transactions.len().to_string()),
            ("Total Amount".to_string(), format!("{} {}", self.total_amount, self.currency)),
            ("Generated At".to_string(), self.generated_at.to_rfc3339()),
        ]
    }
}

// ============================================================================
// Account Summary Report
// ============================================================================

/// Account summary for reporting
#[derive(Debug, Clone)]
pub struct AccountSummaryReport {
    pub title: String,
    pub accounts: Vec<AccountSummaryRow>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AccountSummaryRow {
    pub account_id: String,
    pub person_name: String,
    pub person_type: String,
    pub status: String,
    pub wallet_count: usize,
    pub total_balance: String,
}

impl AccountSummaryReport {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            accounts: Vec::new(),
            generated_at: Utc::now(),
        }
    }

    pub fn add_account(&mut self, row: AccountSummaryRow) {
        self.accounts.push(row);
    }
}

impl ReportData for AccountSummaryReport {
    fn title(&self) -> &str {
        &self.title
    }

    fn headers(&self) -> Vec<String> {
        vec![
            "Account ID".to_string(),
            "Name".to_string(),
            "Type".to_string(),
            "Status".to_string(),
            "Wallets".to_string(),
            "Total Balance".to_string(),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.accounts
            .iter()
            .map(|a| {
                vec![
                    a.account_id.clone(),
                    a.person_name.clone(),
                    a.person_type.clone(),
                    a.status.clone(),
                    a.wallet_count.to_string(),
                    a.total_balance.clone(),
                ]
            })
            .collect()
    }

    fn summary(&self) -> Vec<(String, String)> {
        let active = self.accounts.iter().filter(|a| a.status == "active").count();
        vec![
            ("Total Accounts".to_string(), self.accounts.len().to_string()),
            ("Active Accounts".to_string(), active.to_string()),
            ("Generated At".to_string(), self.generated_at.to_rfc3339()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_transaction_report() -> TransactionReport {
        let mut report = TransactionReport::new("Test Transactions", "USD");
        report.add_transaction(TransactionRow {
            id: "TXN_001".to_string(),
            timestamp: "2026-01-25T10:00:00Z".to_string(),
            tx_type: "deposit".to_string(),
            amount: "100.00".to_string(),
            currency: "USD".to_string(),
            account_id: "ACC_001".to_string(),
            wallet_type: "funding".to_string(),
            description: "Test deposit".to_string(),
        });
        report.add_transaction(TransactionRow {
            id: "TXN_002".to_string(),
            timestamp: "2026-01-25T11:00:00Z".to_string(),
            tx_type: "withdrawal".to_string(),
            amount: "50.00".to_string(),
            currency: "USD".to_string(),
            account_id: "ACC_001".to_string(),
            wallet_type: "funding".to_string(),
            description: "Test withdrawal".to_string(),
        });
        report
    }

    #[test]
    fn test_csv_exporter() {
        let report = sample_transaction_report();
        let exporter = CsvExporter::new();
        let output = exporter.export(&report);

        assert!(output.contains("ID,Timestamp,Type,Amount"));
        assert!(output.contains("TXN_001"));
        assert!(output.contains("TXN_002"));
        assert!(output.contains("deposit"));
        assert_eq!(exporter.extension(), "csv");
    }

    #[test]
    fn test_csv_with_special_chars() {
        let mut report = TransactionReport::new("Test", "USD");
        report.add_transaction(TransactionRow {
            id: "TXN_001".to_string(),
            timestamp: "2026-01-25T10:00:00Z".to_string(),
            tx_type: "deposit".to_string(),
            amount: "100.00".to_string(),
            currency: "USD".to_string(),
            account_id: "ACC_001".to_string(),
            wallet_type: "funding".to_string(),
            description: "Test, with \"quotes\" and comma".to_string(),
        });

        let exporter = CsvExporter::new();
        let output = exporter.export(&report);

        // Should escape the description
        assert!(output.contains("\"Test, with \"\"quotes\"\" and comma\""));
    }

    #[test]
    fn test_json_exporter() {
        let report = sample_transaction_report();
        let exporter = JsonExporter::new();
        let output = exporter.export(&report);

        assert!(output.contains("\"title\": \"Test Transactions\""));
        assert!(output.contains("\"TXN_001\""));
        assert!(output.contains("\"deposit\""));
        assert_eq!(exporter.extension(), "json");
    }

    #[test]
    fn test_json_compact() {
        let report = sample_transaction_report();
        let exporter = JsonExporter::new().compact();
        let output = exporter.export(&report);

        // Compact JSON should not have newlines in the main structure
        assert!(!output.contains("  ")); // No indentation
    }

    #[test]
    fn test_markdown_exporter() {
        let report = sample_transaction_report();
        let exporter = MarkdownExporter::new();
        let output = exporter.export(&report);

        assert!(output.contains("# Test Transactions"));
        assert!(output.contains("## Summary"));
        assert!(output.contains("## Data"));
        assert!(output.contains("| ID | Timestamp | Type |"));
        assert!(output.contains("| --- | --- | --- |"));
        assert!(output.contains("| TXN_001 |"));
        assert_eq!(exporter.extension(), "md");
    }

    #[test]
    fn test_markdown_with_toc() {
        let report = sample_transaction_report();
        let exporter = MarkdownExporter::new().with_toc();
        let output = exporter.export(&report);

        assert!(output.contains("## Table of Contents"));
        assert!(output.contains("- [Summary](#summary)"));
        assert!(output.contains("- [Data](#data)"));
    }

    #[test]
    fn test_transaction_report_from_events() {
        let events = vec![
            Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(100), "USD"),
            Event::withdrawal("EVT_002", "CUST_001", "ACC_001", dec!(50), "USD"),
        ];

        let report = TransactionReport::from_events("Event Report", &events);

        assert_eq!(report.transactions.len(), 2);
        assert_eq!(report.total_amount, dec!(150));
    }

    #[test]
    fn test_account_summary_report() {
        let mut report = AccountSummaryReport::new("Account Summary");
        report.add_account(AccountSummaryRow {
            account_id: "ACC_001".to_string(),
            person_name: "Alice".to_string(),
            person_type: "customer".to_string(),
            status: "active".to_string(),
            wallet_count: 2,
            total_balance: "1000.00 USD".to_string(),
        });
        report.add_account(AccountSummaryRow {
            account_id: "ACC_002".to_string(),
            person_name: "Bob".to_string(),
            person_type: "employee".to_string(),
            status: "active".to_string(),
            wallet_count: 1,
            total_balance: "5000.00 USD".to_string(),
        });

        let exporter = MarkdownExporter::new();
        let output = exporter.export(&report);

        assert!(output.contains("ACC_001"));
        assert!(output.contains("Alice"));
        assert!(output.contains("Total Accounts"));
    }
}
```

## File ./simbank\crates\reports\src\lib.rs:
```rust
//! # Simbank Reports
//!
//! Report generation - CSV, JSON, Markdown, AML reports.
//!
//! This crate provides export functionality for different report formats
//! and AML compliance reporting suitable for regulatory audits.
//!
//! ## Exporters
//!
//! - [`CsvExporter`] - CSV format with proper escaping
//! - [`JsonExporter`] - JSON format (pretty or compact)
//! - [`MarkdownExporter`] - Markdown tables for documentation
//!
//! ## Reports
//!
//! - [`TransactionReport`] - Transaction history reports
//! - [`AccountSummaryReport`] - Account overview reports
//! - [`AmlReport`] - AML compliance reports with risk scoring
//! - [`VelocityReport`] - Transaction velocity analysis
//!
//! ## Example
//!
//! ```rust,ignore
//! use simbank_reports::{CsvExporter, MarkdownExporter, ReportExporter, TransactionReport};
//!
//! let report = TransactionReport::new("Monthly Report", "USD");
//! let csv_exporter = CsvExporter::new();
//! let csv_output = csv_exporter.export(&report);
//!
//! let md_exporter = MarkdownExporter::new().with_toc();
//! let md_output = md_exporter.export(&report);
//! ```

pub mod exporters;
pub mod aml_report;

// Re-export main types
pub use exporters::{
    ReportExporter,
    ReportData,
    CsvExporter,
    JsonExporter,
    MarkdownExporter,
    TransactionReport,
    TransactionRow,
    AccountSummaryReport,
    AccountSummaryRow,
};

pub use aml_report::{
    AmlReport,
    FlaggedEvent,
    RiskLevel,
    VelocityReport,
    VelocityAnalysis,
};

```

# Thông tin bổ sung:

## Cargo.toml dependencies:
```toml
members = [
resolver = "2"
version = "0.1.0"
edition = "2021"
authors = ["Simbank Team"]
tokio = { version = "1.36", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rust_decimal = { version = "1.33", features = ["serde-with-str"] }
rust_decimal_macros = "1.33"
thiserror = "2.0"
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.7", features = ["serde", "v4"] }
tracing = "0.1"
tracing-subscriber = "0.3"
tempfile = "3.10"
simbank-core = { path = "./crates/core" }
simbank-persistence = { path = "./crates/persistence" }
simbank-business = { path = "./crates/business" }
simbank-reports = { path = "./crates/reports" }
simbank-dsl = { path = "./crates/dsl" }
```

