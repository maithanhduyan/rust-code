//! Wallet operations: deposit, withdraw, transfer

use anyhow::{bail, Result};
use chrono::Utc;
use rust_decimal::Decimal;
use simbank_core::Event;
use simbank_persistence::EventStore;
use sqlx::SqlitePool;
use std::path::Path;

use crate::db;
use crate::WalletTypeArg;

/// Deposit funds to an account
pub async fn deposit(
    db_path: &Path,
    events_dir: &Path,
    account_id: &str,
    amount: Decimal,
    currency: &str,
    to: WalletTypeArg,
) -> Result<()> {
    let pool = db::connect(db_path).await?;
    let events = EventStore::new(events_dir)?;
    let wallet_type = to.to_core_type();

    // Validate account exists
    let person_id = get_person_id(&pool, account_id).await?;

    // Get wallet
    let wallet_id = get_wallet(&pool, account_id, wallet_type.as_str()).await?;

    // Update balance
    update_balance(&pool, &wallet_id, currency, amount).await?;

    // Record transaction
    let tx_id = generate_tx_id(&pool).await?;
    sqlx::query(
        "INSERT INTO transactions (id, account_id, wallet_id, tx_type, amount, currency_code, description, created_at)
         VALUES (?, ?, ?, 'deposit', ?, ?, ?, ?)",
    )
    .bind(&tx_id)
    .bind(account_id)
    .bind(&wallet_id)
    .bind(amount.to_string())
    .bind(currency)
    .bind(format!("Deposit {} {} to {}", amount, currency, wallet_type.as_str()))
    .bind(Utc::now())
    .execute(&pool)
    .await?;

    // Record event
    let event = Event::deposit(&tx_id, &person_id, account_id, amount, currency);
    events.append(&event)?;

    println!("✅ Deposit successful!");
    println!("   Transaction: {}", tx_id);
    println!("   Amount:      {} {}", amount, currency);
    println!("   To:          {} ({})", wallet_type.as_str(), wallet_id);

    pool.close().await;
    Ok(())
}

/// Withdraw funds from an account
pub async fn withdraw(
    db_path: &Path,
    events_dir: &Path,
    account_id: &str,
    amount: Decimal,
    currency: &str,
    from: WalletTypeArg,
) -> Result<()> {
    let pool = db::connect(db_path).await?;
    let events = EventStore::new(events_dir)?;
    let wallet_type = from.to_core_type();

    // Validate account exists
    let person_id = get_person_id(&pool, account_id).await?;

    // Get wallet
    let wallet_id = get_wallet(&pool, account_id, wallet_type.as_str()).await?;

    // Check balance
    let balance = get_balance(&pool, &wallet_id, currency).await?;
    if balance < amount {
        bail!(
            "Insufficient balance: {} {} available, {} {} requested",
            balance, currency, amount, currency
        );
    }

    // Update balance (subtract)
    update_balance(&pool, &wallet_id, currency, -amount).await?;

    // Record transaction
    let tx_id = generate_tx_id(&pool).await?;
    sqlx::query(
        "INSERT INTO transactions (id, account_id, wallet_id, tx_type, amount, currency_code, description, created_at)
         VALUES (?, ?, ?, 'withdrawal', ?, ?, ?, ?)",
    )
    .bind(&tx_id)
    .bind(account_id)
    .bind(&wallet_id)
    .bind(amount.to_string())
    .bind(currency)
    .bind(format!("Withdrawal {} {} from {}", amount, currency, wallet_type.as_str()))
    .bind(Utc::now())
    .execute(&pool)
    .await?;

    // Record event
    let event = Event::withdrawal(&tx_id, &person_id, account_id, amount, currency);
    events.append(&event)?;

    println!("✅ Withdrawal successful!");
    println!("   Transaction: {}", tx_id);
    println!("   Amount:      {} {}", amount, currency);
    println!("   From:        {} ({})", wallet_type.as_str(), wallet_id);

    pool.close().await;
    Ok(())
}

/// Transfer funds between wallets
pub async fn transfer(
    db_path: &Path,
    events_dir: &Path,
    account_id: &str,
    amount: Decimal,
    currency: &str,
    from: WalletTypeArg,
    to: WalletTypeArg,
) -> Result<()> {
    let pool = db::connect(db_path).await?;
    let events = EventStore::new(events_dir)?;
    let from_type = from.to_core_type();
    let to_type = to.to_core_type();

    if from_type == to_type {
        bail!("Source and destination wallets must be different");
    }

    // Validate account exists
    let person_id = get_person_id(&pool, account_id).await?;

    // Get wallets
    let from_wallet_id = get_wallet(&pool, account_id, from_type.as_str()).await?;
    let to_wallet_id = get_wallet(&pool, account_id, to_type.as_str()).await?;

    // Check balance
    let balance = get_balance(&pool, &from_wallet_id, currency).await?;
    if balance < amount {
        bail!(
            "Insufficient balance: {} {} available, {} {} requested",
            balance, currency, amount, currency
        );
    }

    // Update balances
    update_balance(&pool, &from_wallet_id, currency, -amount).await?;
    update_balance(&pool, &to_wallet_id, currency, amount).await?;

    // Record transaction
    let tx_id = generate_tx_id(&pool).await?;
    sqlx::query(
        "INSERT INTO transactions (id, account_id, wallet_id, tx_type, amount, currency_code, description, created_at)
         VALUES (?, ?, ?, 'internal_transfer', ?, ?, ?, ?)",
    )
    .bind(&tx_id)
    .bind(account_id)
    .bind(&from_wallet_id)
    .bind(amount.to_string())
    .bind(currency)
    .bind(format!("Transfer {} {} from {} to {}", amount, currency, from_type.as_str(), to_type.as_str()))
    .bind(Utc::now())
    .execute(&pool)
    .await?;

    // Record event
    let event = Event::internal_transfer(
        &tx_id,
        &person_id,
        account_id,
        from_type,
        to_type,
        amount,
        currency,
    );
    events.append(&event)?;

    println!("✅ Transfer successful!");
    println!("   Transaction: {}", tx_id);
    println!("   Amount:      {} {}", amount, currency);
    println!("   From:        {} ({})", from_type.as_str(), from_wallet_id);
    println!("   To:          {} ({})", to_type.as_str(), to_wallet_id);

    pool.close().await;
    Ok(())
}

// ============================================================================
// Helper functions
// ============================================================================

async fn get_person_id(pool: &SqlitePool, account_id: &str) -> Result<String> {
    let result = sqlx::query_as::<_, (String,)>(
        "SELECT person_id FROM accounts WHERE id = ?",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;

    match result {
        Some((person_id,)) => Ok(person_id),
        None => bail!("Account '{}' not found", account_id),
    }
}

async fn get_wallet(pool: &SqlitePool, account_id: &str, wallet_type: &str) -> Result<String> {
    let result = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM wallets WHERE account_id = ? AND wallet_type = ?",
    )
    .bind(account_id)
    .bind(wallet_type)
    .fetch_optional(pool)
    .await?;

    match result {
        Some((wallet_id,)) => Ok(wallet_id),
        None => bail!(
            "Wallet type '{}' not found for account '{}'",
            wallet_type,
            account_id
        ),
    }
}

async fn get_balance(pool: &SqlitePool, wallet_id: &str, currency: &str) -> Result<Decimal> {
    let result = sqlx::query_as::<_, (String,)>(
        "SELECT available FROM balances WHERE wallet_id = ? AND currency_code = ?",
    )
    .bind(wallet_id)
    .bind(currency)
    .fetch_optional(pool)
    .await?;

    match result {
        Some((available,)) => Ok(available.parse().unwrap_or(Decimal::ZERO)),
        None => Ok(Decimal::ZERO),
    }
}

async fn update_balance(
    pool: &SqlitePool,
    wallet_id: &str,
    currency: &str,
    delta: Decimal,
) -> Result<()> {
    // Try to update existing balance
    let result = sqlx::query(
        "UPDATE balances SET available = CAST((CAST(available AS REAL) + ?) AS TEXT), updated_at = ?
         WHERE wallet_id = ? AND currency_code = ?",
    )
    .bind(delta.to_string())
    .bind(Utc::now())
    .bind(wallet_id)
    .bind(currency)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        // Insert new balance record
        let initial = if delta > Decimal::ZERO { delta } else { Decimal::ZERO };
        sqlx::query(
            "INSERT INTO balances (wallet_id, currency_code, available, locked, updated_at)
             VALUES (?, ?, ?, '0', ?)",
        )
        .bind(wallet_id)
        .bind(currency)
        .bind(initial.to_string())
        .bind(Utc::now())
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn generate_tx_id(pool: &SqlitePool) -> Result<String> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
        .fetch_one(pool)
        .await?;
    Ok(format!("TXN_{:03}", count.0 + 1))
}
