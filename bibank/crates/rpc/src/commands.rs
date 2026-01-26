//! CLI commands

use bibank_core::Amount;
use bibank_ledger::{AccountCategory, AccountKey, JournalEntryBuilder, TransactionIntent, validate_intent};
use bibank_matching::{MatchingEngine, Order, OrderSide, TradingPair};
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

// === Phase 3: Margin Trading Commands ===

/// Borrow funds (margin trading)
///
/// Creates a loan by crediting user's available balance from system loan pool.
pub async fn borrow(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    // LIAB:USER:<USER_ID>:<ASSET>:LOAN - tracks user's loan obligation
    let loan_account = AccountKey::new(
        AccountCategory::Liability,
        "USER",
        user_id,
        asset,
        "LOAN",
    );

    // ASSET:SYSTEM:LENDING_POOL:<ASSET>:MAIN - system lending pool
    let lending_pool = AccountKey::new(
        AccountCategory::Asset,
        "SYSTEM",
        "LENDING_POOL",
        asset,
        "MAIN",
    );

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Borrow)
        .correlation_id(correlation_id)
        // Debit lending pool (reduce system's lending funds)
        .debit(lending_pool, amount)
        // Credit loan liability (increase user's loan obligation)
        .credit(loan_account, amount)
        // Credit user's available balance
        .credit(AccountKey::user_available(user_id, asset), amount)
        // Debit a receivable (to maintain balance)
        .debit(
            AccountKey::new(AccountCategory::Asset, "SYSTEM", "LOANS_RECEIVABLE", asset, "MAIN"),
            amount,
        )
        .metadata("loan_amount", json!(amount.to_string()))
        .metadata("loan_asset", json!(asset))
        .metadata("borrower", json!(user_id))
        .build_unsigned()?;

    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Borrowed {} {} for {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Repay borrowed funds
pub async fn repay(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let loan_account = AccountKey::new(
        AccountCategory::Liability,
        "USER",
        user_id,
        asset,
        "LOAN",
    );

    let lending_pool = AccountKey::new(
        AccountCategory::Asset,
        "SYSTEM",
        "LENDING_POOL",
        asset,
        "MAIN",
    );

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Repay)
        .correlation_id(correlation_id)
        // Debit user's available balance (reduce funds)
        .debit(AccountKey::user_available(user_id, asset), amount)
        // Credit lending pool (return to system)
        .credit(lending_pool, amount)
        // Debit loan liability (reduce user's loan obligation)
        .debit(loan_account, amount)
        // Credit receivable (reduce system's receivable)
        .credit(
            AccountKey::new(AccountCategory::Asset, "SYSTEM", "LOANS_RECEIVABLE", asset, "MAIN"),
            amount,
        )
        .metadata("repay_amount", json!(amount.to_string()))
        .metadata("repay_asset", json!(asset))
        .metadata("borrower", json!(user_id))
        .build_unsigned()?;

    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Repaid {} {} for {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Place a limit order
///
/// Locks collateral and submits order to matching engine.
pub async fn place_order(
    ctx: &mut AppContext,
    user_id: &str,
    side: &str,
    base: &str,
    quote: &str,
    price: Decimal,
    quantity: Decimal,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let order_side = match side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => anyhow::bail!("Invalid order side: {}. Use 'buy' or 'sell'", side),
    };

    let pair = TradingPair::new(base, quote);

    // Calculate lock amount based on side
    let (lock_asset, lock_amount) = match order_side {
        OrderSide::Buy => (quote.to_uppercase(), price * quantity), // Lock quote (USDT)
        OrderSide::Sell => (base.to_uppercase(), quantity),          // Lock base (BTC)
    };

    let lock_amt = Amount::new(lock_amount)?;

    // Create order (get order ID)
    let order = Order::new(user_id, pair.clone(), order_side, price, quantity);
    let order_id = order.id.clone();

    // Create journal entry to lock collateral
    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::OrderPlace)
        .correlation_id(correlation_id)
        // Debit from available balance
        .debit(AccountKey::user_available(user_id, &lock_asset), lock_amt)
        // Credit to locked balance
        .credit(AccountKey::user_locked(user_id, &lock_asset), lock_amt)
        .metadata("order_id", json!(order_id))
        .metadata("order_side", json!(side.to_lowercase()))
        .metadata("base_asset", json!(base.to_uppercase()))
        .metadata("quote_asset", json!(quote.to_uppercase()))
        .metadata("price", json!(price.to_string()))
        .metadata("quantity", json!(quantity.to_string()))
        .metadata("lock_asset", json!(lock_asset))
        .metadata("lock_amount", json!(lock_amount.to_string()))
        .build_unsigned()?;

    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    // TODO: Submit order to matching engine (ctx.matching_engine)
    // For now, just print success

    println!(
        "✅ Order placed: {} {} {} @ {} {} (order_id: {}, seq: {})",
        side.to_uppercase(),
        quantity,
        base.to_uppercase(),
        price,
        quote.to_uppercase(),
        order_id,
        committed.sequence
    );
    Ok(())
}

/// Cancel an open order
pub async fn cancel_order(
    ctx: &mut AppContext,
    order_id: &str,
    base: &str,
    quote: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    // For now, we need to know the lock details from the order
    // In a real implementation, we'd look this up from the matching engine

    // TODO: Look up order from matching engine
    // let order = ctx.matching_engine.get_order(&pair, order_id)?;

    // For demonstration, we'll require the user to provide unlock info
    // In reality, this would be looked up from projection/matching engine

    println!("⚠️  Order cancellation requires order lookup (not yet implemented)");
    println!("   Order ID: {}", order_id);
    println!("   Pair: {}/{}", base.to_uppercase(), quote.to_uppercase());

    Ok(())
}

/// Show margin status for a user
pub async fn margin_status(ctx: &AppContext, user_id: &str) -> Result<(), anyhow::Error> {
    let state = ctx.risk.state();

    println!("Margin Status for {}", user_id);
    println!("{:-<50}", "");

    // Show balances
    println!("\n📊 Balances:");
    for asset in ["USDT", "BTC", "ETH"] {
        let available = state.get_balance(&AccountKey::user_available(user_id, asset));
        let locked = state.get_balance(&AccountKey::user_locked(user_id, asset));

        if !available.is_zero() || !locked.is_zero() {
            println!(
                "   {}: available={}, locked={}",
                asset, available, locked
            );
        }
    }

    // Show loans
    println!("\n💰 Loans:");
    let mut has_loans = false;
    for asset in ["USDT", "BTC", "ETH"] {
        let loan_account = AccountKey::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOAN",
        );
        let loan = state.get_balance(&loan_account);

        if !loan.is_zero() {
            println!("   {}: {}", asset, loan);
            has_loans = true;
        }
    }

    if !has_loans {
        println!("   No active loans");
    }

    // Show margin ratio (simplified)
    let usdt_balance = state.get_balance(&AccountKey::user_available(user_id, "USDT"));
    let usdt_loan = state.get_balance(&AccountKey::new(
        AccountCategory::Liability,
        "USER",
        user_id,
        "USDT",
        "LOAN",
    ));

    if !usdt_loan.is_zero() {
        let margin_ratio = (usdt_balance / usdt_loan) * Decimal::from(100);
        println!("\n📈 Margin Ratio (USDT): {:.2}%", margin_ratio);

        if margin_ratio < Decimal::from(120) {
            println!("   ⚠️  WARNING: Below maintenance margin (120%)");
        } else if margin_ratio < Decimal::from(150) {
            println!("   ⚠️  CAUTION: Approaching maintenance margin");
        } else {
            println!("   ✅ Healthy margin level");
        }
    }

    Ok(())
}

/// Show order book depth
pub async fn order_book(
    ctx: &AppContext,
    base: &str,
    quote: &str,
    depth: usize,
) -> Result<(), anyhow::Error> {
    // TODO: Get from ctx.matching_engine when integrated
    println!("Order Book: {}/{}", base.to_uppercase(), quote.to_uppercase());
    println!("{:-<60}", "");
    println!("(Order book not yet integrated with context)");
    println!("\nTo see order book, matching engine needs to be integrated.");

    Ok(())
}
