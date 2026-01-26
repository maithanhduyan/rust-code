//! CLI commands

use bibank_core::Amount;
use bibank_ledger::{AccountCategory, AccountKey, JournalEntryBuilder, TransactionIntent, validate_intent};
use rust_decimal::Decimal;
use serde_json::json;

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

// === Phase 2: Trade and Fee commands ===

/// Execute a trade between two users
///
/// Alice sells `sell_amount` of `sell_asset` and buys `buy_amount` of `buy_asset` from Bob.
pub async fn trade(
    ctx: &mut AppContext,
    maker: &str,        // Alice - the one selling
    taker: &str,        // Bob - the one buying
    sell_amount: Decimal,
    sell_asset: &str,
    buy_amount: Decimal,
    buy_asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let sell_amt = Amount::new(sell_amount)?;
    let buy_amt = Amount::new(buy_amount)?;

    // Calculate price for metadata
    let price = sell_amount / buy_amount;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Trade)
        .correlation_id(correlation_id)
        // Sell leg: Maker pays sell_asset, Taker receives
        .debit(AccountKey::user_available(maker, sell_asset), sell_amt)
        .credit(AccountKey::user_available(taker, sell_asset), sell_amt)
        // Buy leg: Taker pays buy_asset, Maker receives
        .debit(AccountKey::user_available(taker, buy_asset), buy_amt)
        .credit(AccountKey::user_available(maker, buy_asset), buy_amt)
        // Metadata
        .metadata("trade_id", json!(correlation_id))
        .metadata("base_asset", json!(buy_asset))
        .metadata("quote_asset", json!(sell_asset))
        .metadata("price", json!(price.to_string()))
        .metadata("base_amount", json!(buy_amount.to_string()))
        .metadata("quote_amount", json!(sell_amount.to_string()))
        .metadata("maker", json!(maker))
        .metadata("taker", json!(taker))
        .build_unsigned()?;

    // Validate trade-specific rules
    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Trade executed: {} sells {} {} for {} {} from {} (seq: {})",
        maker, sell_amount, sell_asset, buy_amount, buy_asset, taker, committed.sequence
    );
    Ok(())
}

/// Charge a fee from a user
pub async fn fee(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    fee_type: &str,  // "trading", "withdrawal", etc.
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    // Fee account: REV:SYSTEM:FEE:<ASSET>:<FEE_TYPE>
    let fee_account = AccountKey::new(
        AccountCategory::Revenue,
        "SYSTEM",
        "FEE",
        asset,
        fee_type.to_uppercase(),
    );

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Fee)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(user_id, asset), amount)
        .credit(fee_account, amount)
        .metadata("fee_amount", json!(amount.to_string()))
        .metadata("fee_asset", json!(asset))
        .metadata("fee_type", json!(fee_type))
        .build_unsigned()?;

    // Validate fee-specific rules
    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Fee charged: {} {} {} from {} (seq: {})",
        amount, asset, fee_type, user_id, committed.sequence
    );
    Ok(())
}

/// Execute a trade with fee (atomic: Trade + Fee entries)
pub async fn trade_with_fee(
    ctx: &mut AppContext,
    maker: &str,
    taker: &str,
    sell_amount: Decimal,
    sell_asset: &str,
    buy_amount: Decimal,
    buy_asset: &str,
    fee_amount: Decimal,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    // First execute the trade
    trade(
        ctx, maker, taker,
        sell_amount, sell_asset,
        buy_amount, buy_asset,
        correlation_id,
    ).await?;

    // Then charge the fee (separate entry, atomic in business sense)
    let fee_correlation = format!("{}-fee", correlation_id);
    fee(ctx, maker, fee_amount, sell_asset, "trading", &fee_correlation).await?;

    Ok(())
}

// === Phase 2.1: Trade History ===

/// List trade history
pub async fn trades(
    ctx: &AppContext,
    user: Option<&str>,
    pair: Option<(&str, &str)>,
    limit: u32,
) -> Result<(), anyhow::Error> {
    let Some(ref projection) = ctx.projection else {
        anyhow::bail!("Projection not available");
    };

    let trades = if let Some(user_id) = user {
        projection.trade.get_user_trades(user_id).await?
    } else if let Some((base, quote)) = pair {
        projection.trade.get_pair_trades(base, quote).await?
    } else {
        projection.trade.get_recent_trades(limit).await?
    };

    if trades.is_empty() {
        println!("No trades found");
        return Ok(());
    }

    println!("Trade History ({} trades):", trades.len());
    println!("{:-<80}", "");
    println!(
        "{:>6} | {:>8} | {:>8} | {:>12} {:>6} | {:>12} {:>6}",
        "ID", "Seller", "Buyer", "Sold", "Asset", "Bought", "Asset"
    );
    println!("{:-<80}", "");

    for trade in trades.iter().take(limit as usize) {
        println!(
            "{:>6} | {:>8} | {:>8} | {:>12} {:>6} | {:>12} {:>6}",
            trade.trade_id,
            trade.seller,
            trade.buyer,
            trade.sell_amount,
            trade.sell_asset,
            trade.buy_amount,
            trade.buy_asset,
        );
    }

    Ok(())
}
