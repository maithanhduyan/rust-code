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
