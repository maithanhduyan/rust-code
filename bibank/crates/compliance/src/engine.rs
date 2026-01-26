//! Compliance Engine - Main orchestrator
//!
//! Coordinates rule evaluation, decision aggregation, and ledger writes.

use chrono::Utc;
use rust_decimal::Decimal;

use crate::config::ComplianceConfig;
use crate::decision::{AmlDecision, ApprovalLevel, RiskScore};
use crate::error::ComplianceResult;
use crate::event::{ComplianceEvent, ReviewDecision};
use crate::ledger::ComplianceLedger;
use crate::state::ComplianceState;

/// Transaction context for rule evaluation
#[derive(Debug, Clone)]
pub struct TransactionContext {
    /// Unique transaction ID
    pub correlation_id: String,
    /// User initiating the transaction
    pub user_id: String,
    /// Transaction type
    pub intent: String,
    /// Amount
    pub amount: Decimal,
    /// Asset (e.g., "USDT", "BTC")
    pub asset: String,
    /// Account age in days (if known)
    pub account_age_days: Option<i64>,
}

/// Result of a compliance check
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// The final aggregated decision
    pub decision: AmlDecision,
    /// Rules that were triggered
    pub rules_triggered: Vec<String>,
    /// Risk score (if any)
    pub risk_score: Option<RiskScore>,
}

/// Main Compliance Engine
///
/// Orchestrates:
/// - Rule evaluation
/// - Decision aggregation (lattice max)
/// - Compliance Ledger writes
/// - In-memory state updates
pub struct ComplianceEngine {
    /// Configuration (thresholds, etc.)
    config: ComplianceConfig,
    /// Compliance Ledger (append-only)
    ledger: ComplianceLedger,
    /// In-memory state (sliding window)
    state: ComplianceState,
}

impl ComplianceEngine {
    /// Create a new Compliance Engine
    pub fn new(config: ComplianceConfig, ledger: ComplianceLedger) -> Self {
        Self {
            config,
            ledger,
            state: ComplianceState::new(),
        }
    }

    /// Create an engine with in-memory ledger (for testing)
    pub fn in_memory() -> Self {
        Self::new(ComplianceConfig::default(), ComplianceLedger::in_memory())
    }

    /// Check a transaction against all rules
    pub fn check_transaction(&mut self, ctx: &TransactionContext) -> ComplianceResult<CheckResult> {
        let mut decisions = Vec::new();
        let mut rules_triggered = Vec::new();

        // === BLOCK Rules (Pre-commit) ===
        // These would reject the transaction immediately

        // Rule: KYC limit check (simplified - would need real KYC data)
        // Skipped in this implementation - would integrate with KYC provider

        // === FLAG Rules (Post-commit) ===

        // Rule: Large transaction
        if ctx.amount >= self.config.large_tx_threshold {
            rules_triggered.push("LARGE_TX_ALERT".to_string());
            decisions.push(AmlDecision::flagged(
                format!("Large transaction: {} {}", ctx.amount, ctx.asset),
                RiskScore::Medium,
                ApprovalLevel::L1,
            ));
        }

        // Rule: CTR threshold
        if ctx.amount >= self.config.ctr_threshold {
            rules_triggered.push("CTR_THRESHOLD".to_string());
            // CTR is a reporting requirement, not necessarily a flag
            // But we log it
        }

        // Rule: Structuring detection
        let tx_count = self.state.tx_count_in_last(&ctx.user_id, self.config.velocity_window_minutes);
        let volume = self.state.volume_in_last(&ctx.user_id, &ctx.asset, self.config.velocity_window_minutes);

        if tx_count >= self.config.structuring_tx_count
            && volume >= self.config.structuring_threshold
            && volume < self.config.ctr_threshold
        {
            rules_triggered.push("STRUCTURING_DETECTION".to_string());
            decisions.push(AmlDecision::flagged(
                "Potential structuring pattern detected",
                RiskScore::High,
                ApprovalLevel::L2,
            ));
        }

        // Rule: Velocity check
        if tx_count >= self.config.velocity_tx_threshold {
            rules_triggered.push("VELOCITY_ALERT".to_string());
            decisions.push(AmlDecision::flagged(
                format!("High transaction velocity: {} tx in {}min", tx_count, self.config.velocity_window_minutes),
                RiskScore::Medium,
                ApprovalLevel::L1,
            ));
        }

        // Rule: New account large transaction
        if let Some(age_days) = ctx.account_age_days {
            if age_days < self.config.new_account_days
                && ctx.amount >= self.config.large_tx_threshold / Decimal::new(2, 0)
            {
                rules_triggered.push("NEW_ACCOUNT_LARGE_TX".to_string());
                decisions.push(AmlDecision::flagged(
                    format!("New account ({} days) large transaction", age_days),
                    RiskScore::Medium,
                    ApprovalLevel::L1,
                ));
            }
        }

        // Aggregate decisions (most restrictive wins)
        let decision = AmlDecision::aggregate(decisions);

        // Determine risk score from decision
        let risk_score = match &decision {
            AmlDecision::Flagged { risk_score, .. } => Some(*risk_score),
            AmlDecision::Blocked { .. } => Some(RiskScore::Critical),
            AmlDecision::Approved => None,
        };

        // Write to compliance ledger
        let event = ComplianceEvent::CheckPerformed {
            id: uuid::Uuid::new_v4().to_string(),
            correlation_id: ctx.correlation_id.clone(),
            user_id: ctx.user_id.clone(),
            decision: decision.clone(),
            rules_triggered: rules_triggered.clone(),
            risk_score,
            timestamp: Utc::now(),
        };
        self.ledger.append(&event)?;

        // Update in-memory state
        self.state.record_transaction(&ctx.user_id, &ctx.asset, ctx.amount);

        // If flagged, also write a TransactionFlagged event
        if let AmlDecision::Flagged { reason, required_approval, .. } = &decision {
            let expires_at = Utc::now() + self.config.review_expiry();
            let flag_event = ComplianceEvent::transaction_flagged(
                &ctx.correlation_id,
                &ctx.user_id,
                reason,
                *required_approval,
                expires_at,
            );
            self.ledger.append(&flag_event)?;
        }

        Ok(CheckResult {
            decision,
            rules_triggered,
            risk_score,
        })
    }

    /// Record a review decision
    pub fn record_review(
        &mut self,
        flag_id: &str,
        decision: ReviewDecision,
        reviewer_id: &str,
        notes: &str,
    ) -> ComplianceResult<()> {
        let event = ComplianceEvent::review_completed(flag_id, decision, reviewer_id, notes);
        self.ledger.append(&event)?;
        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &ComplianceConfig {
        &self.config
    }

    /// Get the in-memory state (for queries)
    pub fn state(&self) -> &ComplianceState {
        &self.state
    }

    /// Rebuild state from ledger (for startup/recovery)
    pub fn rebuild_state(&mut self) -> ComplianceResult<usize> {
        self.state.clear();

        let events = self.ledger.read_all()?;
        let count = events.len();

        for event in events {
            if let ComplianceEvent::CheckPerformed { user_id, timestamp, .. } = event {
                // We don't have amount/asset in the event, so we just record presence
                // In a real implementation, we'd store more data
                self.state.record_transaction_at(&user_id, "UNKNOWN", Decimal::ZERO, timestamp);
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_ctx(amount: Decimal) -> TransactionContext {
        TransactionContext {
            correlation_id: uuid::Uuid::new_v4().to_string(),
            user_id: "USER-001".to_string(),
            intent: "Deposit".to_string(),
            amount,
            asset: "USDT".to_string(),
            account_age_days: Some(30),
        }
    }

    #[test]
    fn test_small_transaction_approved() {
        let mut engine = ComplianceEngine::in_memory();

        let ctx = create_ctx(dec!(100));
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.decision.is_approved());
        assert!(result.rules_triggered.is_empty());
    }

    #[test]
    fn test_large_transaction_flagged() {
        let mut engine = ComplianceEngine::in_memory();

        let ctx = create_ctx(dec!(15000));
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.decision.is_flagged());
        assert!(result.rules_triggered.contains(&"LARGE_TX_ALERT".to_string()));
        assert!(result.rules_triggered.contains(&"CTR_THRESHOLD".to_string()));
    }

    #[test]
    fn test_velocity_check() {
        let mut engine = ComplianceEngine::in_memory();

        // Make 5 transactions (velocity threshold)
        for i in 0..5 {
            let ctx = TransactionContext {
                correlation_id: format!("TX-{}", i),
                user_id: "USER-001".to_string(),
                intent: "Deposit".to_string(),
                amount: dec!(100),
                asset: "USDT".to_string(),
                account_age_days: Some(30),
            };
            engine.check_transaction(&ctx).unwrap();
        }

        // 6th transaction should trigger velocity alert
        let ctx = create_ctx(dec!(100));
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.rules_triggered.contains(&"VELOCITY_ALERT".to_string()));
    }

    #[test]
    fn test_structuring_detection() {
        let mut engine = ComplianceEngine::in_memory();

        // Make 3 transactions totaling just under CTR threshold
        for i in 0..3 {
            let ctx = TransactionContext {
                correlation_id: format!("TX-{}", i),
                user_id: "USER-001".to_string(),
                intent: "Deposit".to_string(),
                amount: dec!(3000), // 3 x 3000 = 9000 (under 10000 CTR)
                asset: "USDT".to_string(),
                account_age_days: Some(30),
            };
            engine.check_transaction(&ctx).unwrap();
        }

        // 4th transaction should trigger structuring
        let ctx = TransactionContext {
            correlation_id: "TX-3".to_string(),
            user_id: "USER-001".to_string(),
            intent: "Deposit".to_string(),
            amount: dec!(500),
            asset: "USDT".to_string(),
            account_age_days: Some(30),
        };
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.rules_triggered.contains(&"STRUCTURING_DETECTION".to_string()));
    }

    #[test]
    fn test_new_account_large_tx() {
        let mut engine = ComplianceEngine::in_memory();

        // New account (3 days) with large-ish transaction
        let ctx = TransactionContext {
            correlation_id: "TX-001".to_string(),
            user_id: "NEW-USER".to_string(),
            intent: "Deposit".to_string(),
            amount: dec!(6000), // >= 10000/2 = 5000
            asset: "USDT".to_string(),
            account_age_days: Some(3), // < 7 days
        };
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.rules_triggered.contains(&"NEW_ACCOUNT_LARGE_TX".to_string()));
    }

    #[test]
    fn test_decision_aggregation() {
        let mut engine = ComplianceEngine::in_memory();

        // Transaction that triggers multiple rules
        // Large + new account = should take highest level
        let ctx = TransactionContext {
            correlation_id: "TX-001".to_string(),
            user_id: "NEW-USER".to_string(),
            intent: "Deposit".to_string(),
            amount: dec!(15000), // Large
            asset: "USDT".to_string(),
            account_age_days: Some(2), // New account
        };
        let result = engine.check_transaction(&ctx).unwrap();

        assert!(result.decision.is_flagged());
        // Multiple rules triggered
        assert!(result.rules_triggered.len() >= 2);
    }

    #[test]
    fn test_record_review() {
        let mut engine = ComplianceEngine::in_memory();

        engine
            .record_review(
                "FLAG-001",
                ReviewDecision::Approved,
                "OFFICER-001",
                "Verified source of funds",
            )
            .unwrap();
    }

    #[test]
    fn test_config_access() {
        let engine = ComplianceEngine::in_memory();
        assert_eq!(engine.config().large_tx_threshold, dec!(10000));
    }

    #[test]
    fn test_state_access() {
        let mut engine = ComplianceEngine::in_memory();

        let ctx = create_ctx(dec!(100));
        engine.check_transaction(&ctx).unwrap();

        assert!(engine.state().has_user("USER-001"));
    }
}
