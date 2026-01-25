//! Database initialization and status

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

/// Initialize the database with schema
pub async fn init_database(db_path: &Path, force: bool) -> Result<()> {
    if force && db_path.exists() {
        std::fs::remove_file(db_path).context("Failed to remove existing database")?;
        println!("🗑️  Removed existing database");
    }

    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .context("Failed to connect to database")?;

    // Run schema creation
    create_schema(&pool).await?;
    seed_data(&pool).await?;

    pool.close().await;
    Ok(())
}

/// Show database status
pub async fn show_status(db_path: &Path) -> Result<()> {
    if !db_path.exists() {
        println!("❌ Database not found at {:?}", db_path);
        println!("   Run 'simbank init' to create the database");
        return Ok(());
    }

    let db_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&db_url).await?;

    println!("📊 Database Status");
    println!("   Path: {:?}", db_path);
    println!();

    // Count records
    let person_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM persons")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    let account_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    let wallet_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM wallets")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    let tx_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    println!("   Persons:      {}", person_count.0);
    println!("   Accounts:     {}", account_count.0);
    println!("   Wallets:      {}", wallet_count.0);
    println!("   Transactions: {}", tx_count.0);

    pool.close().await;
    Ok(())
}

/// Create database schema
async fn create_schema(pool: &SqlitePool) -> Result<()> {
    println!("📦 Creating schema...");

    sqlx::query(
        r#"
        -- Wallet types enum table
        CREATE TABLE IF NOT EXISTS wallet_types (
            code TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT
        );

        -- Currencies with dynamic decimals
        CREATE TABLE IF NOT EXISTS currencies (
            code TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            decimals INTEGER NOT NULL,
            symbol TEXT
        );

        -- Person types
        CREATE TABLE IF NOT EXISTS persons (
            id TEXT PRIMARY KEY,
            person_type TEXT NOT NULL,
            name TEXT NOT NULL,
            email TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Accounts (1:1 with Person)
        CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            person_id TEXT NOT NULL UNIQUE,
            status TEXT DEFAULT 'active',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (person_id) REFERENCES persons(id)
        );

        -- Wallets
        CREATE TABLE IF NOT EXISTS wallets (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            wallet_type TEXT NOT NULL,
            status TEXT DEFAULT 'active',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(account_id, wallet_type),
            FOREIGN KEY (account_id) REFERENCES accounts(id),
            FOREIGN KEY (wallet_type) REFERENCES wallet_types(code)
        );

        -- Balances
        CREATE TABLE IF NOT EXISTS balances (
            wallet_id TEXT NOT NULL,
            currency_code TEXT NOT NULL,
            available TEXT NOT NULL DEFAULT '0',
            locked TEXT NOT NULL DEFAULT '0',
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (wallet_id, currency_code),
            FOREIGN KEY (wallet_id) REFERENCES wallets(id),
            FOREIGN KEY (currency_code) REFERENCES currencies(code)
        );

        -- Transactions
        CREATE TABLE IF NOT EXISTS transactions (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            wallet_id TEXT NOT NULL,
            tx_type TEXT NOT NULL,
            amount TEXT NOT NULL,
            currency_code TEXT NOT NULL,
            description TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (account_id) REFERENCES accounts(id),
            FOREIGN KEY (wallet_id) REFERENCES wallets(id)
        );
        "#,
    )
    .execute(pool)
    .await
    .context("Failed to create schema")?;

    Ok(())
}

/// Seed reference data
async fn seed_data(pool: &SqlitePool) -> Result<()> {
    println!("🌱 Seeding reference data...");

    // Wallet types
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO wallet_types VALUES
            ('spot', 'Spot Wallet', 'For trading'),
            ('funding', 'Funding Wallet', 'For deposit/withdraw'),
            ('margin', 'Margin Wallet', 'For margin trading'),
            ('futures', 'Futures Wallet', 'For futures contracts'),
            ('earn', 'Earn Wallet', 'For staking/savings')
        "#,
    )
    .execute(pool)
    .await?;

    // Currencies
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO currencies VALUES
            ('VND', 'Vietnamese Dong', 0, '₫'),
            ('USD', 'US Dollar', 2, '$'),
            ('USDT', 'Tether', 6, '₮'),
            ('BTC', 'Bitcoin', 8, '₿'),
            ('ETH', 'Ethereum', 18, 'Ξ')
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Connect to database pool
pub async fn connect(db_path: &Path) -> Result<SqlitePool> {
    let db_url = format!("sqlite:{}", db_path.display());
    SqlitePool::connect(&db_url)
        .await
        .context("Failed to connect to database. Run 'simbank init' first.")
}
