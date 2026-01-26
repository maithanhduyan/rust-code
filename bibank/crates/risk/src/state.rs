//! In-memory risk state
//!
//! This state is rebuilt from ledger replay on startup.
//! It tracks balances for pre-commit validation.

use bibank_ledger::{AccountCategory, AccountKey, JournalEntry, Posting};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Constants for margin trading
pub const MAX_LEVERAGE: Decimal = Decimal::from_parts(10, 0, 0, false, 0); // 10x
pub const MAINTENANCE_MARGIN: Decimal = Decimal::from_parts(5, 0, 0, false, 2); // 5%
pub const INITIAL_MARGIN: Decimal = Decimal::from_parts(10, 0, 0, false, 2); // 10%
pub const LIQUIDATION_THRESHOLD: Decimal = Decimal::from_parts(1, 0, 0, false, 0); // Ratio < 1.0

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

    // === Phase 3: Margin Trading Methods ===

    /// Create a LOAN account key for a user
    pub fn loan_account(user: &str, asset: &str) -> AccountKey {
        AccountKey::new(AccountCategory::Asset, "USER", user, asset, "LOAN")
    }

    /// Get the loan balance for a user/asset
    /// LOAN is stored in ASSET:USER:*:*:LOAN
    pub fn get_loan_balance(&self, user: &str, asset: &str) -> Decimal {
        let account = Self::loan_account(user, asset);
        self.get_balance(&account)
    }

    /// Get available balance for a user/asset
    pub fn get_available_balance(&self, user: &str, asset: &str) -> Decimal {
        let account = AccountKey::user_available(user, asset);
        self.get_balance(&account)
    }

    /// Calculate equity for a user/asset
    /// Equity = Available - Loan
    /// Note: This is simplified - real equity would include unrealized PnL
    pub fn get_equity(&self, user: &str, asset: &str) -> Decimal {
        let available = self.get_available_balance(user, asset);
        let loan = self.get_loan_balance(user, asset);
        available - loan
    }

    /// Calculate margin ratio for a user/asset
    /// Margin Ratio = (Available / Loan) if Loan > 0, else infinity (represented as 100.0)
    pub fn get_margin_ratio(&self, user: &str, asset: &str) -> Decimal {
        let available = self.get_available_balance(user, asset);
        let loan = self.get_loan_balance(user, asset);

        if loan <= Decimal::ZERO {
            // No loan = infinite margin (represented as 100.0)
            Decimal::from(100)
        } else {
            available / loan
        }
    }

    /// Check if a borrow would exceed max leverage
    /// Returns true if the borrow is allowed
    pub fn check_borrow_allowed(
        &self,
        user: &str,
        asset: &str,
        borrow_amount: Decimal,
    ) -> Result<(), MarginError> {
        let current_available = self.get_available_balance(user, asset);
        let current_loan = self.get_loan_balance(user, asset);

        // After borrow: available += borrow_amount, loan += borrow_amount
        let new_loan = current_loan + borrow_amount;

        // When you borrow, your available goes up, your loan goes up by same amount
        // So your equity (Available - Loan) stays the same
        // The constraint is: Equity >= Loan * Initial_Margin
        // Or: Equity / Loan >= Initial_Margin
        // Or: current_available / new_loan >= Initial_Margin

        let equity = current_available; // Equity doesn't change after borrow
        if new_loan > Decimal::ZERO {
            let margin_ratio = equity / new_loan;
            if margin_ratio < INITIAL_MARGIN {
                return Err(MarginError::ExceedsMaxLeverage {
                    requested: borrow_amount.to_string(),
                    max_allowed: (equity / INITIAL_MARGIN - current_loan).max(Decimal::ZERO).to_string(),
                    current_margin_ratio: margin_ratio.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Check if a user is subject to liquidation
    /// Returns true if margin ratio < 1.0
    pub fn is_liquidatable(&self, user: &str, asset: &str) -> bool {
        let margin_ratio = self.get_margin_ratio(user, asset);
        margin_ratio < LIQUIDATION_THRESHOLD
    }

    /// Get all users with positions that may need liquidation
    /// Scans all LOAN accounts and checks margin ratio
    pub fn get_liquidatable_positions(&self) -> Vec<(String, String, Decimal)> {
        let mut positions = Vec::new();

        for (key, &balance) in &self.balances {
            // Only check ASSET:USER:*:*:LOAN accounts with positive balance
            if key.starts_with("ASSET:USER:") && key.ends_with(":LOAN") && balance > Decimal::ZERO {
                // Parse: ASSET:USER:alice:USDT:LOAN
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() == 5 {
                    let user = parts[2];
                    let asset = parts[3];
                    if self.is_liquidatable(user, asset) {
                        let margin_ratio = self.get_margin_ratio(user, asset);
                        positions.push((user.to_string(), asset.to_string(), margin_ratio));
                    }
                }
            }
        }

        positions
    }
}

/// Margin-related errors
#[derive(Debug, Clone)]
pub enum MarginError {
    ExceedsMaxLeverage {
        requested: String,
        max_allowed: String,
        current_margin_ratio: String,
    },
    InsufficientMargin {
        user: String,
        asset: String,
        current_ratio: String,
    },
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
            signatures: Vec::new(),
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
            signatures: Vec::new(),
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
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&transfer);
        assert!(violations.is_empty());
    }

    // === Phase 2: Trade tests ===

    fn deposit_btc_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("BTC"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "BTC"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_trade_balance_check_both_users() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT
        state.apply_entry(&deposit_entry("ALICE", 100));
        // Bob deposits 1 BTC
        state.apply_entry(&deposit_btc_entry("BOB", 1));

        // Trade: Alice sells 100 USDT for 1 BTC
        let trade = JournalEntry {
            sequence: 3,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Trade,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                // USDT leg
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                // BTC leg
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&trade);
        assert!(violations.is_empty(), "Valid trade should pass risk check");
    }

    #[test]
    fn test_trade_insufficient_balance_seller() {
        let mut state = RiskState::new();

        // Alice only has 50 USDT
        state.apply_entry(&deposit_entry("ALICE", 50));
        // Bob deposits 1 BTC
        state.apply_entry(&deposit_btc_entry("BOB", 1));

        // Trade: Alice tries to sell 100 USDT (insufficient)
        let trade = JournalEntry {
            sequence: 3,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Trade,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&trade);
        assert!(!violations.is_empty(), "Trade should fail - Alice has insufficient USDT");
        assert!(violations.iter().any(|(acc, _)| acc.contains("ALICE")));
    }

    // === Phase 3: Margin Trading tests ===

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
                // ASSET:LOAN increases (BiBank's receivable)
                Posting::debit(RiskState::loan_account(user, "USDT"), amount(val)),
                // LIAB:AVAILABLE increases (User's balance)
                Posting::credit(AccountKey::user_available(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    fn repay_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Repay,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                // LIAB:AVAILABLE decreases (User pays)
                Posting::debit(AccountKey::user_available(user, "USDT"), amount(val)),
                // ASSET:LOAN decreases (BiBank's receivable)
                Posting::credit(RiskState::loan_account(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_borrow_updates_loan_balance() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT (equity)
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Alice borrows 500 USDT (5x leverage, within 10x limit)
        state.apply_entry(&borrow_entry("ALICE", 500));

        // Check balances
        let available = state.get_available_balance("ALICE", "USDT");
        let loan = state.get_loan_balance("ALICE", "USDT");
        let equity = state.get_equity("ALICE", "USDT");

        assert_eq!(available, Decimal::new(600, 0), "Available = 100 + 500");
        assert_eq!(loan, Decimal::new(500, 0), "Loan = 500");
        assert_eq!(equity, Decimal::new(100, 0), "Equity = Available - Loan = 100");
    }

    #[test]
    fn test_repay_reduces_loan_balance() {
        let mut state = RiskState::new();

        // Setup: Alice deposits 100, borrows 500
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 500));

        // Alice repays 200
        state.apply_entry(&repay_entry("ALICE", 200));

        let available = state.get_available_balance("ALICE", "USDT");
        let loan = state.get_loan_balance("ALICE", "USDT");

        assert_eq!(available, Decimal::new(400, 0), "Available = 600 - 200");
        assert_eq!(loan, Decimal::new(300, 0), "Loan = 500 - 200");
    }

    #[test]
    fn test_margin_ratio_calculation() {
        let mut state = RiskState::new();

        // Alice deposits 100, borrows 400
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 400));

        // Available = 500, Loan = 400
        // Margin Ratio = Available / Loan = 500/400 = 1.25
        let ratio = state.get_margin_ratio("ALICE", "USDT");
        assert_eq!(ratio, Decimal::new(125, 2));
    }

    #[test]
    fn test_margin_ratio_no_loan() {
        let mut state = RiskState::new();

        // Alice only deposits, no loan
        state.apply_entry(&deposit_entry("ALICE", 100));

        // No loan = infinite margin (represented as 100.0)
        let ratio = state.get_margin_ratio("ALICE", "USDT");
        assert_eq!(ratio, Decimal::from(100));
    }

    #[test]
    fn test_borrow_allowed_within_limit() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT (equity)
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Max borrow at 10x leverage = 100 / 0.10 = 1000
        // Try to borrow 900 (within limit)
        let result = state.check_borrow_allowed("ALICE", "USDT", Decimal::new(900, 0));
        assert!(result.is_ok(), "Should allow borrow within limit");
    }

    #[test]
    fn test_borrow_rejected_exceeds_leverage() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT (equity)
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Max borrow = 100 / 0.10 = 1000
        // Try to borrow 1100 (exceeds limit)
        let result = state.check_borrow_allowed("ALICE", "USDT", Decimal::new(1100, 0));
        assert!(matches!(result, Err(MarginError::ExceedsMaxLeverage { .. })));
    }

    #[test]
    fn test_is_liquidatable() {
        let mut state = RiskState::new();

        // Alice deposits 100, borrows 900
        state.apply_entry(&deposit_entry("ALICE", 100));
        state.apply_entry(&borrow_entry("ALICE", 900));

        // Available = 1000, Loan = 900
        // Margin Ratio = 1000/900 = 1.11 > 1.0 (safe)
        assert!(!state.is_liquidatable("ALICE", "USDT"));

        // Simulate loss: Alice loses 200 (Available drops to 800)
        // We can simulate by doing a transfer out
        let loss_entry = JournalEntry {
            sequence: 99,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Transfer,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(200)),
                Posting::credit(AccountKey::user_available("SYSTEM", "USDT"), amount(200)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };
        state.apply_entry(&loss_entry);

        // Now: Available = 800, Loan = 900
        // Margin Ratio = 800/900 = 0.889 < 1.0 (liquidatable!)
        assert!(state.is_liquidatable("ALICE", "USDT"));
    }
}
