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
