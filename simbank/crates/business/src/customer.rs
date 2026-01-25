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
