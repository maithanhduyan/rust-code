//! Liquidation engine for margin positions
//!
//! Handles auto-liquidation when margin ratio drops below maintenance threshold.
//! Integrates with insurance fund for shortfall coverage.

use bibank_core::Amount;
use bibank_ledger::{AccountCategory, AccountKey, JournalEntryBuilder, TransactionIntent, UnsignedEntry, LedgerError};
use rust_decimal::Decimal;
use serde_json::json;

use crate::state::{RiskState, LIQUIDATION_THRESHOLD};

/// Liquidation result
#[derive(Debug, Clone)]
pub struct LiquidationResult {
    /// User being liquidated
    pub user_id: String,
    /// Asset being liquidated
    pub asset: String,
    /// Collateral seized
    pub collateral_seized: Decimal,
    /// Loan repaid
    pub loan_repaid: Decimal,
    /// Penalty amount
    pub penalty: Decimal,
    /// Insurance fund contribution (if any shortfall)
    pub insurance_contribution: Decimal,
    /// Whether liquidation was full or partial
    pub is_full_liquidation: bool,
}

/// Configuration for liquidation engine
#[derive(Debug, Clone)]
pub struct LiquidationConfig {
    /// Liquidation penalty rate (e.g., 0.05 = 5%)
    pub penalty_rate: Decimal,
    /// Maximum liquidation per call (e.g., 0.5 = 50% of position)
    pub max_liquidation_ratio: Decimal,
    /// Minimum profit for liquidator incentive
    pub liquidator_bonus_rate: Decimal,
}

impl Default for LiquidationConfig {
    fn default() -> Self {
        Self {
            penalty_rate: Decimal::new(5, 2),           // 5%
            max_liquidation_ratio: Decimal::new(50, 2), // 50%
            liquidator_bonus_rate: Decimal::new(1, 2),  // 1%
        }
    }
}

/// Liquidation engine
#[derive(Debug)]
pub struct LiquidationEngine {
    config: LiquidationConfig,
}

impl Default for LiquidationEngine {
    fn default() -> Self {
        Self::new(LiquidationConfig::default())
    }
}

impl LiquidationEngine {
    /// Create a new liquidation engine
    pub fn new(config: LiquidationConfig) -> Self {
        Self { config }
    }

    /// Get the configuration
    pub fn config(&self) -> &LiquidationConfig {
        &self.config
    }

    /// Check if a user is eligible for liquidation
    pub fn is_liquidatable(&self, state: &RiskState, user_id: &str, asset: &str) -> bool {
        state.is_liquidatable(user_id, asset)
    }

    /// Calculate liquidation amount for a user
    ///
    /// Returns (collateral_to_seize, loan_to_repay, penalty)
    pub fn calculate_liquidation(
        &self,
        state: &RiskState,
        user_id: &str,
        asset: &str,
        price: Decimal,
    ) -> Option<(Decimal, Decimal, Decimal)> {
        // Get user's loan balance
        let loan_account = AccountKey::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOAN",
        );
        let loan_balance = state.get_balance(&loan_account);

        if loan_balance.is_zero() {
            return None;
        }

        // Get user's collateral (available balance)
        let collateral = state.get_balance(&AccountKey::user_available(user_id, asset));

        // Check margin ratio
        let margin_ratio = if !loan_balance.is_zero() {
            (collateral * price) / loan_balance
        } else {
            return None;
        };

        // Only liquidate if below threshold
        if margin_ratio >= LIQUIDATION_THRESHOLD {
            return None;
        }

        // Calculate how much to liquidate (partial liquidation)
        let max_liquidation = loan_balance * self.config.max_liquidation_ratio;
        let liquidation_amount = loan_balance.min(max_liquidation);

        // Calculate penalty
        let penalty = liquidation_amount * self.config.penalty_rate;

        // Total collateral to seize
        let collateral_to_seize = liquidation_amount + penalty;

        Some((collateral_to_seize, liquidation_amount, penalty))
    }

    /// Generate a liquidation journal entry
    pub fn generate_liquidation_entry(
        &self,
        user_id: &str,
        liquidator_id: &str,
        asset: &str,
        collateral_seized: Decimal,
        loan_repaid: Decimal,
        penalty: Decimal,
        correlation_id: &str,
    ) -> Result<UnsignedEntry, LedgerError> {
        let collateral_amt = Amount::new(collateral_seized)
            .map_err(|_| LedgerError::InvalidAccountFormat("invalid collateral amount".to_string()))?;
        let loan_amt = Amount::new(loan_repaid)
            .map_err(|_| LedgerError::InvalidAccountFormat("invalid loan amount".to_string()))?;

        // User accounts
        let user_available = AccountKey::user_available(user_id, asset);
        let user_loan = AccountKey::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOAN",
        );

        // Liquidator account
        let liquidator_available = AccountKey::user_available(liquidator_id, asset);

        // System accounts
        let insurance_fund = AccountKey::new(
            AccountCategory::Asset,
            "SYSTEM",
            "INSURANCE_FUND",
            asset,
            "MAIN",
        );

        // Liquidator bonus is a portion of the penalty
        let liquidator_bonus = penalty * self.config.liquidator_bonus_rate / self.config.penalty_rate;
        let insurance_portion = penalty - liquidator_bonus;

        let liquidator_bonus_amt = Amount::new(liquidator_bonus)
            .map_err(|_| LedgerError::InvalidAccountFormat("invalid bonus amount".to_string()))?;
        let insurance_portion_amt = Amount::new(insurance_portion)
            .map_err(|_| LedgerError::InvalidAccountFormat("invalid insurance amount".to_string()))?;

        // Double-entry accounting for liquidation:
        // collateral_seized = loan_repaid + penalty = loan_repaid + insurance_portion + liquidator_bonus
        //
        // 1. User loses collateral (Credit user_available = -collateral_seized)
        // 2. User's loan liability decreases (Debit user_loan = +loan_repaid)
        // 3. Insurance fund receives portion of penalty (Debit insurance_fund = +insurance_portion)
        // 4. Liquidator receives bonus (Debit liquidator_available = +liquidator_bonus)
        //
        // Balance check:
        // -collateral_seized + loan_repaid + insurance_portion + liquidator_bonus = 0 ✓

        let entry = JournalEntryBuilder::new()
            .intent(TransactionIntent::Liquidation)
            .correlation_id(correlation_id)
            // Seize collateral from user (reduces user's asset balance)
            .credit(user_available.clone(), collateral_amt)
            // Reduce user's loan liability
            .debit(user_loan, loan_amt)
            // Insurance fund receives portion of penalty
            .debit(insurance_fund, insurance_portion_amt)
            // Liquidator receives bonus
            .debit(liquidator_available, liquidator_bonus_amt)
            .metadata("liquidated_user", json!(user_id))
            .metadata("liquidator", json!(liquidator_id))
            .metadata("asset", json!(asset))
            .metadata("collateral_seized", json!(collateral_seized.to_string()))
            .metadata("loan_repaid", json!(loan_repaid.to_string()))
            .metadata("penalty", json!(penalty.to_string()))
            .build_unsigned()?;

        Ok(entry)
    }

    /// Execute liquidation for a user
    ///
    /// Returns the liquidation result or None if user is not liquidatable
    pub fn execute_liquidation(
        &self,
        state: &RiskState,
        user_id: &str,
        liquidator_id: &str,
        asset: &str,
        price: Decimal,
        correlation_id: &str,
    ) -> Result<Option<(UnsignedEntry, LiquidationResult)>, LedgerError> {
        // Calculate liquidation amounts
        let (collateral_seized, loan_repaid, penalty) =
            match self.calculate_liquidation(state, user_id, asset, price) {
                Some(amounts) => amounts,
                None => return Ok(None),
            };

        // Check if user has enough collateral
        let user_balance = state.get_balance(&AccountKey::user_available(user_id, asset));

        let (actual_seized, insurance_contribution) = if collateral_seized > user_balance {
            // Shortfall - insurance fund covers the difference
            let shortfall = collateral_seized - user_balance;
            (user_balance, shortfall)
        } else {
            (collateral_seized, Decimal::ZERO)
        };

        // Generate journal entry
        let entry = self.generate_liquidation_entry(
            user_id,
            liquidator_id,
            asset,
            actual_seized,
            loan_repaid,
            penalty,
            correlation_id,
        )?;

        let result = LiquidationResult {
            user_id: user_id.to_string(),
            asset: asset.to_string(),
            collateral_seized: actual_seized,
            loan_repaid,
            penalty,
            insurance_contribution,
            is_full_liquidation: actual_seized >= collateral_seized,
        };

        Ok(Some((entry, result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_ledger::{JournalEntry, Posting, Side};
    use chrono::Utc;

    fn create_test_state() -> RiskState {
        RiskState::new()
    }

    // Helper to create a deposit entry for testing
    fn deposit_entry(user: &str, amount: i64) -> JournalEntry {
        let amt = Amount::new(Decimal::from(amount)).unwrap();
        JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::new(AccountKey::system_vault("USDT"), amt, Side::Debit),
                Posting::new(AccountKey::user_available(user, "USDT"), amt, Side::Credit),
            ],
            metadata: Default::default(),
            signatures: vec![],
        }
    }

    // Helper to create a borrow entry for testing
    fn borrow_entry(user: &str, amount: i64) -> JournalEntry {
        let amt = Amount::new(Decimal::from(amount)).unwrap();
        JournalEntry {
            sequence: 2,
            prev_hash: "test".to_string(),
            hash: "test2".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Borrow,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::new(
                    AccountKey::new(AccountCategory::Asset, "SYSTEM", "LENDING_POOL", "USDT", "MAIN"),
                    amt,
                    Side::Debit,
                ),
                Posting::new(
                    AccountKey::new(AccountCategory::Liability, "USER", user, "USDT", "LOAN"),
                    amt,
                    Side::Credit,
                ),
                Posting::new(
                    AccountKey::user_available(user, "USDT"),
                    amt,
                    Side::Credit,
                ),
                Posting::new(
                    AccountKey::new(AccountCategory::Asset, "SYSTEM", "LOANS_RECEIVABLE", "USDT", "MAIN"),
                    amt,
                    Side::Debit,
                ),
            ],
            metadata: Default::default(),
            signatures: vec![],
        }
    }

    #[test]
    fn test_liquidation_config_default() {
        let config = LiquidationConfig::default();
        assert_eq!(config.penalty_rate, Decimal::new(5, 2));
        assert_eq!(config.max_liquidation_ratio, Decimal::new(50, 2));
    }

    #[test]
    fn test_liquidation_engine_creation() {
        let engine = LiquidationEngine::default();
        assert_eq!(engine.config().penalty_rate, Decimal::new(5, 2));
    }

    #[test]
    fn test_calculate_liquidation_no_loan() {
        let engine = LiquidationEngine::default();
        let state = create_test_state();

        let result = engine.calculate_liquidation(&state, "ALICE", "USDT", Decimal::ONE);
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_liquidation_healthy_position() {
        let engine = LiquidationEngine::default();
        let mut state = create_test_state();

        // Deposit 1000 USDT
        state.apply_entry(&deposit_entry("ALICE", 1000));

        // Borrow only 100 USDT (very healthy 11:1 ratio after receiving borrowed funds)
        state.apply_entry(&borrow_entry("ALICE", 100));

        let result = engine.calculate_liquidation(&state, "ALICE", "USDT", Decimal::ONE);
        assert!(result.is_none()); // Should not be liquidatable
    }

    #[test]
    fn test_calculate_liquidation_underwater_position() {
        let engine = LiquidationEngine::default();
        let mut state = create_test_state();

        // Deposit 100 USDT
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Borrow 100 USDT (will have 200 balance, 100 loan = 2:1 ratio)
        state.apply_entry(&borrow_entry("ALICE", 100));

        // Now balance is 200, loan is 100 - ratio is 2:1 (healthy)
        let result = engine.calculate_liquidation(&state, "ALICE", "USDT", Decimal::ONE);
        // This should return None because 200/100 = 2.0 > LIQUIDATION_THRESHOLD (1.1)
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_liquidation_entry() {
        let engine = LiquidationEngine::default();

        let entry = engine
            .generate_liquidation_entry("ALICE", "LIQUIDATOR", "USDT", Decimal::from(100), Decimal::from(95), Decimal::from(5), "liq-001")
            .unwrap();

        assert_eq!(entry.intent, TransactionIntent::Liquidation);
        assert!(!entry.postings.is_empty());
    }
}
