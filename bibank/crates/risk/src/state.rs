//! In-memory risk state
//!
//! This state is rebuilt from ledger replay on startup.
//! It tracks balances for pre-commit validation.

use bibank_ledger::{AccountKey, JournalEntry, Posting};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// In-memory balance state for risk checking
#[derive(Debug, Default)]
pub struct RiskState {
    /// Balance per account (AccountKey string -> Decimal)
    /// Note: Can be negative during calculation, but post-check should be >= 0
    balances: HashMap<String, Decimal>,
}

impl RiskState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get balance for an account (returns 0 if not found)
    pub fn get_balance(&self, account: &AccountKey) -> Decimal {
        self.balances
            .get(&account.to_string())
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Apply a journal entry to update balances
    ///
    /// For liability accounts (user balances):
    /// - Credit increases balance (money owed to user)
    /// - Debit decreases balance (user spending)
    pub fn apply_entry(&mut self, entry: &JournalEntry) {
        for posting in &entry.postings {
            self.apply_posting(posting);
        }
    }

    /// Apply a single posting
    fn apply_posting(&mut self, posting: &Posting) {
        let key = posting.account.to_string();
        let current = self.balances.entry(key).or_insert(Decimal::ZERO);

        // For LIAB accounts: Credit = increase, Debit = decrease
        // For ASSET accounts: Debit = increase, Credit = decrease
        let normal_side = posting.account.category.normal_balance();

        let delta = if posting.side == normal_side {
            // Same as normal balance = increase
            posting.amount.value()
        } else {
            // Opposite = decrease
            -posting.amount.value()
        };

        *current += delta;
    }

    /// Calculate projected balances after applying an entry (without mutating state)
    pub fn project_balances(&self, entry: &JournalEntry) -> HashMap<String, Decimal> {
        let mut projected = self.balances.clone();

        for posting in &entry.postings {
            let key = posting.account.to_string();
            let current = projected.entry(key).or_insert(Decimal::ZERO);

            let normal_side = posting.account.category.normal_balance();
            let delta = if posting.side == normal_side {
                posting.amount.value()
            } else {
                -posting.amount.value()
            };

            *current += delta;
        }

        projected
    }

    /// Check if all LIAB accounts would have non-negative balance after applying entry
    pub fn check_sufficient_balance(&self, entry: &JournalEntry) -> Vec<(String, Decimal)> {
        let projected = self.project_balances(entry);
        let mut violations = Vec::new();

        for posting in &entry.postings {
            // Only check liability accounts (user balances)
            if posting.account.category == bibank_ledger::AccountCategory::Liability {
                let key = posting.account.to_string();
                if let Some(&balance) = projected.get(&key) {
                    if balance < Decimal::ZERO {
                        violations.push((key, balance));
                    }
                }
            }
        }

        violations
    }

    /// Get all account balances (for debugging/testing)
    pub fn all_balances(&self) -> &HashMap<String, Decimal> {
        &self.balances
    }

    /// Clear all state (for testing)
    pub fn clear(&mut self) {
        self.balances.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_core::Amount;
    use bibank_ledger::TransactionIntent;
    use chrono::Utc;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    fn deposit_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_deposit_increases_user_balance() {
        let mut state = RiskState::new();
        let entry = deposit_entry("ALICE", 100);

        state.apply_entry(&entry);

        let alice_balance = state.get_balance(&AccountKey::user_available("ALICE", "USDT"));
        assert_eq!(alice_balance, Decimal::new(100, 0));
    }

    #[test]
    fn test_transfer_balance_check() {
        let mut state = RiskState::new();

        // Alice deposits 100
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Alice tries to transfer 150 to Bob
        let transfer = JournalEntry {
            sequence: 2,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Transfer,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(150)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(150)),
            ],
            metadata: HashMap::new(),
        };

        let violations = state.check_sufficient_balance(&transfer);
        assert!(!violations.is_empty());
        assert!(violations[0].0.contains("ALICE"));
    }

    #[test]
    fn test_valid_transfer() {
        let mut state = RiskState::new();

        // Alice deposits 100
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Alice transfers 50 to Bob
        let transfer = JournalEntry {
            sequence: 2,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Transfer,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(50)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(50)),
            ],
            metadata: HashMap::new(),
        };

        let violations = state.check_sufficient_balance(&transfer);
        assert!(violations.is_empty());
    }
}
