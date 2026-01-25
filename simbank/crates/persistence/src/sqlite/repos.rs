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