//! Hook context - data passed to hooks

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Context passed to all hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// Unique correlation ID for this transaction
    pub correlation_id: String,

    /// User initiating the transaction
    pub user_id: String,

    /// Transaction intent type
    pub intent: String,

    /// Transaction amount
    pub amount: Decimal,

    /// Asset code (e.g., "USDT", "BTC")
    pub asset: String,

    /// Destination account (if applicable)
    pub destination: Option<String>,

    /// Timestamp of the transaction
    pub timestamp: DateTime<Utc>,

    /// Additional metadata
    pub metadata: HookMetadata,
}

/// Additional metadata for hooks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookMetadata {
    /// User's account age in days
    pub account_age_days: Option<i64>,

    /// User's KYC level (0-4)
    pub kyc_level: Option<u8>,

    /// Whether user is on internal watchlist
    pub is_watchlisted: bool,

    /// Whether user is a PEP (Politically Exposed Person)
    pub is_pep: bool,

    /// Source IP address
    pub source_ip: Option<String>,

    /// Device fingerprint
    pub device_id: Option<String>,
}

impl HookContext {
    /// Create a new hook context
    pub fn new(
        correlation_id: impl Into<String>,
        user_id: impl Into<String>,
        intent: impl Into<String>,
        amount: Decimal,
        asset: impl Into<String>,
    ) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            user_id: user_id.into(),
            intent: intent.into(),
            amount,
            asset: asset.into(),
            destination: None,
            timestamp: Utc::now(),
            metadata: HookMetadata::default(),
        }
    }

    /// Set destination account
    pub fn with_destination(mut self, dest: impl Into<String>) -> Self {
        self.destination = Some(dest.into());
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: HookMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set account age
    pub fn with_account_age(mut self, days: i64) -> Self {
        self.metadata.account_age_days = Some(days);
        self
    }

    /// Set KYC level
    pub fn with_kyc_level(mut self, level: u8) -> Self {
        self.metadata.kyc_level = Some(level);
        self
    }

    /// Mark as watchlisted
    pub fn with_watchlist(mut self, watchlisted: bool) -> Self {
        self.metadata.is_watchlisted = watchlisted;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_context_creation() {
        let ctx = HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT");

        assert_eq!(ctx.correlation_id, "TX-001");
        assert_eq!(ctx.user_id, "USER-001");
        assert_eq!(ctx.intent, "Deposit");
        assert_eq!(ctx.amount, dec!(1000));
        assert_eq!(ctx.asset, "USDT");
        assert!(ctx.destination.is_none());
    }

    #[test]
    fn test_context_builder() {
        let ctx = HookContext::new("TX-001", "USER-001", "Withdrawal", dec!(5000), "USDT")
            .with_destination("external_address")
            .with_account_age(30)
            .with_kyc_level(2)
            .with_watchlist(false);

        assert_eq!(ctx.destination, Some("external_address".to_string()));
        assert_eq!(ctx.metadata.account_age_days, Some(30));
        assert_eq!(ctx.metadata.kyc_level, Some(2));
        assert!(!ctx.metadata.is_watchlisted);
    }

    #[test]
    fn test_context_serialization() {
        let ctx = HookContext::new("TX-001", "USER-001", "Deposit", dec!(1000), "USDT");
        let json = serde_json::to_string(&ctx).unwrap();

        assert!(json.contains("TX-001"));
        assert!(json.contains("USER-001"));

        let parsed: HookContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.correlation_id, ctx.correlation_id);
    }
}
