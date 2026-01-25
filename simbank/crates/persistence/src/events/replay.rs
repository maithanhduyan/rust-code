//! Event Replay - read events from JSONL files
//!
//! Đọc events từ JSONL files để replay, audit, và AML analysis.

use crate::error::{PersistenceError, PersistenceResult};
use chrono::NaiveDate;
use simbank_core::{AmlFlag, Event, EventType};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Event Reader - đọc events từ files JSONL
pub struct EventReader {
    base_path: PathBuf,
}

impl EventReader {
    /// Tạo reader mới
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Đọc tất cả events từ một file
    pub fn read_file(&self, file_path: &Path) -> PersistenceResult<Vec<Event>> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Event = serde_json::from_str(&line)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Đọc events theo ngày
    pub fn read_date(&self, date: &str) -> PersistenceResult<Vec<Event>> {
        let file_path = self.base_path.join(format!("{}.jsonl", date));
        if file_path.exists() {
            self.read_file(&file_path)
        } else {
            Ok(Vec::new())
        }
    }

    /// Đọc events trong khoảng thời gian
    pub fn read_range(&self, from: &str, to: &str) -> PersistenceResult<Vec<Event>> {
        let from_date = NaiveDate::parse_from_str(from, "%Y-%m-%d")
            .map_err(|e| PersistenceError::Other(format!("Invalid from date: {}", e)))?;
        let to_date = NaiveDate::parse_from_str(to, "%Y-%m-%d")
            .map_err(|e| PersistenceError::Other(format!("Invalid to date: {}", e)))?;

        let mut all_events = Vec::new();
        let mut current = from_date;

        while current <= to_date {
            let date_str = current.format("%Y-%m-%d").to_string();
            let events = self.read_date(&date_str)?;
            all_events.extend(events);
            current = current.succ_opt().unwrap_or(current);
        }

        Ok(all_events)
    }

    /// Đọc tất cả events
    pub fn read_all(&self) -> PersistenceResult<Vec<Event>> {
        let mut all_events = Vec::new();

        if !self.base_path.exists() {
            return Ok(all_events);
        }

        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.base_path)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "jsonl"))
            .collect();

        files.sort();

        for file_path in files {
            let events = self.read_file(&file_path)?;
            all_events.extend(events);
        }

        Ok(all_events)
    }
}

/// Event Filter - lọc events theo điều kiện
#[derive(Default)]
pub struct EventFilter {
    /// Lọc theo account ID
    pub account_id: Option<String>,
    /// Lọc theo actor ID (person who performed action)
    pub actor_id: Option<String>,
    /// Lọc theo event types
    pub event_types: Option<Vec<EventType>>,
    /// Lọc theo AML flags
    pub aml_flags: Option<Vec<AmlFlag>>,
    /// Chỉ lấy events có AML flag
    pub only_flagged: bool,
    /// Minimum amount
    pub min_amount: Option<rust_decimal::Decimal>,
    /// Maximum amount
    pub max_amount: Option<rust_decimal::Decimal>,
}

impl EventFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn account(mut self, account_id: &str) -> Self {
        self.account_id = Some(account_id.to_string());
        self
    }

    pub fn actor(mut self, actor_id: &str) -> Self {
        self.actor_id = Some(actor_id.to_string());
        self
    }

    pub fn event_types(mut self, types: Vec<EventType>) -> Self {
        self.event_types = Some(types);
        self
    }

    pub fn aml_flags(mut self, flags: Vec<AmlFlag>) -> Self {
        self.aml_flags = Some(flags);
        self
    }

    pub fn flagged_only(mut self) -> Self {
        self.only_flagged = true;
        self
    }

    pub fn amount_range(mut self, min: rust_decimal::Decimal, max: rust_decimal::Decimal) -> Self {
        self.min_amount = Some(min);
        self.max_amount = Some(max);
        self
    }

    /// Kiểm tra event có match filter không
    pub fn matches(&self, event: &Event) -> bool {
        // Account filter
        if let Some(ref acc_id) = self.account_id {
            if event.account_id != *acc_id {
                return false;
            }
        }

        // Actor filter
        if let Some(ref actor_id) = self.actor_id {
            if event.actor_id != *actor_id {
                return false;
            }
        }

        // Event type filter
        if let Some(ref types) = self.event_types {
            if !types.contains(&event.event_type) {
                return false;
            }
        }

        // AML flag filter
        if let Some(ref flags) = self.aml_flags {
            // Check if event has any of the specified flags
            let has_matching_flag = event.aml_flags.iter().any(|f| flags.contains(f));
            if !has_matching_flag {
                return false;
            }
        }

        // Only flagged filter
        if self.only_flagged && event.aml_flags.is_empty() {
            return false;
        }

        // Amount range filter
        if let Some(amount) = event.amount {
            if let Some(min) = self.min_amount {
                if amount < min {
                    return false;
                }
            }
            if let Some(max) = self.max_amount {
                if amount > max {
                    return false;
                }
            }
        }

        true
    }

    /// Apply filter to events
    pub fn apply(&self, events: Vec<Event>) -> Vec<Event> {
        events.into_iter().filter(|e| self.matches(e)).collect()
    }
}

/// AML Report - báo cáo cho Anti-Money Laundering
pub struct AmlReport {
    pub total_events: usize,
    pub flagged_events: usize,
    pub large_amount_count: usize,
    pub unusual_pattern_count: usize,
    pub high_risk_country_count: usize,
    pub events_by_flag: std::collections::HashMap<String, Vec<Event>>,
}

impl AmlReport {
    /// Tạo AML report từ events
    pub fn generate(events: &[Event]) -> Self {
        let mut report = Self {
            total_events: events.len(),
            flagged_events: 0,
            large_amount_count: 0,
            unusual_pattern_count: 0,
            high_risk_country_count: 0,
            events_by_flag: std::collections::HashMap::new(),
        };

        for event in events {
            if !event.aml_flags.is_empty() {
                report.flagged_events += 1;

                for flag in &event.aml_flags {
                    match flag {
                        AmlFlag::LargeAmount => report.large_amount_count += 1,
                        AmlFlag::UnusualPattern => report.unusual_pattern_count += 1,
                        AmlFlag::HighRiskCountry => report.high_risk_country_count += 1,
                        _ => {}
                    }

                    report
                        .events_by_flag
                        .entry(flag.as_str().to_string())
                        .or_insert_with(Vec::new)
                        .push(event.clone());
                }
            }
        }

        report
    }

    /// Summary text
    pub fn summary(&self) -> String {
        format!(
            "AML Report:\n\
             - Total events: {}\n\
             - Flagged events: {} ({:.1}%)\n\
             - Large amount: {}\n\
             - Unusual pattern: {}\n\
             - High risk country: {}",
            self.total_events,
            self.flagged_events,
            (self.flagged_events as f64 / self.total_events.max(1) as f64) * 100.0,
            self.large_amount_count,
            self.unusual_pattern_count,
            self.high_risk_country_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventStore;
    use rust_decimal_macros::dec;
    use tempfile::tempdir;

    #[test]
    fn test_event_reader() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path()).unwrap();

        // Write some events
        let event1 = Event::deposit(&store.next_event_id(), "CUST_001", "ACC_001", dec!(100), "USDT");
        let event2 = Event::withdrawal(&store.next_event_id(), "CUST_001", "ACC_001", dec!(50), "USDT");
        store.append(&event1).unwrap();
        store.append(&event2).unwrap();
        store.flush().unwrap();

        // Read back
        let reader = EventReader::new(dir.path());
        let events = reader.read_all().unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, EventType::Deposit);
        assert_eq!(events[1].event_type, EventType::Withdrawal);
    }

    #[test]
    fn test_event_filter() {
        let event1 = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(100), "USDT");
        let event2 = Event::deposit("EVT_002", "CUST_002", "ACC_002", dec!(200), "USDT");
        let event3 = Event::withdrawal("EVT_003", "CUST_001", "ACC_001", dec!(50), "USDT");

        let events = vec![event1, event2, event3];

        // Filter by account
        let filter = EventFilter::new().account("ACC_001");
        let filtered = filter.apply(events.clone());
        assert_eq!(filtered.len(), 2);

        // Filter by actor
        let filter = EventFilter::new().actor("CUST_002");
        let filtered = filter.apply(events.clone());
        assert_eq!(filtered.len(), 1);

        // Filter by event type
        let filter = EventFilter::new().event_types(vec![EventType::Withdrawal]);
        let filtered = filter.apply(events.clone());
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_aml_report() {
        let event1 = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(100000), "USDT")
            .with_aml_flag(AmlFlag::LargeAmount);

        let event2 = Event::deposit("EVT_002", "CUST_002", "ACC_002", dec!(200), "USDT")
            .with_aml_flag(AmlFlag::UnusualPattern);

        let event3 = Event::withdrawal("EVT_003", "CUST_001", "ACC_001", dec!(50), "USDT");

        let events = vec![event1, event2, event3];
        let report = AmlReport::generate(&events);

        assert_eq!(report.total_events, 3);
        assert_eq!(report.flagged_events, 2);
        assert_eq!(report.large_amount_count, 1);
        assert_eq!(report.unusual_pattern_count, 1);
    }
}
