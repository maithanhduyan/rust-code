//! AML Decision types with formal lattice ordering
//!
//! Decisions follow a formal ordering for aggregation:
//! `Approved < Flagged(L1) < Flagged(L2) < ... < Blocked`
//!
//! Aggregation: `max(all_decisions)` - most restrictive wins

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Risk score levels - ordered from lowest to highest
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskScore {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl PartialOrd for RiskScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RiskScore {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl Default for RiskScore {
    fn default() -> Self {
        RiskScore::Low
    }
}

/// Approval level required for flagged transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLevel {
    /// Single compliance officer
    L1 = 1,
    /// Senior compliance officer
    L2 = 2,
    /// Head of compliance
    L3 = 3,
    /// Board level (for critical cases)
    L4 = 4,
}

impl PartialOrd for ApprovalLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ApprovalLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl Default for ApprovalLevel {
    fn default() -> Self {
        ApprovalLevel::L1
    }
}

/// AML Decision - Formal Lattice
///
/// Ordering: Approved < Flagged < Blocked
/// For Flagged decisions, higher ApprovalLevel = more restrictive
///
/// Aggregation uses `max()` - most restrictive decision wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AmlDecision {
    /// Transaction approved, continue (lowest in lattice)
    Approved,

    /// Transaction flagged, requires manual review
    Flagged {
        reason: String,
        risk_score: RiskScore,
        required_approval: ApprovalLevel,
    },

    /// Transaction blocked (highest in lattice)
    Blocked {
        reason: String,
        compliance_code: String,
    },
}

impl AmlDecision {
    /// Create a new Flagged decision
    pub fn flagged(reason: impl Into<String>, risk_score: RiskScore, level: ApprovalLevel) -> Self {
        AmlDecision::Flagged {
            reason: reason.into(),
            risk_score,
            required_approval: level,
        }
    }

    /// Create a new Blocked decision
    pub fn blocked(reason: impl Into<String>, code: impl Into<String>) -> Self {
        AmlDecision::Blocked {
            reason: reason.into(),
            compliance_code: code.into(),
        }
    }

    /// Check if transaction is approved
    pub fn is_approved(&self) -> bool {
        matches!(self, AmlDecision::Approved)
    }

    /// Check if transaction is flagged
    pub fn is_flagged(&self) -> bool {
        matches!(self, AmlDecision::Flagged { .. })
    }

    /// Check if transaction is blocked
    pub fn is_blocked(&self) -> bool {
        matches!(self, AmlDecision::Blocked { .. })
    }

    /// Get numeric value for ordering
    fn order_value(&self) -> u8 {
        match self {
            AmlDecision::Approved => 0,
            AmlDecision::Flagged { required_approval, .. } => *required_approval as u8,
            AmlDecision::Blocked { .. } => 10, // Always highest
        }
    }

    /// Aggregate multiple decisions: take the most restrictive
    ///
    /// This is the core of the formal lattice - we always escalate
    /// to the highest restriction level.
    pub fn aggregate(decisions: impl IntoIterator<Item = AmlDecision>) -> AmlDecision {
        decisions
            .into_iter()
            .max()
            .unwrap_or(AmlDecision::Approved)
    }
}

impl PartialOrd for AmlDecision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AmlDecision {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_value().cmp(&other.order_value())
    }
}

impl Default for AmlDecision {
    fn default() -> Self {
        AmlDecision::Approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_score_ordering() {
        assert!(RiskScore::Low < RiskScore::Medium);
        assert!(RiskScore::Medium < RiskScore::High);
        assert!(RiskScore::High < RiskScore::Critical);
    }

    #[test]
    fn test_approval_level_ordering() {
        assert!(ApprovalLevel::L1 < ApprovalLevel::L2);
        assert!(ApprovalLevel::L2 < ApprovalLevel::L3);
        assert!(ApprovalLevel::L3 < ApprovalLevel::L4);
    }

    #[test]
    fn test_aml_decision_ordering() {
        let approved = AmlDecision::Approved;
        let flagged_l1 = AmlDecision::flagged("test", RiskScore::Low, ApprovalLevel::L1);
        let flagged_l2 = AmlDecision::flagged("test", RiskScore::High, ApprovalLevel::L2);
        let blocked = AmlDecision::blocked("test", "AML-001");

        // Approved < Flagged < Blocked
        assert!(approved < flagged_l1);
        assert!(flagged_l1 < flagged_l2);
        assert!(flagged_l2 < blocked);
    }

    #[test]
    fn test_aggregate_empty() {
        let result = AmlDecision::aggregate(vec![]);
        assert_eq!(result, AmlDecision::Approved);
    }

    #[test]
    fn test_aggregate_all_approved() {
        let decisions = vec![
            AmlDecision::Approved,
            AmlDecision::Approved,
            AmlDecision::Approved,
        ];
        let result = AmlDecision::aggregate(decisions);
        assert_eq!(result, AmlDecision::Approved);
    }

    #[test]
    fn test_aggregate_takes_most_restrictive() {
        let decisions = vec![
            AmlDecision::Approved,
            AmlDecision::flagged("test1", RiskScore::Low, ApprovalLevel::L1),
            AmlDecision::flagged("test2", RiskScore::High, ApprovalLevel::L2),
        ];
        let result = AmlDecision::aggregate(decisions);

        // Should take L2 flagged (highest)
        assert!(result.is_flagged());
        if let AmlDecision::Flagged { required_approval, .. } = result {
            assert_eq!(required_approval, ApprovalLevel::L2);
        }
    }

    #[test]
    fn test_aggregate_blocked_wins() {
        let decisions = vec![
            AmlDecision::Approved,
            AmlDecision::flagged("test", RiskScore::Critical, ApprovalLevel::L4),
            AmlDecision::blocked("blocked", "AML-001"),
        ];
        let result = AmlDecision::aggregate(decisions);

        assert!(result.is_blocked());
    }

    #[test]
    fn test_decision_serialization() {
        let flagged = AmlDecision::flagged("Large TX", RiskScore::High, ApprovalLevel::L2);
        let json = serde_json::to_string(&flagged).unwrap();

        assert!(json.contains("flagged"));
        assert!(json.contains("Large TX"));
        assert!(json.contains("high"));
        assert!(json.contains("l2"));

        let parsed: AmlDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, flagged);
    }

    #[test]
    fn test_blocked_serialization() {
        let blocked = AmlDecision::blocked("Sanctions match", "OFAC-001");
        let json = serde_json::to_string(&blocked).unwrap();

        assert!(json.contains("blocked"));
        assert!(json.contains("OFAC-001"));
    }
}
