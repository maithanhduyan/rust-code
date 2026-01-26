//! In-memory compliance state with sliding window
//!
//! Provides O(1) queries for velocity checks like:
//! - `user.transactions_in_last(1.hour)`
//! - `user.total_volume_in_last(1.hour)`
//!
//! Uses circular buffer with minute-granularity buckets.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Number of buckets (1 per minute for 60-minute window)
const BUCKET_COUNT: usize = 60;

/// In-memory state for fast compliance checks
///
/// Rebuilt from Compliance Ledger events on startup.
/// All queries are O(1).
#[derive(Debug, Default)]
pub struct ComplianceState {
    /// Sliding window aggregates per user
    windows: HashMap<String, TransactionWindow>,
}

/// Sliding window for a single user
#[derive(Debug)]
pub struct TransactionWindow {
    /// Circular buffer: each bucket = 1 minute
    buckets: [Bucket; BUCKET_COUNT],
    /// Last update timestamp (for bucket rotation)
    last_update: DateTime<Utc>,
}

/// Single time bucket (1 minute of data)
#[derive(Debug, Default, Clone)]
pub struct Bucket {
    /// Transaction count in this bucket
    pub tx_count: u32,
    /// Volume per asset in this bucket
    pub volume: HashMap<String, Decimal>,
}

impl Default for TransactionWindow {
    fn default() -> Self {
        Self {
            buckets: std::array::from_fn(|_| Bucket::default()),
            last_update: Utc::now(),
        }
    }
}

impl TransactionWindow {
    /// Get the bucket index for a given timestamp
    fn bucket_index(timestamp: DateTime<Utc>) -> usize {
        let minutes = timestamp.timestamp() / 60;
        (minutes as usize) % BUCKET_COUNT
    }

    /// Rotate buckets to current time, clearing expired ones
    fn rotate_to_now(&mut self, now: DateTime<Utc>) {
        let last_idx = Self::bucket_index(self.last_update);
        let current_idx = Self::bucket_index(now);

        // Calculate how many minutes have passed
        let elapsed_minutes = (now - self.last_update).num_minutes();

        if elapsed_minutes >= BUCKET_COUNT as i64 {
            // All buckets expired, clear everything
            for bucket in &mut self.buckets {
                *bucket = Bucket::default();
            }
        } else if elapsed_minutes > 0 {
            // Clear buckets between last update and now
            let mut idx = (last_idx + 1) % BUCKET_COUNT;
            let count = elapsed_minutes.min(BUCKET_COUNT as i64) as usize;

            for _ in 0..count {
                self.buckets[idx] = Bucket::default();
                idx = (idx + 1) % BUCKET_COUNT;
            }
        }

        self.last_update = now;
        // Clear current bucket if it's a new minute
        if last_idx != current_idx {
            self.buckets[current_idx] = Bucket::default();
        }
    }

    /// Record a transaction
    pub fn record(&mut self, asset: &str, amount: Decimal, timestamp: DateTime<Utc>) {
        self.rotate_to_now(timestamp);
        let idx = Self::bucket_index(timestamp);

        self.buckets[idx].tx_count += 1;
        *self.buckets[idx]
            .volume
            .entry(asset.to_string())
            .or_insert(Decimal::ZERO) += amount;
    }

    /// Get transaction count in last N minutes
    pub fn tx_count_in_last(&self, minutes: u32, now: DateTime<Utc>) -> u32 {
        let minutes = minutes.min(BUCKET_COUNT as u32) as usize;
        let current_idx = Self::bucket_index(now);
        let cutoff = now - Duration::minutes(minutes as i64);

        let mut count = 0;
        for i in 0..minutes {
            let idx = (current_idx + BUCKET_COUNT - i) % BUCKET_COUNT;
            let bucket_time = now - Duration::minutes(i as i64);

            // Only count if bucket is within our window
            if bucket_time >= cutoff && bucket_time <= now {
                // Check if bucket is not stale
                let bucket_age = (now - self.last_update).num_minutes();
                if bucket_age < BUCKET_COUNT as i64 {
                    count += self.buckets[idx].tx_count;
                }
            }
        }
        count
    }

    /// Get total volume for an asset in last N minutes
    pub fn volume_in_last(&self, asset: &str, minutes: u32, now: DateTime<Utc>) -> Decimal {
        let minutes = minutes.min(BUCKET_COUNT as u32) as usize;
        let current_idx = Self::bucket_index(now);
        let cutoff = now - Duration::minutes(minutes as i64);

        let mut total = Decimal::ZERO;
        for i in 0..minutes {
            let idx = (current_idx + BUCKET_COUNT - i) % BUCKET_COUNT;
            let bucket_time = now - Duration::minutes(i as i64);

            if bucket_time >= cutoff && bucket_time <= now {
                let bucket_age = (now - self.last_update).num_minutes();
                if bucket_age < BUCKET_COUNT as i64 {
                    if let Some(vol) = self.buckets[idx].volume.get(asset) {
                        total += vol;
                    }
                }
            }
        }
        total
    }
}

impl ComplianceState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a transaction for a user
    pub fn record_transaction(
        &mut self,
        user_id: &str,
        asset: &str,
        amount: Decimal,
    ) {
        self.record_transaction_at(user_id, asset, amount, Utc::now());
    }

    /// Record a transaction at a specific time (for replay)
    pub fn record_transaction_at(
        &mut self,
        user_id: &str,
        asset: &str,
        amount: Decimal,
        timestamp: DateTime<Utc>,
    ) {
        self.windows
            .entry(user_id.to_string())
            .or_default()
            .record(asset, amount, timestamp);
    }

    /// Get transaction count in last N minutes for a user
    pub fn tx_count_in_last(&self, user_id: &str, minutes: u32) -> u32 {
        self.windows
            .get(user_id)
            .map(|w| w.tx_count_in_last(minutes, Utc::now()))
            .unwrap_or(0)
    }

    /// Get total volume in last N minutes for a user and asset
    pub fn volume_in_last(&self, user_id: &str, asset: &str, minutes: u32) -> Decimal {
        self.windows
            .get(user_id)
            .map(|w| w.volume_in_last(asset, minutes, Utc::now()))
            .unwrap_or(Decimal::ZERO)
    }

    /// Check if user has any recorded transactions
    pub fn has_user(&self, user_id: &str) -> bool {
        self.windows.contains_key(user_id)
    }

    /// Get number of tracked users
    pub fn user_count(&self) -> usize {
        self.windows.len()
    }

    /// Clear all state (for testing)
    pub fn clear(&mut self) {
        self.windows.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_empty_state() {
        let state = ComplianceState::new();
        assert_eq!(state.tx_count_in_last("USER-001", 60), 0);
        assert_eq!(state.volume_in_last("USER-001", "USDT", 60), Decimal::ZERO);
    }

    #[test]
    fn test_record_single_transaction() {
        let mut state = ComplianceState::new();

        state.record_transaction("USER-001", "USDT", dec!(1000));

        assert_eq!(state.tx_count_in_last("USER-001", 60), 1);
        assert_eq!(state.volume_in_last("USER-001", "USDT", 60), dec!(1000));
        assert_eq!(state.volume_in_last("USER-001", "BTC", 60), Decimal::ZERO);
    }

    #[test]
    fn test_record_multiple_transactions() {
        let mut state = ComplianceState::new();

        state.record_transaction("USER-001", "USDT", dec!(1000));
        state.record_transaction("USER-001", "USDT", dec!(2000));
        state.record_transaction("USER-001", "BTC", dec!(0.5));

        assert_eq!(state.tx_count_in_last("USER-001", 60), 3);
        assert_eq!(state.volume_in_last("USER-001", "USDT", 60), dec!(3000));
        assert_eq!(state.volume_in_last("USER-001", "BTC", 60), dec!(0.5));
    }

    #[test]
    fn test_multiple_users() {
        let mut state = ComplianceState::new();

        state.record_transaction("USER-001", "USDT", dec!(1000));
        state.record_transaction("USER-002", "USDT", dec!(5000));

        assert_eq!(state.tx_count_in_last("USER-001", 60), 1);
        assert_eq!(state.tx_count_in_last("USER-002", 60), 1);
        assert_eq!(state.volume_in_last("USER-001", "USDT", 60), dec!(1000));
        assert_eq!(state.volume_in_last("USER-002", "USDT", 60), dec!(5000));
    }

    #[test]
    fn test_bucket_index() {
        let t1 = Utc::now();
        let t2 = t1 + Duration::minutes(1);

        let idx1 = TransactionWindow::bucket_index(t1);
        let idx2 = TransactionWindow::bucket_index(t2);

        // Different minutes should have different indices
        assert_ne!(idx1, idx2);
        // Index should wrap around
        assert!(idx1 < BUCKET_COUNT);
        assert!(idx2 < BUCKET_COUNT);
    }

    #[test]
    fn test_has_user() {
        let mut state = ComplianceState::new();

        assert!(!state.has_user("USER-001"));

        state.record_transaction("USER-001", "USDT", dec!(100));

        assert!(state.has_user("USER-001"));
        assert!(!state.has_user("USER-002"));
    }

    #[test]
    fn test_user_count() {
        let mut state = ComplianceState::new();

        assert_eq!(state.user_count(), 0);

        state.record_transaction("USER-001", "USDT", dec!(100));
        assert_eq!(state.user_count(), 1);

        state.record_transaction("USER-002", "USDT", dec!(200));
        assert_eq!(state.user_count(), 2);

        state.record_transaction("USER-001", "USDT", dec!(300));
        assert_eq!(state.user_count(), 2); // Still 2, not 3
    }

    #[test]
    fn test_clear() {
        let mut state = ComplianceState::new();

        state.record_transaction("USER-001", "USDT", dec!(100));
        state.record_transaction("USER-002", "USDT", dec!(200));

        state.clear();

        assert_eq!(state.user_count(), 0);
        assert_eq!(state.tx_count_in_last("USER-001", 60), 0);
    }

    #[test]
    fn test_window_time_based_query() {
        let mut window = TransactionWindow::default();
        let now = Utc::now();

        // Record at current time
        window.record("USDT", dec!(100), now);

        // Should find it in last 60 minutes
        assert_eq!(window.tx_count_in_last(60, now), 1);
        assert_eq!(window.volume_in_last("USDT", 60, now), dec!(100));

        // Should find it in last 5 minutes
        assert_eq!(window.tx_count_in_last(5, now), 1);
    }
}
