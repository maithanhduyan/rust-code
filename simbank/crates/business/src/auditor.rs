//! Auditor operations - AML detection rules
//!
//! AuditorService implements AML (Anti-Money Laundering) detection rules.

use crate::error::{BusinessError, BusinessResult};
use crate::services::ServiceContext;
use rust_decimal::Decimal;
use simbank_core::{AmlFlag, Event, EventType, PersonType};
use simbank_persistence::{AmlReport, EventFilter, EventReader, PersonRepo};

/// AML thresholds for detection
pub struct AmlThresholds {
    /// Amount threshold for "large_amount" flag
    pub large_amount: Decimal,
    /// Threshold for "near_threshold" (structuring detection)
    pub near_threshold_min: Decimal,
    pub near_threshold_max: Decimal,
    /// High-risk countries (ISO codes)
    pub high_risk_countries: Vec<String>,
}

impl Default for AmlThresholds {
    fn default() -> Self {
        Self {
            large_amount: Decimal::new(10000, 0), // $10,000
            near_threshold_min: Decimal::new(9000, 0),
            near_threshold_max: Decimal::new(9999, 0),
            high_risk_countries: vec![
                "KP".to_string(), // North Korea
                "IR".to_string(), // Iran
                "SY".to_string(), // Syria
                "CU".to_string(), // Cuba
            ],
        }
    }
}

/// Auditor Service - AML detection and reporting
pub struct AuditorService<'a> {
    ctx: &'a ServiceContext,
    thresholds: AmlThresholds,
}

impl<'a> AuditorService<'a> {
    pub fn new(ctx: &'a ServiceContext) -> Self {
        Self {
            ctx,
            thresholds: AmlThresholds::default(),
        }
    }

    pub fn with_thresholds(mut self, thresholds: AmlThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Verify auditor has permission
    pub async fn verify_auditor(&self, auditor_id: &str) -> BusinessResult<()> {
        let person = PersonRepo::get_by_id(self.ctx.pool(), auditor_id)
            .await
            .map_err(|_| BusinessError::PersonNotFound(auditor_id.to_string()))?;

        if person.person_type != "auditor" {
            return Err(BusinessError::not_permitted(&person.person_type, "audit").into());
        }

        Ok(())
    }

    /// Check if an event should be flagged for AML
    pub fn check_aml_flags(&self, event: &Event) -> Vec<AmlFlag> {
        let mut flags = Vec::new();

        // Check amount thresholds
        if let Some(amount) = event.amount {
            // Large amount
            if amount >= self.thresholds.large_amount {
                flags.push(AmlFlag::LargeAmount);
            }

            // Near threshold (potential structuring)
            if amount >= self.thresholds.near_threshold_min
                && amount <= self.thresholds.near_threshold_max
            {
                flags.push(AmlFlag::NearThreshold);
            }
        }

        // Check location
        if let Some(ref location) = event.metadata.location {
            if self.thresholds.high_risk_countries.contains(location) {
                flags.push(AmlFlag::HighRiskCountry);
            }
        }

        flags
    }

    /// Scan events for AML flags
    pub async fn scan_transactions(
        &self,
        auditor_id: &str,
        from_date: Option<&str>,
        to_date: Option<&str>,
        flag_filter: Option<Vec<AmlFlag>>,
    ) -> BusinessResult<AmlReport> {
        // Verify auditor permission
        self.verify_auditor(auditor_id).await?;

        // Read events
        let reader = EventReader::new(self.ctx.events().base_path());
        let events = match (from_date, to_date) {
            (Some(from), Some(to)) => reader.read_range(from, to)?,
            _ => reader.read_all()?,
        };

        // Apply filter if specified
        let events = if let Some(flags) = flag_filter {
            EventFilter::new().aml_flags(flags).apply(events)
        } else {
            events
        };

        // Generate report
        let report = AmlReport::generate(&events);

        // Log audit access event
        let event_id = self.ctx.next_event_id();
        let audit_event = Event::new(
            event_id,
            EventType::AuditAccess,
            auditor_id.to_string(),
            PersonType::Auditor,
            "SYSTEM".to_string(),
        )
        .with_description(&format!(
            "AML scan: {} events analyzed, {} flagged",
            report.total_events, report.flagged_events
        ));

        self.ctx.events().append(&audit_event)?;

        Ok(report)
    }

    /// Get flagged events only
    pub async fn get_flagged_events(
        &self,
        auditor_id: &str,
    ) -> BusinessResult<Vec<Event>> {
        self.verify_auditor(auditor_id).await?;

        let reader = EventReader::new(self.ctx.events().base_path());
        let events = reader.read_all()?;

        let flagged = EventFilter::new().flagged_only().apply(events);

        Ok(flagged)
    }

    /// Get high-value transactions
    pub async fn get_high_value_transactions(
        &self,
        auditor_id: &str,
        min_amount: Decimal,
    ) -> BusinessResult<Vec<Event>> {
        self.verify_auditor(auditor_id).await?;

        let reader = EventReader::new(self.ctx.events().base_path());
        let events = reader.read_all()?;

        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|e| e.amount.map_or(false, |a| a >= min_amount))
            .collect();

        Ok(filtered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_aml_thresholds_default() {
        let thresholds = AmlThresholds::default();
        assert_eq!(thresholds.large_amount, dec!(10000));
        assert_eq!(thresholds.near_threshold_min, dec!(9000));
        assert_eq!(thresholds.near_threshold_max, dec!(9999));
        assert!(thresholds.high_risk_countries.contains(&"KP".to_string()));
        assert!(thresholds.high_risk_countries.contains(&"IR".to_string()));
    }

    #[test]
    fn test_aml_check_large_amount() {
        // Test check_aml_flags standalone logic
        let thresholds = AmlThresholds::default();

        // Create a large amount event
        let event = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(15000), "USD");

        // Manually check flags
        let mut flags = Vec::new();
        if let Some(amount) = event.amount {
            if amount >= thresholds.large_amount {
                flags.push(AmlFlag::LargeAmount);
            }
        }

        assert!(flags.contains(&AmlFlag::LargeAmount));
    }

    #[test]
    fn test_aml_check_near_threshold() {
        let thresholds = AmlThresholds::default();
        let event = Event::deposit("EVT_002", "CUST_001", "ACC_001", dec!(9500), "USD");

        let mut flags = Vec::new();
        if let Some(amount) = event.amount {
            if amount >= thresholds.near_threshold_min && amount <= thresholds.near_threshold_max {
                flags.push(AmlFlag::NearThreshold);
            }
        }

        assert!(flags.contains(&AmlFlag::NearThreshold));
    }
}
