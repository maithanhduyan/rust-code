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
