//! Intent-specific validation rules
//!
//! Each TransactionIntent has specific validation rules beyond basic double-entry.

use crate::account::AccountCategory;
use crate::entry::{Posting, Side, TransactionIntent, UnsignedEntry};
use crate::error::LedgerError;

/// Validation result with detailed error
pub type ValidationResult = Result<(), LedgerError>;

/// Validate an unsigned entry according to its intent
pub fn validate_intent(entry: &UnsignedEntry) -> ValidationResult {
    match entry.intent {
        TransactionIntent::Genesis => validate_genesis(entry),
        TransactionIntent::Deposit => validate_deposit(entry),
        TransactionIntent::Withdrawal => validate_withdrawal(entry),
        TransactionIntent::Transfer => validate_transfer(entry),
        TransactionIntent::Trade => validate_trade(entry),
        TransactionIntent::Fee => validate_fee(entry),
        TransactionIntent::Adjustment => validate_adjustment(entry),
        // Phase 3: Margin Trading
        TransactionIntent::Borrow => validate_borrow(entry),
        TransactionIntent::Repay => validate_repay(entry),
        TransactionIntent::Interest => validate_interest(entry),
        TransactionIntent::Liquidation => validate_liquidation(entry),
        TransactionIntent::OrderPlace => validate_order_place(entry),
        TransactionIntent::OrderCancel => validate_order_cancel(entry),
    }
}

/// Genesis: ASSET ↑, EQUITY ↑
fn validate_genesis(entry: &UnsignedEntry) -> ValidationResult {
    for posting in &entry.postings {
        match posting.account.category {
            AccountCategory::Asset | AccountCategory::Equity => {}
            _ => {
                return Err(LedgerError::InvalidIntentPosting {
                    intent: "Genesis",
                    account: posting.account.to_string(),
                    reason: "Genesis only allows ASSET and EQUITY accounts",
                });
            }
        }
    }
    Ok(())
}

/// Deposit: ASSET ↑, LIAB ↑
fn validate_deposit(entry: &UnsignedEntry) -> ValidationResult {
    let has_asset_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset && p.side == Side::Debit
    });
    let has_liab_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Credit
    });

    if !has_asset_debit || !has_liab_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Deposit",
            account: String::new(),
            reason: "Deposit requires ASSET debit and LIAB credit",
        });
    }
    Ok(())
}

/// Withdrawal: ASSET ↓, LIAB ↓
fn validate_withdrawal(entry: &UnsignedEntry) -> ValidationResult {
    let has_asset_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset && p.side == Side::Credit
    });
    let has_liab_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Debit
    });

    if !has_asset_credit || !has_liab_debit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Withdrawal",
            account: String::new(),
            reason: "Withdrawal requires ASSET credit and LIAB debit",
        });
    }
    Ok(())
}

/// Transfer: LIAB only
fn validate_transfer(entry: &UnsignedEntry) -> ValidationResult {
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Transfer",
                account: posting.account.to_string(),
                reason: "Transfer only allows LIAB accounts",
            });
        }
    }
    Ok(())
}

/// Trade: LIAB only, exactly 2 assets, min 4 postings, zero-sum per asset
pub fn validate_trade(entry: &UnsignedEntry) -> ValidationResult {
    // Rule 1: Min 4 postings (2 legs × 2 sides)
    if entry.postings.len() < 4 {
        return Err(LedgerError::InvalidTradePostings {
            expected: 4,
            actual: entry.postings.len(),
            reason: "Trade requires at least 4 postings (2 assets × 2 sides)",
        });
    }

    // Rule 2: LIAB accounts only
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Trade",
                account: posting.account.to_string(),
                reason: "Trade only allows LIAB accounts",
            });
        }
    }

    // Rule 3: Exactly 2 assets
    let assets = collect_assets(&entry.postings);
    if assets.len() != 2 {
        return Err(LedgerError::InvalidTradeAssets {
            expected: 2,
            actual: assets.len(),
            assets: assets.into_iter().collect(),
        });
    }

    // Rule 4: Zero-sum per asset (already checked in validate_balance)
    // Rule 5: At least 2 users (implicit from 4 postings with different accounts)

    Ok(())
}

/// Fee: LIAB ↓, REV ↑
pub fn validate_fee(entry: &UnsignedEntry) -> ValidationResult {
    let has_liab_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Debit
    });
    let has_rev_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Revenue && p.side == Side::Credit
    });

    if !has_liab_debit || !has_rev_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Fee",
            account: String::new(),
            reason: "Fee requires LIAB debit and REV credit",
        });
    }

    // All postings must be either LIAB debit or REV credit
    for posting in &entry.postings {
        let valid = match (&posting.account.category, &posting.side) {
            (AccountCategory::Liability, Side::Debit) => true,
            (AccountCategory::Revenue, Side::Credit) => true,
            _ => false,
        };
        if !valid {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Fee",
                account: posting.account.to_string(),
                reason: "Fee only allows LIAB debit or REV credit",
            });
        }
    }

    Ok(())
}

/// Adjustment: Any accounts (audit-heavy)
fn validate_adjustment(_entry: &UnsignedEntry) -> ValidationResult {
    // Adjustment allows any accounts but requires approval flag
    // Approval is checked at RPC layer
    Ok(())
}

// === Phase 3: Margin Trading Validation ===

/// Borrow: ASSET:LOAN ↑ (debit), LIAB:AVAILABLE ↑ (credit)
/// User borrows funds - BiBank's receivable increases, User's balance increases
fn validate_borrow(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Borrow",
            account: String::new(),
            reason: "Borrow requires at least 2 postings",
        });
    }

    // Must have ASSET:*:*:LOAN debit (BiBank's receivable increases)
    let has_loan_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset
            && p.account.sub_account == "LOAN"
            && p.side == Side::Debit
    });

    // Must have LIAB:*:*:AVAILABLE credit (User's balance increases)
    let has_avail_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "AVAILABLE"
            && p.side == Side::Credit
    });

    if !has_loan_debit || !has_avail_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Borrow",
            account: String::new(),
            reason: "Borrow requires ASSET:*:LOAN debit and LIAB:*:AVAILABLE credit",
        });
    }

    // All postings must be ASSET:LOAN or LIAB:AVAILABLE
    for posting in &entry.postings {
        let valid = match (&posting.account.category, posting.account.sub_account.as_str()) {
            (AccountCategory::Asset, "LOAN") => true,
            (AccountCategory::Liability, "AVAILABLE") => true,
            _ => false,
        };
        if !valid {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Borrow",
                account: posting.account.to_string(),
                reason: "Borrow only allows ASSET:LOAN or LIAB:AVAILABLE accounts",
            });
        }
    }

    Ok(())
}

/// Repay: LIAB:AVAILABLE ↓ (debit), ASSET:LOAN ↓ (credit)
/// User repays loan - User's balance decreases, BiBank's receivable decreases
fn validate_repay(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Repay",
            account: String::new(),
            reason: "Repay requires at least 2 postings",
        });
    }

    // Must have LIAB:*:*:AVAILABLE debit (User pays)
    let has_avail_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "AVAILABLE"
            && p.side == Side::Debit
    });

    // Must have ASSET:*:*:LOAN credit (Loan decreases)
    let has_loan_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset
            && p.account.sub_account == "LOAN"
            && p.side == Side::Credit
    });

    if !has_avail_debit || !has_loan_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Repay",
            account: String::new(),
            reason: "Repay requires LIAB:*:AVAILABLE debit and ASSET:*:LOAN credit",
        });
    }

    Ok(())
}

/// Interest: ASSET:LOAN ↑ (debit), REV:INTEREST ↑ (credit)
/// Interest accrual - Loan increases, Revenue increases (compound interest)
fn validate_interest(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Interest",
            account: String::new(),
            reason: "Interest requires at least 2 postings",
        });
    }

    // Must have ASSET:*:*:LOAN debit (Loan principal increases)
    let has_loan_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset
            && p.account.sub_account == "LOAN"
            && p.side == Side::Debit
    });

    // Must have REV:*:INTEREST:*:* credit (Revenue increases)
    let has_rev_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Revenue && p.side == Side::Credit
    });

    if !has_loan_debit || !has_rev_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Interest",
            account: String::new(),
            reason: "Interest requires ASSET:LOAN debit and REV credit",
        });
    }

    Ok(())
}

/// Liquidation: Multiple accounts - close position, settle loan
/// Complex validation - at least 4 postings (position close + settlement)
fn validate_liquidation(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 4 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Liquidation",
            account: String::new(),
            reason: "Liquidation requires at least 4 postings",
        });
    }

    // Liquidation can involve LIAB (user balance), ASSET (loan), EQUITY (insurance)
    // We don't restrict categories heavily for liquidation - it's a complex operation
    Ok(())
}

/// OrderPlace: LIAB:AVAILABLE ↓, LIAB:LOCKED ↑
/// Lock funds for order
fn validate_order_place(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "OrderPlace",
            account: String::new(),
            reason: "OrderPlace requires at least 2 postings",
        });
    }

    // Must have LIAB:*:*:AVAILABLE debit (funds leave available)
    let has_avail_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "AVAILABLE"
            && p.side == Side::Debit
    });

    // Must have LIAB:*:*:LOCKED credit (funds go to locked)
    let has_locked_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "LOCKED"
            && p.side == Side::Credit
    });

    if !has_avail_debit || !has_locked_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "OrderPlace",
            account: String::new(),
            reason: "OrderPlace requires LIAB:AVAILABLE debit and LIAB:LOCKED credit",
        });
    }

    // All postings must be LIAB
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "OrderPlace",
                account: posting.account.to_string(),
                reason: "OrderPlace only allows LIAB accounts",
            });
        }
    }

    Ok(())
}

/// OrderCancel: LIAB:LOCKED ↓, LIAB:AVAILABLE ↑
/// Unlock funds from cancelled order
fn validate_order_cancel(entry: &UnsignedEntry) -> ValidationResult {
    if entry.postings.len() < 2 {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "OrderCancel",
            account: String::new(),
            reason: "OrderCancel requires at least 2 postings",
        });
    }

    // Must have LIAB:*:*:LOCKED debit (funds leave locked)
    let has_locked_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "LOCKED"
            && p.side == Side::Debit
    });

    // Must have LIAB:*:*:AVAILABLE credit (funds return to available)
    let has_avail_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability
            && p.account.sub_account == "AVAILABLE"
            && p.side == Side::Credit
    });

    if !has_locked_debit || !has_avail_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "OrderCancel",
            account: String::new(),
            reason: "OrderCancel requires LIAB:LOCKED debit and LIAB:AVAILABLE credit",
        });
    }

    // All postings must be LIAB
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "OrderCancel",
                account: posting.account.to_string(),
                reason: "OrderCancel only allows LIAB accounts",
            });
        }
    }

    Ok(())
}

/// Collect unique assets from postings
fn collect_assets(postings: &[Posting]) -> std::collections::HashSet<String> {
    postings.iter().map(|p| p.account.asset.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountKey;
    use bibank_core::Amount;
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    #[test]
    fn test_validate_trade_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // USDT leg
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                // BTC leg
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_trade(&entry).is_ok());
    }

    #[test]
    fn test_validate_trade_insufficient_postings() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidTradePostings { .. })));
    }

    #[test]
    fn test_validate_trade_wrong_asset_count() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Only USDT, no second asset
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(50)),
                Posting::debit(AccountKey::user_available("CHARLIE", "USDT"), amount(50)),
                Posting::credit(AccountKey::user_available("DAVE", "USDT"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidTradeAssets { .. })));
    }

    #[test]
    fn test_validate_trade_non_liab_account() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // ASSET account not allowed in Trade
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { .. })));
    }

    #[test]
    fn test_validate_fee_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Fee,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1)),
                Posting::credit(AccountKey::fee_revenue("USDT"), amount(1)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_fee(&entry).is_ok());
    }

    #[test]
    fn test_validate_fee_wrong_direction() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Fee,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Wrong: LIAB credit instead of debit
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1)),
                Posting::debit(AccountKey::fee_revenue("USDT"), amount(1)),
            ],
            metadata: Default::default(),
        };

        let result = validate_fee(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { .. })));
    }

    // === Phase 3: Borrow/Repay Tests ===

    fn loan_account(user: &str, asset: &str) -> AccountKey {
        AccountKey::new(AccountCategory::Asset, "USER", user, asset, "LOAN")
    }

    #[test]
    fn test_validate_borrow_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Borrow,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // BiBank's receivable increases (ASSET:LOAN debit)
                Posting::debit(loan_account("ALICE", "USDT"), amount(1000)),
                // User's balance increases (LIAB:AVAILABLE credit)
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_borrow(&entry).is_ok());
    }

    #[test]
    fn test_validate_borrow_missing_loan() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Borrow,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Missing ASSET:LOAN account
                Posting::debit(AccountKey::user_available("BOB", "USDT"), amount(1000)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        let result = validate_borrow(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Borrow", .. })));
    }

    #[test]
    fn test_validate_borrow_wrong_account_type() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Borrow,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(loan_account("ALICE", "USDT"), amount(1000)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
                // Not allowed: EQUITY account in Borrow
                Posting::debit(AccountKey::new(AccountCategory::Equity, "SYSTEM", "INSURANCE", "USDT", "FUND"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_borrow(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Borrow", .. })));
    }

    #[test]
    fn test_validate_repay_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Repay,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // User's balance decreases (LIAB:AVAILABLE debit)
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(500)),
                // BiBank's receivable decreases (ASSET:LOAN credit)
                Posting::credit(loan_account("ALICE", "USDT"), amount(500)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_repay(&entry).is_ok());
    }

    #[test]
    fn test_validate_repay_missing_loan_credit() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Repay,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Has LIAB debit but missing ASSET:LOAN credit
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(500)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(500)),
            ],
            metadata: Default::default(),
        };

        let result = validate_repay(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Repay", .. })));
    }

    // === Phase 3: Interest Tests ===

    fn interest_revenue(asset: &str) -> AccountKey {
        AccountKey::new(AccountCategory::Revenue, "SYSTEM", "INTEREST", asset, "INCOME")
    }

    #[test]
    fn test_validate_interest_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Interest,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Loan increases (compound interest)
                Posting::debit(loan_account("ALICE", "USDT"), amount(5)),
                // Revenue increases
                Posting::credit(interest_revenue("USDT"), amount(5)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_interest(&entry).is_ok());
    }

    #[test]
    fn test_validate_interest_missing_revenue() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Interest,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(loan_account("ALICE", "USDT"), amount(5)),
                // Wrong: No REV credit
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(5)),
            ],
            metadata: Default::default(),
        };

        let result = validate_interest(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Interest", .. })));
    }

    // === Phase 3: Liquidation Tests ===

    #[test]
    fn test_validate_liquidation_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Liquidation,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Close user's position - user loses BTC
                Posting::debit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
                Posting::credit(AccountKey::system_vault("BTC"), amount(1)),
                // Settle loan - loan cleared
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(500)),
                Posting::credit(loan_account("ALICE", "USDT"), amount(500)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_liquidation(&entry).is_ok());
    }

    #[test]
    fn test_validate_liquidation_insufficient_postings() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Liquidation,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Only 2 postings - not enough for liquidation
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(500)),
                Posting::credit(loan_account("ALICE", "USDT"), amount(500)),
            ],
            metadata: Default::default(),
        };

        let result = validate_liquidation(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "Liquidation", .. })));
    }

    // === Phase 3: OrderPlace/OrderCancel Tests ===

    fn locked_account(user: &str, asset: &str) -> AccountKey {
        AccountKey::user_locked(user, asset)
    }

    #[test]
    fn test_validate_order_place_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderPlace,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Funds leave AVAILABLE
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
                // Funds go to LOCKED
                Posting::credit(locked_account("ALICE", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_order_place(&entry).is_ok());
    }

    #[test]
    fn test_validate_order_place_missing_locked() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderPlace,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Has AVAILABLE debit but no LOCKED credit
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        let result = validate_order_place(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "OrderPlace", .. })));
    }

    #[test]
    fn test_validate_order_place_non_liab_account() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderPlace,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
                Posting::credit(locked_account("ALICE", "USDT"), amount(1000)),
                // Not allowed: ASSET account in OrderPlace
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_order_place(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "OrderPlace", .. })));
    }

    #[test]
    fn test_validate_order_cancel_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderCancel,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Funds leave LOCKED
                Posting::debit(locked_account("ALICE", "USDT"), amount(1000)),
                // Funds return to AVAILABLE
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_order_cancel(&entry).is_ok());
    }

    #[test]
    fn test_validate_order_cancel_missing_available_credit() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::OrderCancel,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Has LOCKED debit but wrong credit account
                Posting::debit(locked_account("ALICE", "USDT"), amount(1000)),
                Posting::credit(locked_account("BOB", "USDT"), amount(1000)),
            ],
            metadata: Default::default(),
        };

        let result = validate_order_cancel(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { intent: "OrderCancel", .. })));
    }
}
