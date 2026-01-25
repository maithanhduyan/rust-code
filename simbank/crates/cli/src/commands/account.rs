//! Account management commands

use anyhow::{Context, Result};
use chrono::Utc;
use simbank_core::{Event, PersonType, WalletType};
use simbank_persistence::EventStore;
use sqlx::SqlitePool;
use std::path::Path;

use crate::db;
use crate::{AccountAction, PersonTypeArg};

/// Handle account subcommands
pub async fn handle(db_path: &Path, events_dir: &Path, action: AccountAction) -> Result<()> {
    let pool = db::connect(db_path).await?;
    let events = EventStore::new(events_dir)?;

    match action {
        AccountAction::Create { r#type, name, email } => {
            create_account(&pool, &events, r#type, &name, email.as_deref()).await?;
        }
        AccountAction::List { r#type } => {
            list_accounts(&pool, r#type).await?;
        }
        AccountAction::Show { account_id } => {
            show_account(&pool, &account_id).await?;
        }
        AccountAction::Balance { account_id } => {
            show_balance(&pool, &account_id).await?;
        }
    }

    pool.close().await;
    Ok(())
}

/// Create a new account
async fn create_account(
    pool: &SqlitePool,
    events: &EventStore,
    person_type: PersonTypeArg,
    name: &str,
    email: Option<&str>,
) -> Result<()> {
    let core_type = person_type.to_core_type();

    // Generate IDs
    let person_id = generate_person_id(pool, &core_type).await?;
    let account_id = generate_account_id(pool).await?;

    // Insert person
    sqlx::query(
        "INSERT INTO persons (id, person_type, name, email, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&person_id)
    .bind(core_type.as_str())
    .bind(name)
    .bind(email)
    .bind(Utc::now())
    .execute(pool)
    .await
    .context("Failed to create person")?;

    // Insert account
    sqlx::query("INSERT INTO accounts (id, person_id, status, created_at) VALUES (?, ?, 'active', ?)")
        .bind(&account_id)
        .bind(&person_id)
        .bind(Utc::now())
        .execute(pool)
        .await
        .context("Failed to create account")?;

    // Create wallets based on person type
    let wallet_types = get_wallet_types_for_person(&core_type);
    for wallet_type in &wallet_types {
        let wallet_id = generate_wallet_id(pool).await?;
        sqlx::query(
            "INSERT INTO wallets (id, account_id, wallet_type, status, created_at) VALUES (?, ?, ?, 'active', ?)",
        )
        .bind(&wallet_id)
        .bind(&account_id)
        .bind(wallet_type.as_str())
        .bind(Utc::now())
        .execute(pool)
        .await
        .context("Failed to create wallet")?;
    }

    // Record event
    let event = Event::new(
        format!("EVT_ACC_{}", account_id),
        simbank_core::EventType::AccountCreated,
        person_id.clone(),
        core_type,
        account_id.clone(),
    );
    events.append(&event)?;

    println!("✅ Created {} account:", core_type.as_str());
    println!("   Person ID:  {}", person_id);
    println!("   Account ID: {}", account_id);
    println!("   Name:       {}", name);
    if let Some(email) = email {
        println!("   Email:      {}", email);
    }
    println!("   Wallets:    {:?}", wallet_types.iter().map(|w| w.as_str()).collect::<Vec<_>>());

    Ok(())
}

/// List accounts
async fn list_accounts(pool: &SqlitePool, filter_type: Option<PersonTypeArg>) -> Result<()> {
    let query = match filter_type {
        Some(pt) => {
            let type_str = pt.to_core_type().as_str();
            sqlx::query_as::<_, (String, String, String, String, String)>(
                r#"
                SELECT a.id, p.id, p.name, p.person_type, a.status
                FROM accounts a
                JOIN persons p ON a.person_id = p.id
                WHERE p.person_type = ?
                ORDER BY a.created_at DESC
                "#,
            )
            .bind(type_str)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, (String, String, String, String, String)>(
                r#"
                SELECT a.id, p.id, p.name, p.person_type, a.status
                FROM accounts a
                JOIN persons p ON a.person_id = p.id
                ORDER BY a.created_at DESC
                "#,
            )
            .fetch_all(pool)
            .await?
        }
    };

    if query.is_empty() {
        println!("No accounts found.");
        return Ok(());
    }

    println!("{:<12} {:<12} {:<20} {:<12} {:<8}", "ACCOUNT", "PERSON", "NAME", "TYPE", "STATUS");
    println!("{}", "-".repeat(70));
    for (acc_id, person_id, name, person_type, status) in query {
        println!("{:<12} {:<12} {:<20} {:<12} {:<8}", acc_id, person_id, name, person_type, status);
    }

    Ok(())
}

/// Show account details
async fn show_account(pool: &SqlitePool, account_id: &str) -> Result<()> {
    let account = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT a.id, p.name, p.person_type, a.status
        FROM accounts a
        JOIN persons p ON a.person_id = p.id
        WHERE a.id = ?
        "#,
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;

    match account {
        Some((acc_id, name, person_type, status)) => {
            println!("📋 Account Details");
            println!("   Account ID: {}", acc_id);
            println!("   Name:       {}", name);
            println!("   Type:       {}", person_type);
            println!("   Status:     {}", status);

            // List wallets
            let wallets = sqlx::query_as::<_, (String, String)>(
                "SELECT id, wallet_type FROM wallets WHERE account_id = ?",
            )
            .bind(account_id)
            .fetch_all(pool)
            .await?;

            println!("\n   Wallets:");
            for (wallet_id, wallet_type) in wallets {
                println!("     - {} ({})", wallet_id, wallet_type);
            }
        }
        None => {
            println!("❌ Account '{}' not found", account_id);
        }
    }

    Ok(())
}

/// Show account balances
async fn show_balance(pool: &SqlitePool, account_id: &str) -> Result<()> {
    let balances = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT w.wallet_type, b.currency_code, b.available, b.locked
        FROM wallets w
        JOIN balances b ON w.id = b.wallet_id
        WHERE w.account_id = ?
        ORDER BY w.wallet_type, b.currency_code
        "#,
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;

    if balances.is_empty() {
        println!("No balances found for account '{}'", account_id);
        return Ok(());
    }

    println!("💰 Balances for {}", account_id);
    println!("{:<12} {:<8} {:>18} {:>18}", "WALLET", "CURRENCY", "AVAILABLE", "LOCKED");
    println!("{}", "-".repeat(60));
    for (wallet_type, currency, available, locked) in balances {
        println!("{:<12} {:<8} {:>18} {:>18}", wallet_type, currency, available, locked);
    }

    Ok(())
}

/// Get wallet types for a person type
fn get_wallet_types_for_person(person_type: &PersonType) -> Vec<WalletType> {
    match person_type {
        PersonType::Customer => vec![WalletType::Spot, WalletType::Funding],
        PersonType::Employee => vec![WalletType::Funding],
        PersonType::Shareholder => vec![WalletType::Funding],
        PersonType::Manager | PersonType::Auditor => vec![], // No wallets
    }
}

/// Generate next person ID
async fn generate_person_id(pool: &SqlitePool, person_type: &PersonType) -> Result<String> {
    let prefix = match person_type {
        PersonType::Customer => "CUST",
        PersonType::Employee => "EMP",
        PersonType::Shareholder => "SH",
        PersonType::Manager => "MGR",
        PersonType::Auditor => "AUD",
    };

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM persons WHERE person_type = ?")
        .bind(person_type.as_str())
        .fetch_one(pool)
        .await?;

    Ok(format!("{}_{:03}", prefix, count.0 + 1))
}

/// Generate next account ID
async fn generate_account_id(pool: &SqlitePool) -> Result<String> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
        .fetch_one(pool)
        .await?;
    Ok(format!("ACC_{:03}", count.0 + 1))
}

/// Generate next wallet ID
async fn generate_wallet_id(pool: &SqlitePool) -> Result<String> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM wallets")
        .fetch_one(pool)
        .await?;
    Ok(format!("WAL_{:03}", count.0 + 1))
}
