//! Integration tests for BiBank Phase 1
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
