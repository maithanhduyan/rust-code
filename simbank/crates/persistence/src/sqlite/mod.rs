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
