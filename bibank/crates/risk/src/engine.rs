//! Risk engine implementation

use crate::error::RiskError;
use crate::state::RiskState;
use bibank_ledger::{JournalEntry, UnsignedEntry};

/// Risk Engine - Pre-commit gatekeeper
///
/// Validates transactions before they are committed to the ledger.
/// Maintains in-memory state rebuilt from event replay.
pub struct RiskEngine {
    state: RiskState,
}

impl RiskEngine {
    /// Create a new empty risk engine
    pub fn new() -> Self {
        Self {
            state: RiskState::new(),
        }
    }

    /// Get reference to internal state
    pub fn state(&self) -> &RiskState {
        &self.state
    }

    /// Get mutable reference to internal state
    pub fn state_mut(&mut self) -> &mut RiskState {
        &mut self.state
    }

    /// Check if an unsigned entry passes all risk checks
    ///
    /// Returns Ok(()) if the entry is allowed, Err otherwise.
    pub fn check(&self, entry: &UnsignedEntry) -> Result<(), RiskError> {
        // Build a temporary JournalEntry for checking
        let temp_entry = JournalEntry {
            sequence: 0,
            prev_hash: String::new(),
            hash: String::new(),
            timestamp: chrono::Utc::now(),
            intent: entry.intent,
            correlation_id: entry.correlation_id.clone(),
            causality_id: entry.causality_id.clone(),
            postings: entry.postings.clone(),
            metadata: entry.metadata.clone(),
        };

        self.check_entry(&temp_entry)
    }

    /// Check if a journal entry passes all risk checks
    pub fn check_entry(&self, entry: &JournalEntry) -> Result<(), RiskError> {
        // Rule 1: Check sufficient balance for liability accounts
        let violations = self.state.check_sufficient_balance(entry);

        if let Some((account, _balance)) = violations.first() {
            // Find the required amount from postings
            let required = entry
                .postings
                .iter()
                .find(|p| p.account.to_string() == *account)
                .map(|p| p.amount.value())
                .unwrap_or_default();

            let available = self.state.get_balance(
                &account
                    .parse()
                    .map_err(|_| RiskError::AccountNotFound(account.clone()))?,
            );

            return Err(RiskError::InsufficientBalance {
                account: account.clone(),
                available: available.to_string(),
                required: required.to_string(),
            });
        }

        Ok(())
    }

    /// Apply a committed entry to update internal state
    ///
    /// This should be called AFTER the entry is committed to ledger.
    pub fn apply(&mut self, entry: &JournalEntry) {
        self.state.apply_entry(entry);
    }

    /// Rebuild state from a sequence of entries (replay)
    pub fn replay<'a>(&mut self, entries: impl Iterator<Item = &'a JournalEntry>) {
        self.state.clear();
        for entry in entries {
            self.state.apply_entry(entry);
        }
    }
}

impl Default for RiskEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_core::Amount;
    use bibank_ledger::{AccountKey, JournalEntryBuilder, TransactionIntent};
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    #[test]
    fn test_deposit_always_allowed() {
        let engine = RiskEngine::new();

        let entry = JournalEntryBuilder::new()
            .intent(TransactionIntent::Deposit)
            .correlation_id("test-1")
            .debit(AccountKey::system_vault("USDT"), amount(100))
            .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .build_unsigned()
            .unwrap();

        assert!(engine.check(&entry).is_ok());
    }

    #[test]
    fn test_withdrawal_blocked_on_insufficient_balance() {
        let engine = RiskEngine::new();

        // No prior deposits, try to withdraw
        let entry = JournalEntryBuilder::new()
            .intent(TransactionIntent::Withdrawal)
            .correlation_id("test-1")
            .debit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .credit(AccountKey::system_vault("USDT"), amount(100))
            .build_unsigned()
            .unwrap();

        let result = engine.check(&entry);
        assert!(matches!(result, Err(RiskError::InsufficientBalance { .. })));
    }
}
