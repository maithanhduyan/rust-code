//! Projection engine - coordinates replay and updates

use crate::balance::BalanceProjection;
use crate::error::ProjectionError;
use crate::trade::TradeProjection;
use bibank_bus::EventBus;
use bibank_ledger::JournalEntry;
use sqlx::SqlitePool;
use std::path::Path;

/// Projection engine - coordinates replay and updates
pub struct ProjectionEngine {
    pub balance: BalanceProjection,
    pub trade: TradeProjection,
}

impl ProjectionEngine {
    /// Create a new projection engine
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self, ProjectionError> {
        let db_url = format!("sqlite:{}?mode=rwc", db_path.as_ref().display());
        let pool = SqlitePool::connect(&db_url).await?;

        let balance = BalanceProjection::new(pool.clone());
        balance.init().await?;

        let trade = TradeProjection::new(pool);
        trade.init().await?;

        Ok(Self { balance, trade })
    }

    /// Apply a single entry
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), ProjectionError> {
        self.balance.apply(entry).await?;
        self.trade.apply(entry).await?;
        Ok(())
    }

    /// Replay all events from the bus
    pub async fn replay(&self, bus: &EventBus) -> Result<usize, ProjectionError> {
        let reader = bus.reader()?;
        let entries = reader.read_all()?;

        self.balance.clear().await?;
        self.trade.clear().await?;

        let count = entries.len();
        for entry in &entries {
            self.balance.apply(entry).await?;
            self.trade.apply(entry).await?;
        }

        Ok(count)
    }

    /// Get the balance projection
    pub fn balance(&self) -> &BalanceProjection {
        &self.balance
    }

    /// Get the trade projection
    pub fn trade(&self) -> &TradeProjection {
        &self.trade
    }
}
