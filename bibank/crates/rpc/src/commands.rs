//! CLI commands

use bibank_core::Amount;
use bibank_ledger::{AccountCategory, AccountKey, JournalEntryBuilder, TransactionIntent};
use rust_decimal::Decimal;

use crate::context::AppContext;

/// Initialize the system with Genesis entry
pub async fn init(ctx: &mut AppContext, correlation_id: &str) -> Result<(), anyhow::Error> {
    if ctx.is_initialized() {
        anyhow::bail!("System already initialized (sequence = {})", ctx.last_sequence());
    }

    // Create Genesis entry with initial system capital
    let initial_capital = Amount::new(Decimal::new(1_000_000_000, 0))?; // 1 billion units

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id(correlation_id)
        .debit(AccountKey::system_vault("USDT"), initial_capital)
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            initial_capital,
        )
        .build_unsigned()?;

    ctx.commit(entry).await?;

    println!("✅ System initialized with Genesis entry");
    Ok(())
}

/// Deposit funds to a user
pub async fn deposit(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id(correlation_id)
        .debit(AccountKey::system_vault(asset), amount)
        .credit(AccountKey::user_available(user_id, asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Deposited {} {} to {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Transfer funds between users
pub async fn transfer(
    ctx: &mut AppContext,
    from_user: &str,
    to_user: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Transfer)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(from_user, asset), amount)
        .credit(AccountKey::user_available(to_user, asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Transferred {} {} from {} to {} (seq: {})",
        amount, asset, from_user, to_user, committed.sequence
    );
    Ok(())
}

/// Withdraw funds from a user
pub async fn withdraw(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Withdrawal)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(user_id, asset), amount)
        .credit(AccountKey::system_vault(asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Withdrew {} {} from {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Get balance for a user
pub async fn balance(ctx: &AppContext, user_id: &str) -> Result<(), anyhow::Error> {
    let account = AccountKey::user_available(user_id, "USDT");
    let balance = ctx.risk.state().get_balance(&account);

    println!("Balance for {}: {} USDT", user_id, balance);

    // Check other common assets
    for asset in ["BTC", "ETH", "USD"] {
        let account = AccountKey::user_available(user_id, asset);
        let bal = ctx.risk.state().get_balance(&account);
        if !bal.is_zero() {
            println!("              {} {}", bal, asset);
        }
    }

    Ok(())
}
