//! Compliance configuration with configurable thresholds
//!
//! All thresholds are configurable via file/env, not hardcoded.
//! This allows production tuning without recompilation.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the Compliance Engine
///
/// All thresholds can be overridden via environment variables or config file.
/// Defaults are conservative (stricter limits).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConfig {
    // === Thresholds ===
    /// Large transaction threshold (triggers FLAG)
    #[serde(default = "default_large_tx_threshold")]
    pub large_tx_threshold: Decimal,

    /// Currency Transaction Report threshold (regulatory requirement)
    #[serde(default = "default_ctr_threshold")]
    pub ctr_threshold: Decimal,

    /// Structuring detection threshold (just below CTR)
    #[serde(default = "default_structuring_threshold")]
    pub structuring_threshold: Decimal,

    /// Number of transactions to trigger structuring alert
    #[serde(default = "default_structuring_tx_count")]
    pub structuring_tx_count: u32,

    /// Account age considered "new" (in days)
    #[serde(default = "default_new_account_days")]
    pub new_account_days: i64,

    // === Velocity Windows ===
    /// Time window for velocity checks (in minutes)
    #[serde(default = "default_velocity_window_minutes")]
    pub velocity_window_minutes: u32,

    /// Transaction count threshold for velocity alert
    #[serde(default = "default_velocity_tx_threshold")]
    pub velocity_tx_threshold: u32,

    // === External Services ===
    /// Timeout for external service calls (KYC, Watchlist)
    #[serde(default = "default_external_timeout_ms")]
    pub external_timeout_ms: u64,

    /// Cache TTL for external data
    #[serde(default = "default_external_cache_ttl_secs")]
    pub external_cache_ttl_secs: u64,

    /// Policy when external service fails
    #[serde(default)]
    pub external_fail_policy: FailPolicy,

    // === Review Settings ===
    /// Hours until flagged transaction expires (auto-reject)
    #[serde(default = "default_review_expiry_hours")]
    pub review_expiry_hours: u64,
}

/// Policy when external service (KYC, Watchlist) fails
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailPolicy {
    /// Block transaction if external check fails (SAFER - DEFAULT)
    /// Rationale: False positive > False negative for compliance
    #[default]
    FailClosed,

    /// Allow transaction but flag for review (RISKIER)
    /// Use only when external service is unreliable
    FailOpen,
}

// Default value functions for serde
fn default_large_tx_threshold() -> Decimal {
    Decimal::new(10_000, 0)
}

fn default_ctr_threshold() -> Decimal {
    Decimal::new(10_000, 0)
}

fn default_structuring_threshold() -> Decimal {
    Decimal::new(9_000, 0)
}

fn default_structuring_tx_count() -> u32 {
    3
}

fn default_new_account_days() -> i64 {
    7
}

fn default_velocity_window_minutes() -> u32 {
    60
}

fn default_velocity_tx_threshold() -> u32 {
    5
}

fn default_external_timeout_ms() -> u64 {
    500
}

fn default_external_cache_ttl_secs() -> u64 {
    300 // 5 minutes
}

fn default_review_expiry_hours() -> u64 {
    72 // 3 days
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            large_tx_threshold: default_large_tx_threshold(),
            ctr_threshold: default_ctr_threshold(),
            structuring_threshold: default_structuring_threshold(),
            structuring_tx_count: default_structuring_tx_count(),
            new_account_days: default_new_account_days(),
            velocity_window_minutes: default_velocity_window_minutes(),
            velocity_tx_threshold: default_velocity_tx_threshold(),
            external_timeout_ms: default_external_timeout_ms(),
            external_cache_ttl_secs: default_external_cache_ttl_secs(),
            external_fail_policy: FailPolicy::default(),
            review_expiry_hours: default_review_expiry_hours(),
        }
    }
}

impl ComplianceConfig {
    /// Load configuration from JSON file
    pub fn from_file(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })
    }

    /// Get external timeout as Duration
    pub fn external_timeout(&self) -> Duration {
        Duration::from_millis(self.external_timeout_ms)
    }

    /// Get cache TTL as Duration
    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.external_cache_ttl_secs)
    }

    /// Get review expiry as chrono Duration
    pub fn review_expiry(&self) -> chrono::Duration {
        chrono::Duration::hours(self.review_expiry_hours as i64)
    }

    /// Get new account threshold as chrono Duration
    pub fn new_account_threshold(&self) -> chrono::Duration {
        chrono::Duration::days(self.new_account_days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ComplianceConfig::default();

        assert_eq!(config.large_tx_threshold, Decimal::new(10_000, 0));
        assert_eq!(config.ctr_threshold, Decimal::new(10_000, 0));
        assert_eq!(config.structuring_threshold, Decimal::new(9_000, 0));
        assert_eq!(config.structuring_tx_count, 3);
        assert_eq!(config.new_account_days, 7);
        assert_eq!(config.velocity_window_minutes, 60);
        assert_eq!(config.velocity_tx_threshold, 5);
        assert_eq!(config.external_timeout_ms, 500);
        assert_eq!(config.external_cache_ttl_secs, 300);
        assert_eq!(config.external_fail_policy, FailPolicy::FailClosed);
        assert_eq!(config.review_expiry_hours, 72);
    }

    #[test]
    fn test_fail_policy_default_is_closed() {
        let policy = FailPolicy::default();
        assert_eq!(policy, FailPolicy::FailClosed);
    }

    #[test]
    fn test_config_serialization() {
        let config = ComplianceConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();

        // Should contain our fields
        assert!(json.contains("large_tx_threshold"));
        assert!(json.contains("fail_closed"));

        // Should be deserializable
        let parsed: ComplianceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.large_tx_threshold, config.large_tx_threshold);
    }

    #[test]
    fn test_config_partial_json() {
        // Should use defaults for missing fields
        let json = r#"{ "large_tx_threshold": "5000" }"#;
        let config: ComplianceConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.large_tx_threshold, Decimal::new(5_000, 0));
        assert_eq!(config.ctr_threshold, Decimal::new(10_000, 0)); // default
    }

    #[test]
    fn test_duration_helpers() {
        let config = ComplianceConfig::default();

        assert_eq!(config.external_timeout(), Duration::from_millis(500));
        assert_eq!(config.cache_ttl(), Duration::from_secs(300));
        assert_eq!(config.review_expiry(), chrono::Duration::hours(72));
        assert_eq!(config.new_account_threshold(), chrono::Duration::days(7));
    }
}
