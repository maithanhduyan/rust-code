//! Compliance events (written to Compliance Ledger)
//!
//! These events form the Decision Truth - separate from financial truth.
//! All events are append-only and immutable.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::decision::{AmlDecision, ApprovalLevel, RiskScore};

/// Events appended to Compliance Ledger (append-only JSONL)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ComplianceEvent {
    /// Transaction was checked against rules
    CheckPerformed {
        id: String,
        correlation_id: String,
        user_id: String,
        decision: AmlDecision,
        rules_triggered: Vec<String>,
        risk_score: Option<RiskScore>,
        timestamp: DateTime<Utc>,
    },

    /// Transaction was flagged for review
    TransactionFlagged {
        id: String,
        correlation_id: String,
        user_id: String,
        reason: String,
        required_approval: ApprovalLevel,
        expires_at: DateTime<Utc>,
        timestamp: DateTime<Utc>,
    },

    /// Review decision made
    ReviewCompleted {
        id: String,
        flag_id: String,
        decision: ReviewDecision,
        reviewer_id: String,
        notes: String,
        timestamp: DateTime<Utc>,
    },

    /// Rule set activated/deactivated
    RuleSetChanged {
        id: String,
        rule_set_name: String,
        rule_set_version: String,
        rule_set_hash: String,
        action: RuleAction,
        performed_by: String,
        approved_by: Vec<String>,
        timestamp: DateTime<Utc>,
    },

    /// User added to internal watchlist
    WatchlistUpdated {
        id: String,
        user_id: String,
        action: WatchlistAction,
        reason: String,
        performed_by: String,
        timestamp: DateTime<Utc>,
    },
}

/// Review decision outcomes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    /// Transaction approved after review
    Approved,
    /// Transaction rejected after review
    Rejected,
    /// Review expired (auto-reject)
    Expired,
}

/// Rule set actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    /// Rule set activated
    Activated,
    /// Rule set deactivated
    Deactivated,
}

/// Watchlist actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchlistAction {
    /// User added to watchlist
    Added,
    /// User removed from watchlist
    Removed,
}

impl ComplianceEvent {
    /// Get the event ID
    pub fn id(&self) -> &str {
        match self {
            ComplianceEvent::CheckPerformed { id, .. } => id,
            ComplianceEvent::TransactionFlagged { id, .. } => id,
            ComplianceEvent::ReviewCompleted { id, .. } => id,
            ComplianceEvent::RuleSetChanged { id, .. } => id,
            ComplianceEvent::WatchlistUpdated { id, .. } => id,
        }
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            ComplianceEvent::CheckPerformed { timestamp, .. } => *timestamp,
            ComplianceEvent::TransactionFlagged { timestamp, .. } => *timestamp,
            ComplianceEvent::ReviewCompleted { timestamp, .. } => *timestamp,
            ComplianceEvent::RuleSetChanged { timestamp, .. } => *timestamp,
            ComplianceEvent::WatchlistUpdated { timestamp, .. } => *timestamp,
        }
    }

    /// Get the user ID if applicable
    pub fn user_id(&self) -> Option<&str> {
        match self {
            ComplianceEvent::CheckPerformed { user_id, .. } => Some(user_id),
            ComplianceEvent::TransactionFlagged { user_id, .. } => Some(user_id),
            ComplianceEvent::ReviewCompleted { .. } => None,
            ComplianceEvent::RuleSetChanged { .. } => None,
            ComplianceEvent::WatchlistUpdated { user_id, .. } => Some(user_id),
        }
    }

    /// Create a new CheckPerformed event
    pub fn check_performed(
        correlation_id: impl Into<String>,
        user_id: impl Into<String>,
        decision: AmlDecision,
        rules_triggered: Vec<String>,
    ) -> Self {
        ComplianceEvent::CheckPerformed {
            id: uuid::Uuid::new_v4().to_string(),
            correlation_id: correlation_id.into(),
            user_id: user_id.into(),
            decision,
            rules_triggered,
            risk_score: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a new TransactionFlagged event
    pub fn transaction_flagged(
        correlation_id: impl Into<String>,
        user_id: impl Into<String>,
        reason: impl Into<String>,
        required_approval: ApprovalLevel,
        expires_at: DateTime<Utc>,
    ) -> Self {
        ComplianceEvent::TransactionFlagged {
            id: uuid::Uuid::new_v4().to_string(),
            correlation_id: correlation_id.into(),
            user_id: user_id.into(),
            reason: reason.into(),
            required_approval,
            expires_at,
            timestamp: Utc::now(),
        }
    }

    /// Create a new ReviewCompleted event
    pub fn review_completed(
        flag_id: impl Into<String>,
        decision: ReviewDecision,
        reviewer_id: impl Into<String>,
        notes: impl Into<String>,
    ) -> Self {
        ComplianceEvent::ReviewCompleted {
            id: uuid::Uuid::new_v4().to_string(),
            flag_id: flag_id.into(),
            decision,
            reviewer_id: reviewer_id.into(),
            notes: notes.into(),
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_performed_serialization() {
        let event = ComplianceEvent::check_performed(
            "TX-123",
            "USER-001",
            AmlDecision::Approved,
            vec!["RULE_1".to_string()],
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("check_performed"));
        assert!(json.contains("TX-123"));
        assert!(json.contains("USER-001"));

        let parsed: ComplianceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), event.id());
    }

    #[test]
    fn test_transaction_flagged_serialization() {
        let expires_at = Utc::now() + chrono::Duration::hours(72);
        let event = ComplianceEvent::transaction_flagged(
            "TX-456",
            "USER-002",
            "Large transaction",
            ApprovalLevel::L2,
            expires_at,
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("transaction_flagged"));
        assert!(json.contains("Large transaction"));
        assert!(json.contains("l2"));
    }

    #[test]
    fn test_review_decision_serialization() {
        let approved = ReviewDecision::Approved;
        let rejected = ReviewDecision::Rejected;
        let expired = ReviewDecision::Expired;

        assert_eq!(
            serde_json::to_string(&approved).unwrap(),
            "\"approved\""
        );
        assert_eq!(
            serde_json::to_string(&rejected).unwrap(),
            "\"rejected\""
        );
        assert_eq!(
            serde_json::to_string(&expired).unwrap(),
            "\"expired\""
        );
    }

    #[test]
    fn test_event_accessors() {
        let event = ComplianceEvent::check_performed(
            "TX-789",
            "USER-003",
            AmlDecision::Approved,
            vec![],
        );

        assert!(!event.id().is_empty());
        assert_eq!(event.user_id(), Some("USER-003"));
        assert!(event.timestamp() <= Utc::now());
    }
}
