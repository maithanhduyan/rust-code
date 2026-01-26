//! Interest Accrual Module
//!
//! Handles compound interest calculation for margin loans.
//! Interest is accrued daily and added to the loan principal.

use bibank_core::Amount;
use bibank_ledger::{AccountKey, Posting, TransactionIntent, UnsignedEntry};
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;

use crate::state::RiskState;

/// Default interest rate (0.05% daily = ~18.25% APR)
pub const DEFAULT_DAILY_RATE: Decimal = Decimal::from_parts(5, 0, 0, false, 4); // 0.0005

/// Interest accrual calculator
pub struct InterestCalculator {
    /// Daily interest rate (e.g., 0.0005 for 0.05%)
    daily_rate: Decimal,
}

impl InterestCalculator {
    /// Create a new calculator with default rate
    pub fn new() -> Self {
        Self {
            daily_rate: DEFAULT_DAILY_RATE,
        }
    }

    /// Create with custom daily rate
    pub fn with_rate(daily_rate: Decimal) -> Self {
        Self { daily_rate }
    }

    /// Calculate interest for a single loan
    pub fn calculate_interest(&self, principal: Decimal) -> Decimal {
        principal * self.daily_rate
    }

    /// Generate interest entries for all active loans
    ///
    /// Returns a list of UnsignedEntry for each user/asset with a loan.
    /// Each entry has:
    /// - Debit: ASSET:USER:*:*:LOAN (loan increases)
    /// - Credit: REV:SYSTEM:INTEREST:*:INCOME (revenue increases)
    pub fn generate_interest_entries(
        &self,
        state: &RiskState,
        correlation_prefix: &str,
    ) -> Vec<UnsignedEntry> {
        let mut entries = Vec::new();

        // Scan all balances for LOAN accounts
        for (account_key, &balance) in state.all_balances() {
            // Only process ASSET:USER:*:*:LOAN with positive balance
            if account_key.starts_with("ASSET:USER:")
                && account_key.ends_with(":LOAN")
                && balance > Decimal::ZERO
            {
                // Parse: ASSET:USER:alice:USDT:LOAN
                let parts: Vec<&str> = account_key.split(':').collect();
                if parts.len() == 5 {
                    let user = parts[2];
                    let asset = parts[3];

                    let interest = self.calculate_interest(balance);
                    if interest > Decimal::ZERO {
                        if let Some(entry) = self.create_interest_entry(
                            user,
                            asset,
                            interest,
                            &format!("{}-{}-{}", correlation_prefix, user, asset),
                        ) {
                            entries.push(entry);
                        }
                    }
                }
            }
        }

        entries
    }

    /// Create an interest entry for a single user/asset
    fn create_interest_entry(
        &self,
        user: &str,
        asset: &str,
        interest: Decimal,
        correlation_id: &str,
    ) -> Option<UnsignedEntry> {
        let amount = Amount::new(interest).ok()?;

        // LOAN account (BiBank's receivable increases)
        let loan_account = RiskState::loan_account(user, asset);

        // Revenue account (BiBank's income increases)
        let revenue_account = AccountKey::new(
            bibank_ledger::AccountCategory::Revenue,
            "SYSTEM",
            "INTEREST",
            asset,
            "INCOME",
        );

        let mut metadata = HashMap::new();
        metadata.insert("interest_rate".to_string(), json!(self.daily_rate.to_string()));
        metadata.insert("principal".to_string(), json!(interest.to_string()));
        metadata.insert("accrual_type".to_string(), json!("compound"));

        Some(UnsignedEntry {
            intent: TransactionIntent::Interest,
            correlation_id: correlation_id.to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(loan_account, amount.clone()),
                Posting::credit(revenue_account, amount),
            ],
            metadata,
        })
    }

    /// Get the daily rate
    pub fn daily_rate(&self) -> Decimal {
        self.daily_rate
    }

    /// Get annualized rate (APR, simple approximation)
    pub fn annual_rate(&self) -> Decimal {
        self.daily_rate * Decimal::from(365)
    }
}

impl Default for InterestCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_ledger::{JournalEntry, Posting};
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
            signatures: Vec::new(),
        }
    }

    fn borrow_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Borrow,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(RiskState::loan_account(user, "USDT"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_calculate_interest_basic() {
        let calc = InterestCalculator::new();

        // 1000 USDT loan at 0.05% daily = 0.5 USDT interest
        let interest = calc.calculate_interest(Decimal::from(1000));
        assert_eq!(interest, Decimal::from_str_exact("0.5").unwrap());
    }

    #[test]
    fn test_calculate_interest_custom_rate() {
        // 0.1% daily rate
        let calc = InterestCalculator::with_rate(Decimal::from_str_exact("0.001").unwrap());

        // 1000 USDT loan at 0.1% daily = 1 USDT interest
        let interest = calc.calculate_interest(Decimal::from(1000));
        assert_eq!(interest, Decimal::from(1));
    }

    #[test]
    fn test_generate_interest_entries_single_user() {
        let mut state = RiskState::new();
        let calc = InterestCalculator::new();

        // Alice deposits 100 and borrows 500
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 500));

        // Generate interest entries
        let entries = calc.generate_interest_entries(&state, "daily-2025-01-01");

        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.intent, TransactionIntent::Interest);
        assert_eq!(entry.postings.len(), 2);

        // Interest = 500 * 0.0005 = 0.25 USDT
        assert_eq!(
            entry.postings[0].amount.value(),
            Decimal::from_str_exact("0.25").unwrap()
        );
    }

    #[test]
    fn test_generate_interest_entries_multiple_users() {
        let mut state = RiskState::new();
        let calc = InterestCalculator::new();

        // Alice borrows 1000
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 1000));

        // Bob borrows 2000
        state.apply_entry(&deposit_entry("BOB", 200));
        state.apply_entry(&borrow_entry("BOB", 2000));

        let entries = calc.generate_interest_entries(&state, "daily");

        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_no_interest_for_zero_loan() {
        let mut state = RiskState::new();
        let calc = InterestCalculator::new();

        // Alice only deposits, no loan
        state.apply_entry(&deposit_entry("ALICE", 100));

        let entries = calc.generate_interest_entries(&state, "daily");

        assert!(entries.is_empty());
    }

    #[test]
    fn test_compound_interest_simulation() {
        let mut state = RiskState::new();
        let calc = InterestCalculator::new();

        // Alice deposits 100 and borrows 1000
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 1000));

        // Day 1: Interest = 1000 * 0.0005 = 0.5
        let day1_entries = calc.generate_interest_entries(&state, "day1");
        assert_eq!(day1_entries.len(), 1);

        // Simulate applying interest (convert to JournalEntry)
        let day1_interest = JournalEntry {
            sequence: 10,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Interest,
            correlation_id: "day1".to_string(),
            causality_id: None,
            postings: day1_entries[0].postings.clone(),
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };
        state.apply_entry(&day1_interest);

        // Day 2: Loan is now 1000.5, Interest = 1000.5 * 0.0005 = 0.50025
        let loan_after = state.get_loan_balance("ALICE", "USDT");
        assert_eq!(loan_after, Decimal::from_str_exact("1000.5").unwrap());

        // Generate day 2 interest
        let day2_entries = calc.generate_interest_entries(&state, "day2");
        assert_eq!(
            day2_entries[0].postings[0].amount.value(),
            Decimal::from_str_exact("0.50025").unwrap()
        );
    }
}
