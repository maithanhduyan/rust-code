//! AML Report formatting for Big 4 compliance
//!
//! This module provides detailed AML (Anti-Money Laundering) report
//! generation suitable for regulatory compliance and audit purposes.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use simbank_core::{AmlFlag, Event};

use crate::exporters::ReportData;

// ============================================================================
// AML Report Data
// ============================================================================

/// AML Report with detailed analysis
#[derive(Debug, Clone)]
pub struct AmlReport {
    pub title: String,
    pub generated_at: DateTime<Utc>,
    pub total_events: usize,
    pub flagged_events: usize,
    pub large_amount_count: usize,
    pub near_threshold_count: usize,
    pub unusual_pattern_count: usize,
    pub high_risk_country_count: usize,
    pub events_by_flag: HashMap<AmlFlag, Vec<FlaggedEvent>>,
    pub risk_score: f64,
}

/// A flagged event for AML reporting
#[derive(Debug, Clone)]
pub struct FlaggedEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub account_id: String,
    pub amount: String,
    pub currency: String,
    pub flag: AmlFlag,
    pub risk_level: RiskLevel,
    pub description: String,
}

/// Risk level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "Low",
            RiskLevel::Medium => "Medium",
            RiskLevel::High => "High",
            RiskLevel::Critical => "Critical",
        }
    }

    pub fn from_flag(flag: &AmlFlag) -> Self {
        match flag {
            AmlFlag::LargeAmount => RiskLevel::High,
            AmlFlag::NearThreshold => RiskLevel::Medium,
            AmlFlag::UnusualPattern => RiskLevel::High,
            AmlFlag::HighRiskCountry => RiskLevel::Critical,
            AmlFlag::CrossBorder => RiskLevel::Medium,
            AmlFlag::NewAccountLargeTx => RiskLevel::High,
            AmlFlag::RapidWithdrawal => RiskLevel::High,
        }
    }
}

impl AmlReport {
    /// Create a new empty AML report
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            generated_at: Utc::now(),
            total_events: 0,
            flagged_events: 0,
            large_amount_count: 0,
            near_threshold_count: 0,
            unusual_pattern_count: 0,
            high_risk_country_count: 0,
            events_by_flag: HashMap::new(),
            risk_score: 0.0,
        }
    }

    /// Generate AML report from events
    pub fn generate(title: &str, events: &[Event]) -> Self {
        let mut report = Self::new(title);
        report.total_events = events.len();

        for event in events {
            for flag in &event.aml_flags {
                report.flagged_events += 1;

                match flag {
                    AmlFlag::LargeAmount => report.large_amount_count += 1,
                    AmlFlag::NearThreshold => report.near_threshold_count += 1,
                    AmlFlag::UnusualPattern => report.unusual_pattern_count += 1,
                    AmlFlag::HighRiskCountry => report.high_risk_country_count += 1,
                    // Other flags are counted in flagged_events but don't have dedicated counters
                    AmlFlag::CrossBorder | AmlFlag::NewAccountLargeTx | AmlFlag::RapidWithdrawal => {},
                }

                let flagged_event = FlaggedEvent {
                    event_id: event.event_id.clone(),
                    timestamp: event.timestamp,
                    event_type: event.event_type.as_str().to_string(),
                    account_id: event.account_id.clone(),
                    amount: event.amount.map(|a| a.to_string()).unwrap_or_default(),
                    currency: event.currency.clone().unwrap_or_default(),
                    flag: flag.clone(),
                    risk_level: RiskLevel::from_flag(flag),
                    description: event.description.clone().unwrap_or_default(),
                };

                report.events_by_flag
                    .entry(flag.clone())
                    .or_insert_with(Vec::new)
                    .push(flagged_event);
            }
        }

        report.calculate_risk_score();
        report
    }

    /// Calculate overall risk score (0-100)
    fn calculate_risk_score(&mut self) {
        if self.total_events == 0 {
            self.risk_score = 0.0;
            return;
        }

        // Weight factors for different risk types
        let weights = [
            (self.large_amount_count as f64, 3.0),      // High weight
            (self.near_threshold_count as f64, 2.0),    // Medium weight
            (self.unusual_pattern_count as f64, 3.5),   // High weight
            (self.high_risk_country_count as f64, 5.0), // Highest weight
        ];

        let weighted_sum: f64 = weights.iter().map(|(count, weight)| count * weight).sum();
        let max_possible = self.total_events as f64 * 5.0; // Max weight

        self.risk_score = (weighted_sum / max_possible * 100.0).min(100.0);
    }

    /// Get risk classification based on score
    pub fn risk_classification(&self) -> RiskLevel {
        match self.risk_score as u32 {
            0..=25 => RiskLevel::Low,
            26..=50 => RiskLevel::Medium,
            51..=75 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }

    /// Get summary text
    pub fn summary_text(&self) -> String {
        let mut summary = String::new();
        summary.push_str(&format!("=== {} ===\n\n", self.title));
        summary.push_str(&format!("Generated: {}\n", self.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        summary.push_str(&format!("Risk Score: {:.1}/100 ({})\n\n", self.risk_score, self.risk_classification().as_str()));

        summary.push_str("--- Statistics ---\n");
        summary.push_str(&format!("Total Events Analyzed: {}\n", self.total_events));
        summary.push_str(&format!("Flagged Events: {} ({:.1}%)\n",
            self.flagged_events,
            if self.total_events > 0 { self.flagged_events as f64 / self.total_events as f64 * 100.0 } else { 0.0 }
        ));
        summary.push_str(&format!("  - Large Amount (>$10,000): {}\n", self.large_amount_count));
        summary.push_str(&format!("  - Near Threshold ($9,000-$9,999): {}\n", self.near_threshold_count));
        summary.push_str(&format!("  - Unusual Pattern: {}\n", self.unusual_pattern_count));
        summary.push_str(&format!("  - High Risk Country: {}\n", self.high_risk_country_count));

        summary
    }

    /// Get flagged events sorted by risk level (highest first)
    pub fn flagged_events_sorted(&self) -> Vec<&FlaggedEvent> {
        let mut events: Vec<&FlaggedEvent> = self.events_by_flag
            .values()
            .flatten()
            .collect();

        events.sort_by(|a, b| {
            let a_score = match a.risk_level {
                RiskLevel::Critical => 4,
                RiskLevel::High => 3,
                RiskLevel::Medium => 2,
                RiskLevel::Low => 1,
            };
            let b_score = match b.risk_level {
                RiskLevel::Critical => 4,
                RiskLevel::High => 3,
                RiskLevel::Medium => 2,
                RiskLevel::Low => 1,
            };
            b_score.cmp(&a_score)
        });

        events
    }
}

impl ReportData for AmlReport {
    fn title(&self) -> &str {
        &self.title
    }

    fn headers(&self) -> Vec<String> {
        vec![
            "Event ID".to_string(),
            "Timestamp".to_string(),
            "Type".to_string(),
            "Account".to_string(),
            "Amount".to_string(),
            "Currency".to_string(),
            "Flag".to_string(),
            "Risk Level".to_string(),
            "Description".to_string(),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.flagged_events_sorted()
            .iter()
            .map(|e| {
                vec![
                    e.event_id.clone(),
                    e.timestamp.to_rfc3339(),
                    e.event_type.clone(),
                    e.account_id.clone(),
                    e.amount.clone(),
                    e.currency.clone(),
                    e.flag.as_str().to_string(),
                    e.risk_level.as_str().to_string(),
                    e.description.clone(),
                ]
            })
            .collect()
    }

    fn summary(&self) -> Vec<(String, String)> {
        vec![
            ("Total Events".to_string(), self.total_events.to_string()),
            ("Flagged Events".to_string(), self.flagged_events.to_string()),
            ("Large Amount".to_string(), self.large_amount_count.to_string()),
            ("Near Threshold".to_string(), self.near_threshold_count.to_string()),
            ("Unusual Pattern".to_string(), self.unusual_pattern_count.to_string()),
            ("High Risk Country".to_string(), self.high_risk_country_count.to_string()),
            ("Risk Score".to_string(), format!("{:.1}/100", self.risk_score)),
            ("Risk Level".to_string(), self.risk_classification().as_str().to_string()),
            ("Generated At".to_string(), self.generated_at.to_rfc3339()),
        ]
    }
}

// ============================================================================
// Velocity Report (for detecting structuring)
// ============================================================================

/// Velocity analysis for detecting rapid transactions
#[derive(Debug, Clone)]
pub struct VelocityReport {
    pub title: String,
    pub generated_at: DateTime<Utc>,
    pub analysis_window_hours: u32,
    pub accounts: Vec<VelocityAnalysis>,
}

#[derive(Debug, Clone)]
pub struct VelocityAnalysis {
    pub account_id: String,
    pub transaction_count: usize,
    pub total_amount: String,
    pub time_span_minutes: i64,
    pub avg_transaction_interval: String,
    pub risk_level: RiskLevel,
    pub transactions: Vec<String>, // Event IDs
}

impl VelocityReport {
    pub fn new(title: &str, window_hours: u32) -> Self {
        Self {
            title: title.to_string(),
            generated_at: Utc::now(),
            analysis_window_hours: window_hours,
            accounts: Vec::new(),
        }
    }

    pub fn generate(title: &str, events: &[Event], window_hours: u32) -> Self {
        let mut report = Self::new(title, window_hours);

        // Group events by account
        let mut by_account: HashMap<String, Vec<&Event>> = HashMap::new();
        for event in events {
            if event.amount.is_some() {
                by_account
                    .entry(event.account_id.clone())
                    .or_insert_with(Vec::new)
                    .push(event);
            }
        }

        // Analyze each account
        for (account_id, account_events) in by_account {
            if account_events.len() < 2 {
                continue;
            }

            // Sort by timestamp
            let mut sorted_events = account_events.clone();
            sorted_events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

            // Calculate metrics
            let total: rust_decimal::Decimal = sorted_events
                .iter()
                .filter_map(|e| e.amount)
                .sum();

            let first_ts = sorted_events.first().map(|e| e.timestamp).unwrap();
            let last_ts = sorted_events.last().map(|e| e.timestamp).unwrap();
            let time_span = (last_ts - first_ts).num_minutes();

            let avg_interval = if sorted_events.len() > 1 {
                time_span as f64 / (sorted_events.len() - 1) as f64
            } else {
                0.0
            };

            // Determine risk level based on velocity
            let risk_level = if avg_interval < 5.0 && sorted_events.len() > 5 {
                RiskLevel::Critical // Very rapid transactions
            } else if avg_interval < 30.0 && sorted_events.len() > 3 {
                RiskLevel::High
            } else if avg_interval < 60.0 && sorted_events.len() > 2 {
                RiskLevel::Medium
            } else {
                RiskLevel::Low
            };

            let analysis = VelocityAnalysis {
                account_id,
                transaction_count: sorted_events.len(),
                total_amount: total.to_string(),
                time_span_minutes: time_span,
                avg_transaction_interval: format!("{:.1} min", avg_interval),
                risk_level,
                transactions: sorted_events.iter().map(|e| e.event_id.clone()).collect(),
            };

            report.accounts.push(analysis);
        }

        // Sort by risk level
        report.accounts.sort_by(|a, b| {
            let a_score = match a.risk_level {
                RiskLevel::Critical => 4,
                RiskLevel::High => 3,
                RiskLevel::Medium => 2,
                RiskLevel::Low => 1,
            };
            let b_score = match b.risk_level {
                RiskLevel::Critical => 4,
                RiskLevel::High => 3,
                RiskLevel::Medium => 2,
                RiskLevel::Low => 1,
            };
            b_score.cmp(&a_score)
        });

        report
    }
}

impl ReportData for VelocityReport {
    fn title(&self) -> &str {
        &self.title
    }

    fn headers(&self) -> Vec<String> {
        vec![
            "Account".to_string(),
            "Tx Count".to_string(),
            "Total Amount".to_string(),
            "Time Span".to_string(),
            "Avg Interval".to_string(),
            "Risk Level".to_string(),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.accounts
            .iter()
            .map(|a| {
                vec![
                    a.account_id.clone(),
                    a.transaction_count.to_string(),
                    a.total_amount.clone(),
                    format!("{} min", a.time_span_minutes),
                    a.avg_transaction_interval.clone(),
                    a.risk_level.as_str().to_string(),
                ]
            })
            .collect()
    }

    fn summary(&self) -> Vec<(String, String)> {
        let high_risk_count = self.accounts.iter()
            .filter(|a| matches!(a.risk_level, RiskLevel::High | RiskLevel::Critical))
            .count();

        vec![
            ("Analysis Window".to_string(), format!("{} hours", self.analysis_window_hours)),
            ("Accounts Analyzed".to_string(), self.accounts.len().to_string()),
            ("High Risk Accounts".to_string(), high_risk_count.to_string()),
            ("Generated At".to_string(), self.generated_at.to_rfc3339()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_aml_report_empty() {
        let report = AmlReport::new("Empty Report");
        assert_eq!(report.total_events, 0);
        assert_eq!(report.risk_score, 0.0);
        assert_eq!(report.risk_classification(), RiskLevel::Low);
    }

    #[test]
    fn test_aml_report_generate() {
        let mut event1 = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");
        event1.aml_flags.push(AmlFlag::LargeAmount);

        let mut event2 = Event::deposit("EVT_002", "CUST_001", "ACC_001", dec!(9500), "USD");
        event2.aml_flags.push(AmlFlag::NearThreshold);

        let events = vec![event1, event2];
        let report = AmlReport::generate("Test AML Report", &events);

        assert_eq!(report.total_events, 2);
        assert_eq!(report.flagged_events, 2);
        assert_eq!(report.large_amount_count, 1);
        assert_eq!(report.near_threshold_count, 1);
        assert!(report.risk_score > 0.0);
    }

    #[test]
    fn test_risk_level_from_flag() {
        assert_eq!(RiskLevel::from_flag(&AmlFlag::LargeAmount), RiskLevel::High);
        assert_eq!(RiskLevel::from_flag(&AmlFlag::NearThreshold), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_flag(&AmlFlag::UnusualPattern), RiskLevel::High);
        assert_eq!(RiskLevel::from_flag(&AmlFlag::HighRiskCountry), RiskLevel::Critical);
    }

    #[test]
    fn test_flagged_events_sorted() {
        let mut event1 = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");
        event1.aml_flags.push(AmlFlag::NearThreshold); // Medium risk

        let mut event2 = Event::deposit("EVT_002", "CUST_001", "ACC_001", dec!(50000), "USD");
        event2.aml_flags.push(AmlFlag::HighRiskCountry); // Critical risk

        let events = vec![event1, event2];
        let report = AmlReport::generate("Test", &events);

        let sorted = report.flagged_events_sorted();
        assert!(!sorted.is_empty());
        // Critical should come first
        assert_eq!(sorted[0].risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_aml_report_summary() {
        let report = AmlReport::new("Summary Test");
        let summary = report.summary();

        assert!(summary.iter().any(|(k, _)| k == "Total Events"));
        assert!(summary.iter().any(|(k, _)| k == "Risk Score"));
    }

    #[test]
    fn test_aml_report_as_report_data() {
        let mut event = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");
        event.aml_flags.push(AmlFlag::LargeAmount);

        let events = vec![event];
        let report = AmlReport::generate("Test", &events);

        // Test ReportData trait implementation
        assert_eq!(report.title(), "Test");
        assert!(!report.headers().is_empty());
        assert!(!report.rows().is_empty());
        assert!(!report.summary().is_empty());
    }

    #[test]
    fn test_velocity_report_empty() {
        let report = VelocityReport::new("Empty Velocity", 24);
        assert_eq!(report.accounts.len(), 0);
        assert_eq!(report.analysis_window_hours, 24);
    }

    #[test]
    fn test_velocity_report_generate() {
        use chrono::Duration;

        let base_time = Utc::now();

        let mut events = vec![];
        for i in 0..5 {
            let mut event = Event::deposit(
                &format!("EVT_{:03}", i),
                "CUST_001",
                "ACC_001",
                dec!(100),
                "USD",
            );
            // Set timestamps 2 minutes apart
            event.timestamp = base_time + Duration::minutes(i * 2);
            events.push(event);
        }

        let report = VelocityReport::generate("Velocity Test", &events, 24);

        assert_eq!(report.accounts.len(), 1);
        assert_eq!(report.accounts[0].transaction_count, 5);
    }

    #[test]
    fn test_summary_text_format() {
        let mut event = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");
        event.aml_flags.push(AmlFlag::LargeAmount);

        let events = vec![event];
        let report = AmlReport::generate("Test Report", &events);

        let summary = report.summary_text();
        assert!(summary.contains("Test Report"));
        assert!(summary.contains("Risk Score"));
        assert!(summary.contains("Large Amount"));
    }
}
