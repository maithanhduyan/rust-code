//! Integration tests for BiBank
//!
//! These tests verify the complete flow from CLI commands through
//! ledger, risk engine, events, and projections.

use bibank_core::Amount;
use bibank_events::EventReader;
use bibank_ledger::{
    hash::verify_chain, AccountCategory, AccountKey, JournalEntryBuilder, TransactionIntent,
};
use bibank_rpc::AppContext;
use rust_decimal::Decimal;
use tempfile::TempDir;

fn amount(val: i64) -> Amount {
    Amount::new(Decimal::new(val, 0)).unwrap()
}

/// Test: Genesis → Deposit → Transfer → Balance check
#[tokio::test]
async fn test_full_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // 1. System should not be initialized
    assert!(!ctx.is_initialized());

    // 2. Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();

    ctx.commit(genesis).await.unwrap();
    assert!(ctx.is_initialized());
    assert_eq!(ctx.last_sequence(), 1);

    // 3. Deposit 500 USDT to ALICE
    let deposit = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-1")
        .debit(AccountKey::system_vault("USDT"), amount(500))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(500))
        .build_unsigned()
        .unwrap();

    ctx.commit(deposit).await.unwrap();
    assert_eq!(ctx.last_sequence(), 2);

    // 4. Verify Alice has 500 USDT
    let alice_balance = ctx
        .risk
        .state()
        .get_balance(&AccountKey::user_available("ALICE", "USDT"));
    assert_eq!(alice_balance, Decimal::new(500, 0));

    // 5. Transfer 200 USDT from ALICE to BOB
    let transfer = JournalEntryBuilder::new()
        .intent(TransactionIntent::Transfer)
        .correlation_id("transfer-1")
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(200))
        .credit(AccountKey::user_available("BOB", "USDT"), amount(200))
        .build_unsigned()
        .unwrap();

    ctx.commit(transfer).await.unwrap();
    assert_eq!(ctx.last_sequence(), 3);

    // 6. Verify balances
    let alice_balance = ctx
        .risk
        .state()
        .get_balance(&AccountKey::user_available("ALICE", "USDT"));
    let bob_balance = ctx
        .risk
        .state()
        .get_balance(&AccountKey::user_available("BOB", "USDT"));

    assert_eq!(alice_balance, Decimal::new(300, 0));
    assert_eq!(bob_balance, Decimal::new(200, 0));
}

/// Test: Risk engine blocks overdraft
#[tokio::test]
async fn test_risk_blocks_overdraft() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Deposit 100 USDT to ALICE
    let deposit = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-1")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit).await.unwrap();

    // Try to withdraw 150 USDT (should fail)
    let withdraw = JournalEntryBuilder::new()
        .intent(TransactionIntent::Withdrawal)
        .correlation_id("withdraw-1")
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(150))
        .credit(AccountKey::system_vault("USDT"), amount(150))
        .build_unsigned()
        .unwrap();

    let result = ctx.commit(withdraw).await;
    assert!(result.is_err());

    // Balance should still be 100
    let alice_balance = ctx
        .risk
        .state()
        .get_balance(&AccountKey::user_available("ALICE", "USDT"));
    assert_eq!(alice_balance, Decimal::new(100, 0));
}

/// Test: Hash chain integrity
#[tokio::test]
async fn test_hash_chain_integrity() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Create several entries
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    let deposit = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-1")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit).await.unwrap();

    let transfer = JournalEntryBuilder::new()
        .intent(TransactionIntent::Transfer)
        .correlation_id("transfer-1")
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(50))
        .credit(AccountKey::user_available("BOB", "USDT"), amount(50))
        .build_unsigned()
        .unwrap();
    ctx.commit(transfer).await.unwrap();

    // Verify hash chain
    let reader = EventReader::from_directory(ctx.journal_path()).unwrap();
    let entries = reader.read_all().unwrap();

    assert_eq!(entries.len(), 3);
    assert!(verify_chain(&entries).is_ok());

    // Verify sequence is continuous
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.sequence, (i + 1) as u64);
    }

    // Verify first entry has GENESIS prev_hash
    assert_eq!(entries[0].prev_hash, "GENESIS");
}

/// Test: Replay rebuilds identical state
#[tokio::test]
async fn test_replay_rebuilds_state() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    // Phase 1: Create some entries
    {
        let mut ctx = AppContext::new(data_path).await.unwrap();

        let genesis = JournalEntryBuilder::new()
            .intent(TransactionIntent::Genesis)
            .correlation_id("genesis-1")
            .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
            .credit(
                AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
                amount(1_000_000),
            )
            .build_unsigned()
            .unwrap();
        ctx.commit(genesis).await.unwrap();

        let deposit = JournalEntryBuilder::new()
            .intent(TransactionIntent::Deposit)
            .correlation_id("deposit-1")
            .debit(AccountKey::system_vault("USDT"), amount(100))
            .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .build_unsigned()
            .unwrap();
        ctx.commit(deposit).await.unwrap();

        let transfer = JournalEntryBuilder::new()
            .intent(TransactionIntent::Transfer)
            .correlation_id("transfer-1")
            .debit(AccountKey::user_available("ALICE", "USDT"), amount(30))
            .credit(AccountKey::user_available("BOB", "USDT"), amount(30))
            .build_unsigned()
            .unwrap();
        ctx.commit(transfer).await.unwrap();
    }

    // Phase 2: Reopen and verify state is rebuilt correctly
    {
        let ctx = AppContext::new(data_path).await.unwrap();

        assert_eq!(ctx.last_sequence(), 3);

        let alice_balance = ctx
            .risk
            .state()
            .get_balance(&AccountKey::user_available("ALICE", "USDT"));
        let bob_balance = ctx
            .risk
            .state()
            .get_balance(&AccountKey::user_available("BOB", "USDT"));

        assert_eq!(alice_balance, Decimal::new(70, 0));
        assert_eq!(bob_balance, Decimal::new(30, 0));
    }
}

/// Test: Double-entry validation rejects unbalanced entries
#[test]
fn test_double_entry_validation() {
    // Unbalanced entry should fail
    let result = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("bad-1")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(50)) // Unbalanced!
        .build_unsigned();

    assert!(result.is_err());

    // Single posting should fail
    let result = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("bad-2")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .build_unsigned();

    assert!(result.is_err());
}

/// Test: Multi-asset entry (Trade preparation for Phase 2)
#[tokio::test]
async fn test_multi_asset_balanced() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis with both USDT and BTC
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Deposit USDT to Alice
    let deposit_usdt = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-usdt")
        .debit(AccountKey::system_vault("USDT"), amount(1000))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(1000))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit_usdt).await.unwrap();

    // Deposit BTC to Bob (simulate with small integer for test)
    let deposit_btc = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-btc")
        .debit(AccountKey::system_vault("BTC"), amount(10))
        .credit(AccountKey::user_available("BOB", "BTC"), amount(10))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit_btc).await.unwrap();

    // Verify multi-asset balances
    let alice_usdt = ctx
        .risk
        .state()
        .get_balance(&AccountKey::user_available("ALICE", "USDT"));
    let bob_btc = ctx
        .risk
        .state()
        .get_balance(&AccountKey::user_available("BOB", "BTC"));

    assert_eq!(alice_usdt, Decimal::new(1000, 0));
    assert_eq!(bob_btc, Decimal::new(10, 0));
}

// =============================================================================
// Phase 2 Integration Tests
// =============================================================================

/// Test: Trade between two users (multi-asset atomic swap)
#[tokio::test]
async fn test_trade_multi_asset_swap() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Deposit USDT to Alice
    let deposit_usdt = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-usdt")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit_usdt).await.unwrap();

    // Deposit BTC to Bob
    let deposit_btc = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-btc")
        .debit(AccountKey::system_vault("BTC"), amount(1))
        .credit(AccountKey::user_available("BOB", "BTC"), amount(1))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit_btc).await.unwrap();

    // Trade: Alice sells 100 USDT for 1 BTC from Bob
    let trade = JournalEntryBuilder::new()
        .intent(TransactionIntent::Trade)
        .correlation_id("trade-1")
        // USDT leg: Alice pays, Bob receives
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .credit(AccountKey::user_available("BOB", "USDT"), amount(100))
        // BTC leg: Bob pays, Alice receives
        .debit(AccountKey::user_available("BOB", "BTC"), amount(1))
        .credit(AccountKey::user_available("ALICE", "BTC"), amount(1))
        .build_unsigned()
        .unwrap();

    ctx.commit(trade).await.unwrap();

    // Verify final balances
    let alice_usdt = ctx.risk.state().get_balance(&AccountKey::user_available("ALICE", "USDT"));
    let alice_btc = ctx.risk.state().get_balance(&AccountKey::user_available("ALICE", "BTC"));
    let bob_usdt = ctx.risk.state().get_balance(&AccountKey::user_available("BOB", "USDT"));
    let bob_btc = ctx.risk.state().get_balance(&AccountKey::user_available("BOB", "BTC"));

    assert_eq!(alice_usdt, Decimal::ZERO);  // Sold all USDT
    assert_eq!(alice_btc, Decimal::new(1, 0));  // Got 1 BTC
    assert_eq!(bob_usdt, Decimal::new(100, 0));  // Got 100 USDT
    assert_eq!(bob_btc, Decimal::ZERO);  // Sold all BTC
}

/// Test: Fee collection
#[tokio::test]
async fn test_fee_collection() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Deposit to Alice
    let deposit = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-1")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit).await.unwrap();

    // Charge fee
    let fee = JournalEntryBuilder::new()
        .intent(TransactionIntent::Fee)
        .correlation_id("fee-1")
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(1))
        .credit(AccountKey::fee_revenue("USDT"), amount(1))
        .build_unsigned()
        .unwrap();

    ctx.commit(fee).await.unwrap();

    // Verify balances
    let alice_balance = ctx.risk.state().get_balance(&AccountKey::user_available("ALICE", "USDT"));
    let fee_revenue = ctx.risk.state().get_balance(&AccountKey::fee_revenue("USDT"));

    assert_eq!(alice_balance, Decimal::new(99, 0));
    assert_eq!(fee_revenue, Decimal::new(1, 0));
}

/// Test: Trade + Fee (atomic flow)
#[tokio::test]
async fn test_trade_with_fee() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Deposit USDT to Alice (extra for fee)
    let deposit_usdt = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-usdt")
        .debit(AccountKey::system_vault("USDT"), amount(101))  // 100 for trade + 1 for fee
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(101))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit_usdt).await.unwrap();

    // Deposit BTC to Bob
    let deposit_btc = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-btc")
        .debit(AccountKey::system_vault("BTC"), amount(1))
        .credit(AccountKey::user_available("BOB", "BTC"), amount(1))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit_btc).await.unwrap();

    // Trade
    let trade = JournalEntryBuilder::new()
        .intent(TransactionIntent::Trade)
        .correlation_id("trade-1")
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .credit(AccountKey::user_available("BOB", "USDT"), amount(100))
        .debit(AccountKey::user_available("BOB", "BTC"), amount(1))
        .credit(AccountKey::user_available("ALICE", "BTC"), amount(1))
        .build_unsigned()
        .unwrap();
    ctx.commit(trade).await.unwrap();

    // Fee (separate entry)
    let fee = JournalEntryBuilder::new()
        .intent(TransactionIntent::Fee)
        .correlation_id("fee-1")
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(1))
        .credit(AccountKey::fee_revenue("USDT"), amount(1))
        .build_unsigned()
        .unwrap();
    ctx.commit(fee).await.unwrap();

    // Verify
    let alice_usdt = ctx.risk.state().get_balance(&AccountKey::user_available("ALICE", "USDT"));
    let fee_revenue = ctx.risk.state().get_balance(&AccountKey::fee_revenue("USDT"));

    assert_eq!(alice_usdt, Decimal::ZERO);  // 101 - 100 (trade) - 1 (fee) = 0
    assert_eq!(fee_revenue, Decimal::new(1, 0));
}

/// Test: Digital signatures
#[tokio::test]
async fn test_digital_signatures() {
    use bibank_ledger::SystemSigner;

    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    // Set up signer via env var
    let signer = SystemSigner::generate();
    std::env::set_var("BIBANK_SYSTEM_KEY", signer.seed_hex());

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();

    let entry = ctx.commit(genesis).await.unwrap();

    // Entry should have a signature
    assert!(entry.has_system_signature());
    assert_eq!(entry.signatures.len(), 1);
    assert_eq!(entry.signatures[0].signer_id, "SYSTEM");

    // Signature should verify
    assert!(entry.verify_signatures().is_ok());

    // Clean up env var
    std::env::remove_var("BIBANK_SYSTEM_KEY");
}

/// Test: Trade risk check blocks insufficient balance
#[tokio::test]
async fn test_trade_risk_blocks_insufficient() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Deposit only 50 USDT to Alice (not enough for 100 trade)
    let deposit_usdt = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-usdt")
        .debit(AccountKey::system_vault("USDT"), amount(50))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(50))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit_usdt).await.unwrap();

    // Deposit BTC to Bob
    let deposit_btc = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-btc")
        .debit(AccountKey::system_vault("BTC"), amount(1))
        .credit(AccountKey::user_available("BOB", "BTC"), amount(1))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit_btc).await.unwrap();

    // Trade should fail - Alice doesn't have 100 USDT
    let trade = JournalEntryBuilder::new()
        .intent(TransactionIntent::Trade)
        .correlation_id("trade-1")
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .credit(AccountKey::user_available("BOB", "USDT"), amount(100))
        .debit(AccountKey::user_available("BOB", "BTC"), amount(1))
        .credit(AccountKey::user_available("ALICE", "BTC"), amount(1))
        .build_unsigned()
        .unwrap();

    let result = ctx.commit(trade).await;
    assert!(result.is_err(), "Trade should fail due to insufficient USDT");

    // Balances should be unchanged
    let alice_usdt = ctx.risk.state().get_balance(&AccountKey::user_available("ALICE", "USDT"));
    assert_eq!(alice_usdt, Decimal::new(50, 0));
}

// ============================================================================
// Phase 3: Margin Trading Tests
// ============================================================================

/// Helper: Create loan account key
fn loan_account(user: &str, asset: &str) -> AccountKey {
    AccountKey::new(AccountCategory::Asset, "USER", user, asset, "LOAN")
}

/// Test: Borrow and Repay workflow
#[tokio::test]
async fn test_margin_borrow_repay() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Alice deposits 100 USDT (her equity)
    let deposit = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-1")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit).await.unwrap();

    // Alice borrows 500 USDT (5x leverage, within 10x limit)
    let borrow = JournalEntryBuilder::new()
        .intent(TransactionIntent::Borrow)
        .correlation_id("borrow-1")
        .debit(loan_account("ALICE", "USDT"), amount(500))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(500))
        .build_unsigned()
        .unwrap();
    ctx.commit(borrow).await.unwrap();

    // Verify: Alice has 600 available, 500 loan
    let alice_available = ctx.risk.state().get_available_balance("ALICE", "USDT");
    let alice_loan = ctx.risk.state().get_loan_balance("ALICE", "USDT");
    let alice_equity = ctx.risk.state().get_equity("ALICE", "USDT");

    assert_eq!(alice_available, Decimal::new(600, 0), "Available = 100 + 500");
    assert_eq!(alice_loan, Decimal::new(500, 0), "Loan = 500");
    assert_eq!(alice_equity, Decimal::new(100, 0), "Equity = Available - Loan");

    // Alice repays 200 USDT
    let repay = JournalEntryBuilder::new()
        .intent(TransactionIntent::Repay)
        .correlation_id("repay-1")
        .debit(AccountKey::user_available("ALICE", "USDT"), amount(200))
        .credit(loan_account("ALICE", "USDT"), amount(200))
        .build_unsigned()
        .unwrap();
    ctx.commit(repay).await.unwrap();

    // Verify: Alice has 400 available, 300 loan
    let alice_available = ctx.risk.state().get_available_balance("ALICE", "USDT");
    let alice_loan = ctx.risk.state().get_loan_balance("ALICE", "USDT");

    assert_eq!(alice_available, Decimal::new(400, 0), "Available = 600 - 200");
    assert_eq!(alice_loan, Decimal::new(300, 0), "Loan = 500 - 200");
}

/// Test: Interest accrual increases loan
#[tokio::test]
async fn test_margin_interest_accrual() {
    use bibank_risk::InterestCalculator;

    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Alice deposits 100, borrows 1000
    let deposit = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-1")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit).await.unwrap();

    let borrow = JournalEntryBuilder::new()
        .intent(TransactionIntent::Borrow)
        .correlation_id("borrow-1")
        .debit(loan_account("ALICE", "USDT"), amount(1000))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(1000))
        .build_unsigned()
        .unwrap();
    ctx.commit(borrow).await.unwrap();

    // Generate interest entries
    let calc = InterestCalculator::new();
    let interest_entries = calc.generate_interest_entries(ctx.risk.state(), "daily-2025-01-01");

    assert_eq!(interest_entries.len(), 1, "Should have 1 interest entry for Alice");

    // Commit interest entry
    ctx.commit(interest_entries[0].clone()).await.unwrap();

    // Verify: Loan increased by 0.5 (1000 * 0.0005)
    let alice_loan = ctx.risk.state().get_loan_balance("ALICE", "USDT");
    assert_eq!(alice_loan, Decimal::from_str_exact("1000.5").unwrap());
}

/// Test: Margin ratio calculation
#[tokio::test]
async fn test_margin_ratio_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path();

    let mut ctx = AppContext::new(data_path).await.unwrap();

    // Genesis
    let genesis = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id("genesis-1")
        .debit(AccountKey::system_vault("USDT"), amount(1_000_000))
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            amount(1_000_000),
        )
        .build_unsigned()
        .unwrap();
    ctx.commit(genesis).await.unwrap();

    // Alice deposits 100, borrows 400
    let deposit = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id("deposit-1")
        .debit(AccountKey::system_vault("USDT"), amount(100))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
        .build_unsigned()
        .unwrap();
    ctx.commit(deposit).await.unwrap();

    let borrow = JournalEntryBuilder::new()
        .intent(TransactionIntent::Borrow)
        .correlation_id("borrow-1")
        .debit(loan_account("ALICE", "USDT"), amount(400))
        .credit(AccountKey::user_available("ALICE", "USDT"), amount(400))
        .build_unsigned()
        .unwrap();
    ctx.commit(borrow).await.unwrap();

    // Margin Ratio = Available / Loan = 500 / 400 = 1.25
    let ratio = ctx.risk.state().get_margin_ratio("ALICE", "USDT");
    assert_eq!(ratio, Decimal::from_str_exact("1.25").unwrap());

    // Not liquidatable (ratio > 1.0)
    assert!(!ctx.risk.state().is_liquidatable("ALICE", "USDT"));
}
