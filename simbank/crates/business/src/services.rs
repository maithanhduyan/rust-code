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
