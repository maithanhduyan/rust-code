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
}
