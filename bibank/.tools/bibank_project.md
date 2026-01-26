---
date: 2026-01-26 18:34:13 
---

# Cấu trúc Dự án như sau:

```
..\bibank
├── Cargo.toml
├── crates
│   ├── bus
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── channel.rs
│   │       ├── error.rs
│   │       ├── event.rs
│   │       ├── lib.rs
│   │       └── subscriber.rs
│   ├── core
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── amount.rs
│   │       ├── currency.rs
│   │       └── lib.rs
│   ├── dsl
│   │   ├── Cargo.toml
│   │   └── src
│   │       └── lib.rs
│   ├── events
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       ├── reader.rs
│   │       └── store.rs
│   ├── ledger
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── account.rs
│   │       ├── entry.rs
│   │       ├── error.rs
│   │       ├── hash.rs
│   │       ├── lib.rs
│   │       ├── signature.rs
│   │       └── validation.rs
│   ├── projection
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── balance.rs
│   │       ├── engine.rs
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       └── trade.rs
│   ├── risk
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── engine.rs
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       └── state.rs
│   └── rpc
│       ├── Cargo.toml
│       └── src
│           ├── commands.rs
│           ├── context.rs
│           ├── lib.rs
│           └── main.rs
├── data-test
│   ├── journal
│   │   └── 2026-01-26.jsonl
│   ├── projection.db
│   └── system.key
└── data-test2
    ├── journal
    │   └── 2026-01-26.jsonl
    └── projection.db
```

# Danh sách chi tiết các file:

## File ..\bibank\crates\bus\src\channel.rs:
```rust
//! Async event bus implementation
//!
//! Phase 2: Async broadcast channel with subscriber management

use crate::error::BusError;
use crate::event::LedgerEvent;
use crate::subscriber::EventSubscriber;
use bibank_events::EventReader;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Default channel capacity
const DEFAULT_CAPACITY: usize = 1024;

/// Async event bus for distributing ledger events
///
/// The bus uses a broadcast channel for non-blocking event distribution.
/// Subscribers receive events asynchronously and can fail independently.
pub struct EventBus {
    /// Journal path for replay
    journal_path: std::path::PathBuf,
    /// Broadcast sender
    sender: broadcast::Sender<LedgerEvent>,
    /// Registered subscribers
    subscribers: Vec<Arc<dyn EventSubscriber>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new(journal_path: impl AsRef<Path>) -> Self {
        let (sender, _) = broadcast::channel(DEFAULT_CAPACITY);
        Self {
            journal_path: journal_path.as_ref().to_path_buf(),
            sender,
            subscribers: Vec::new(),
        }
    }

    /// Create with custom capacity
    pub fn with_capacity(journal_path: impl AsRef<Path>, capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            journal_path: journal_path.as_ref().to_path_buf(),
            sender,
            subscribers: Vec::new(),
        }
    }

    /// Register a subscriber
    pub fn subscribe(&mut self, subscriber: Arc<dyn EventSubscriber>) {
        info!("Registering subscriber: {}", subscriber.name());
        self.subscribers.push(subscriber);
    }

    /// Publish an event to all subscribers
    ///
    /// This is non-blocking - the event is sent to the broadcast channel
    /// and subscribers process it asynchronously.
    pub async fn publish(&self, event: LedgerEvent) -> Result<(), BusError> {
        debug!("Publishing event to {} subscribers", self.subscribers.len());

        // Send to broadcast channel (for any channel receivers)
        let _ = self.sender.send(event.clone());

        // Also directly notify registered subscribers
        for subscriber in &self.subscribers {
            if let Err(e) = subscriber.handle(&event).await {
                // Log but don't fail - subscriber failures don't block ledger
                warn!(
                    "Subscriber '{}' failed to handle event: {}",
                    subscriber.name(),
                    e
                );
            }
        }

        Ok(())
    }

    /// Replay events from a sequence number
    ///
    /// Reads from JSONL files and publishes to subscribers.
    pub async fn replay_from(&self, from_sequence: u64) -> Result<usize, BusError> {
        info!("Starting replay from sequence {}", from_sequence);

        // Notify subscribers of replay start
        let start_event = LedgerEvent::replay_started(from_sequence);
        for subscriber in &self.subscribers {
            if let Err(e) = subscriber.on_replay_start().await {
                error!("Subscriber '{}' failed on replay start: {}", subscriber.name(), e);
            }
        }
        let _ = self.sender.send(start_event);

        // Read entries from JSONL
        let reader = EventReader::from_directory(&self.journal_path)?;
        let entries = reader.read_all()?;

        let mut count = 0;
        for entry in entries {
            if entry.sequence >= from_sequence {
                let event = LedgerEvent::entry_committed(entry);
                self.publish(event).await?;
                count += 1;
            }
        }

        // Notify subscribers of replay complete
        let complete_event = LedgerEvent::replay_completed(count);
        for subscriber in &self.subscribers {
            if let Err(e) = subscriber.on_replay_complete().await {
                error!("Subscriber '{}' failed on replay complete: {}", subscriber.name(), e);
            }
        }
        let _ = self.sender.send(complete_event);

        info!("Replay completed: {} entries", count);
        Ok(count)
    }

    /// Get a receiver for the broadcast channel
    ///
    /// This can be used by external consumers that want to receive events.
    pub fn receiver(&self) -> broadcast::Receiver<LedgerEvent> {
        self.sender.subscribe()
    }

    /// Get an event reader for direct JSONL access
    pub fn reader(&self) -> Result<EventReader, bibank_events::EventError> {
        EventReader::from_directory(&self.journal_path)
    }

    /// Get the journal path
    pub fn journal_path(&self) -> &Path {
        &self.journal_path
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    struct CountingSubscriber {
        name: String,
        count: AtomicUsize,
    }

    impl CountingSubscriber {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                count: AtomicUsize::new(0),
            }
        }

        fn count(&self) -> usize {
            self.count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl EventSubscriber for CountingSubscriber {
        fn name(&self) -> &str {
            &self.name
        }

        async fn handle(&self, _event: &LedgerEvent) -> Result<(), BusError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_publish_to_subscriber() {
        let dir = tempdir().unwrap();
        let mut bus = EventBus::new(dir.path());

        let subscriber = Arc::new(CountingSubscriber::new("test"));
        bus.subscribe(subscriber.clone());

        // Create a dummy event
        let event = LedgerEvent::ChainVerified {
            last_sequence: 1,
            last_hash: "abc".to_string(),
        };

        bus.publish(event).await.unwrap();

        assert_eq!(subscriber.count(), 1);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let dir = tempdir().unwrap();
        let mut bus = EventBus::new(dir.path());

        let sub1 = Arc::new(CountingSubscriber::new("sub1"));
        let sub2 = Arc::new(CountingSubscriber::new("sub2"));

        bus.subscribe(sub1.clone());
        bus.subscribe(sub2.clone());

        let event = LedgerEvent::ChainVerified {
            last_sequence: 1,
            last_hash: "abc".to_string(),
        };

        bus.publish(event).await.unwrap();

        assert_eq!(sub1.count(), 1);
        assert_eq!(sub2.count(), 1);
    }

    struct FailingSubscriber;

    #[async_trait]
    impl EventSubscriber for FailingSubscriber {
        fn name(&self) -> &str {
            "failing"
        }

        async fn handle(&self, _event: &LedgerEvent) -> Result<(), BusError> {
            Err(BusError::SubscriberFailed {
                name: "failing".to_string(),
                reason: "intentional failure".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn test_subscriber_failure_does_not_block() {
        let dir = tempdir().unwrap();
        let mut bus = EventBus::new(dir.path());

        let failing = Arc::new(FailingSubscriber);
        let counting = Arc::new(CountingSubscriber::new("counting"));

        bus.subscribe(failing);
        bus.subscribe(counting.clone());

        let event = LedgerEvent::ChainVerified {
            last_sequence: 1,
            last_hash: "abc".to_string(),
        };

        // Should not error even though one subscriber fails
        let result = bus.publish(event).await;
        assert!(result.is_ok());

        // The counting subscriber should still have received the event
        assert_eq!(counting.count(), 1);
    }
}

```

## File ..\bibank\crates\bus\src\error.rs:
```rust
//! Event bus errors

use thiserror::Error;

/// Errors that can occur in the event bus
#[derive(Error, Debug)]
pub enum BusError {
    #[error("Failed to send event: {0}")]
    SendFailed(String),

    #[error("Subscriber '{name}' failed: {reason}")]
    SubscriberFailed { name: String, reason: String },

    #[error("Replay failed: {0}")]
    ReplayFailed(String),

    #[error("Event store error: {0}")]
    EventStoreError(#[from] bibank_events::EventError),

    #[error("Channel closed")]
    ChannelClosed,
}

```

## File ..\bibank\crates\bus\src\event.rs:
```rust
//! Ledger events for pub/sub distribution

use bibank_ledger::JournalEntry;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events emitted by the ledger system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LedgerEvent {
    /// A journal entry was committed
    EntryCommitted {
        /// The committed entry
        entry: JournalEntry,
        /// When the event was published
        timestamp: DateTime<Utc>,
    },

    /// Hash chain was verified
    ChainVerified {
        /// Last verified sequence number
        last_sequence: u64,
        /// Hash of the last entry
        last_hash: String,
    },

    /// Replay has started
    ReplayStarted {
        /// Starting sequence number
        from_sequence: u64,
    },

    /// Replay has completed
    ReplayCompleted {
        /// Number of entries replayed
        entries_count: usize,
    },
}

impl LedgerEvent {
    /// Create an EntryCommitted event
    pub fn entry_committed(entry: JournalEntry) -> Self {
        Self::EntryCommitted {
            entry,
            timestamp: Utc::now(),
        }
    }

    /// Create a ChainVerified event
    pub fn chain_verified(last_sequence: u64, last_hash: String) -> Self {
        Self::ChainVerified {
            last_sequence,
            last_hash,
        }
    }

    /// Create a ReplayStarted event
    pub fn replay_started(from_sequence: u64) -> Self {
        Self::ReplayStarted { from_sequence }
    }

    /// Create a ReplayCompleted event
    pub fn replay_completed(entries_count: usize) -> Self {
        Self::ReplayCompleted { entries_count }
    }
}

```

## File ..\bibank\crates\bus\src\lib.rs:
```rust
//! BiBank Event Bus - In-process async event distribution
//!
//! Distributes committed events to subscribers (projections, etc.)
//!
//! # Phase 2 Features
//! - Async pub/sub with tokio broadcast channel
//! - EventSubscriber trait for custom handlers
//! - Replay from JSONL (Source of Truth)
//! - No retention in bus - events only in JSONL

pub mod channel;
pub mod error;
pub mod event;
pub mod subscriber;

pub use channel::EventBus;
pub use error::BusError;
pub use event::LedgerEvent;
pub use subscriber::EventSubscriber;

```

## File ..\bibank\crates\bus\src\subscriber.rs:
```rust
//! Event subscriber trait for async event handling

use crate::event::LedgerEvent;
use crate::error::BusError;
use async_trait::async_trait;

/// Trait for event subscribers
///
/// Subscribers receive events from the event bus and process them asynchronously.
/// Each subscriber should be idempotent (handle duplicate events gracefully).
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Get the subscriber name (for logging)
    fn name(&self) -> &str;

    /// Handle a ledger event
    ///
    /// This is called for each event published to the bus.
    /// The subscriber should process the event and return Ok(()) on success.
    async fn handle(&self, event: &LedgerEvent) -> Result<(), BusError>;

    /// Called when replay starts (optional)
    ///
    /// Subscribers can use this to prepare for bulk event processing.
    async fn on_replay_start(&self) -> Result<(), BusError> {
        Ok(())
    }

    /// Called when replay completes (optional)
    ///
    /// Subscribers can use this to finalize state after bulk processing.
    async fn on_replay_complete(&self) -> Result<(), BusError> {
        Ok(())
    }

    /// Get the last processed sequence (for replay optimization)
    ///
    /// Returns None if the subscriber doesn't track sequence numbers.
    fn last_processed_sequence(&self) -> Option<u64> {
        None
    }
}

```

## File ..\bibank\crates\core\src\amount.rs:
```rust
//! Amount - Non-negative decimal wrapper for financial amounts
//!
//! All financial amounts in BiBank MUST be non-negative.
//! This is enforced at the type level.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Errors that can occur when working with amounts
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum AmountError {
    #[error("Amount cannot be negative: {0}")]
    NegativeAmount(Decimal),
}

/// A non-negative decimal amount for financial operations.
///
/// # Invariant
/// The inner value is always >= 0. This is enforced by the constructor.
///
/// # Example
/// ```
/// use bibank_core::Amount;
/// use rust_decimal::Decimal;
///
/// let amount = Amount::new(Decimal::new(100, 0)).unwrap();
/// assert_eq!(amount.value(), Decimal::new(100, 0));
///
/// // Negative amounts are rejected
/// let negative = Amount::new(Decimal::new(-100, 0));
/// assert!(negative.is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "Decimal", into = "Decimal")]
pub struct Amount(Decimal);

impl Amount {
    /// Zero amount constant
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Create a new Amount from a Decimal.
    ///
    /// Returns an error if the value is negative.
    pub fn new(value: Decimal) -> Result<Self, AmountError> {
        if value < Decimal::ZERO {
            Err(AmountError::NegativeAmount(value))
        } else {
            Ok(Self(value))
        }
    }

    /// Create an Amount without validation.
    ///
    /// # Safety
    /// The caller MUST ensure the value is non-negative.
    /// Use only for trusted sources (e.g., deserialization from validated storage).
    #[inline]
    pub const fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    /// Get the inner Decimal value
    #[inline]
    pub const fn value(&self) -> Decimal {
        self.0
    }

    /// Check if the amount is zero
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Saturating addition - returns the sum or panics on overflow
    pub fn checked_add(&self, other: &Amount) -> Option<Amount> {
        self.0.checked_add(other.0).map(Amount)
    }

    /// Saturating subtraction - returns None if result would be negative
    pub fn checked_sub(&self, other: &Amount) -> Option<Amount> {
        let result = self.0.checked_sub(other.0)?;
        if result < Decimal::ZERO {
            None
        } else {
            Some(Amount(result))
        }
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<Decimal> for Amount {
    type Error = AmountError;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Amount> for Decimal {
    fn from(amount: Amount) -> Self {
        amount.0
    }
}

impl Default for Amount {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_positive() {
        let amount = Amount::new(Decimal::new(100, 0)).unwrap();
        assert_eq!(amount.value(), Decimal::new(100, 0));
    }

    #[test]
    fn test_amount_zero() {
        let amount = Amount::new(Decimal::ZERO).unwrap();
        assert!(amount.is_zero());
    }

    #[test]
    fn test_amount_negative_rejected() {
        let result = Amount::new(Decimal::new(-100, 0));
        assert!(matches!(result, Err(AmountError::NegativeAmount(_))));
    }

    #[test]
    fn test_checked_sub_prevents_negative() {
        let a = Amount::new(Decimal::new(50, 0)).unwrap();
        let b = Amount::new(Decimal::new(100, 0)).unwrap();
        assert!(a.checked_sub(&b).is_none());
    }

    #[test]
    fn test_checked_sub_success() {
        let a = Amount::new(Decimal::new(100, 0)).unwrap();
        let b = Amount::new(Decimal::new(30, 0)).unwrap();
        let result = a.checked_sub(&b).unwrap();
        assert_eq!(result.value(), Decimal::new(70, 0));
    }

    #[test]
    fn test_serde_roundtrip() {
        let amount = Amount::new(Decimal::new(12345, 2)).unwrap(); // 123.45
        let json = serde_json::to_string(&amount).unwrap();
        let parsed: Amount = serde_json::from_str(&json).unwrap();
        assert_eq!(amount, parsed);
    }
}

```

## File ..\bibank\crates\core\src\currency.rs:
```rust
//! Currency - Type-safe currency/asset codes
//!
//! Instead of raw strings, we use an enum for common currencies
//! and a fallback for custom tokens.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur when parsing currencies
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CurrencyError {
    #[error("Empty currency code")]
    EmptyCode,

    #[error("Currency code too long (max 10 chars): {0}")]
    TooLong(String),

    #[error("Invalid currency code format: {0}")]
    InvalidFormat(String),
}

/// Currency/Asset codes
///
/// Common currencies are pre-defined for type safety and performance.
/// Custom tokens use the `Other` variant.
///
/// # Examples
/// ```
/// use bibank_core::Currency;
///
/// let usdt: Currency = "USDT".parse().unwrap();
/// assert_eq!(usdt, Currency::Usdt);
///
/// let btc = Currency::Btc;
/// assert_eq!(btc.to_string(), "BTC");
///
/// // Custom token
/// let custom: Currency = "MYTOKEN".parse().unwrap();
/// assert!(matches!(custom, Currency::Other(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Currency {
    // === Stablecoins ===
    /// Tether USD
    Usdt,
    /// USD Coin
    Usdc,
    /// Binance USD
    Busd,
    /// Dai
    Dai,

    // === Major Crypto ===
    /// Bitcoin
    Btc,
    /// Ethereum
    Eth,
    /// Binance Coin
    Bnb,
    /// Solana
    Sol,
    /// XRP
    Xrp,
    /// Cardano
    Ada,
    /// Dogecoin
    Doge,
    /// Polygon
    Matic,
    /// Litecoin
    Ltc,

    // === Fiat ===
    /// US Dollar
    Usd,
    /// Euro
    Eur,
    /// British Pound
    Gbp,
    /// Japanese Yen
    Jpy,
    /// Vietnamese Dong
    Vnd,

    // === Custom tokens ===
    /// Any other token/currency
    Other(String),
}

impl Currency {
    /// Returns the currency code as a string slice
    pub fn code(&self) -> &str {
        match self {
            // Stablecoins
            Currency::Usdt => "USDT",
            Currency::Usdc => "USDC",
            Currency::Busd => "BUSD",
            Currency::Dai => "DAI",

            // Crypto
            Currency::Btc => "BTC",
            Currency::Eth => "ETH",
            Currency::Bnb => "BNB",
            Currency::Sol => "SOL",
            Currency::Xrp => "XRP",
            Currency::Ada => "ADA",
            Currency::Doge => "DOGE",
            Currency::Matic => "MATIC",
            Currency::Ltc => "LTC",

            // Fiat
            Currency::Usd => "USD",
            Currency::Eur => "EUR",
            Currency::Gbp => "GBP",
            Currency::Jpy => "JPY",
            Currency::Vnd => "VND",

            // Other
            Currency::Other(s) => s.as_str(),
        }
    }

    /// Returns true if this is a stablecoin
    pub fn is_stablecoin(&self) -> bool {
        matches!(
            self,
            Currency::Usdt | Currency::Usdc | Currency::Busd | Currency::Dai
        )
    }

    /// Returns true if this is fiat currency
    pub fn is_fiat(&self) -> bool {
        matches!(
            self,
            Currency::Usd | Currency::Eur | Currency::Gbp | Currency::Jpy | Currency::Vnd
        )
    }

    /// Returns true if this is a major cryptocurrency
    pub fn is_crypto(&self) -> bool {
        matches!(
            self,
            Currency::Btc
                | Currency::Eth
                | Currency::Bnb
                | Currency::Sol
                | Currency::Xrp
                | Currency::Ada
                | Currency::Doge
                | Currency::Matic
                | Currency::Ltc
        )
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl FromStr for Currency {
    type Err = CurrencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_uppercase();

        if s.is_empty() {
            return Err(CurrencyError::EmptyCode);
        }

        if s.len() > 10 {
            return Err(CurrencyError::TooLong(s));
        }

        // Validate: only alphanumeric
        if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(CurrencyError::InvalidFormat(s));
        }

        Ok(match s.as_str() {
            // Stablecoins
            "USDT" => Currency::Usdt,
            "USDC" => Currency::Usdc,
            "BUSD" => Currency::Busd,
            "DAI" => Currency::Dai,

            // Crypto
            "BTC" => Currency::Btc,
            "ETH" => Currency::Eth,
            "BNB" => Currency::Bnb,
            "SOL" => Currency::Sol,
            "XRP" => Currency::Xrp,
            "ADA" => Currency::Ada,
            "DOGE" => Currency::Doge,
            "MATIC" => Currency::Matic,
            "LTC" => Currency::Ltc,

            // Fiat
            "USD" => Currency::Usd,
            "EUR" => Currency::Eur,
            "GBP" => Currency::Gbp,
            "JPY" => Currency::Jpy,
            "VND" => Currency::Vnd,

            // Other
            _ => Currency::Other(s),
        })
    }
}

impl TryFrom<String> for Currency {
    type Error = CurrencyError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Currency> for String {
    fn from(c: Currency) -> Self {
        c.code().to_string()
    }
}

impl From<&str> for Currency {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or_else(|_| Currency::Other(s.to_uppercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_known_currencies() {
        assert_eq!("USDT".parse::<Currency>().unwrap(), Currency::Usdt);
        assert_eq!("btc".parse::<Currency>().unwrap(), Currency::Btc);
        assert_eq!("ETH".parse::<Currency>().unwrap(), Currency::Eth);
        assert_eq!("usd".parse::<Currency>().unwrap(), Currency::Usd);
    }

    #[test]
    fn test_parse_custom_token() {
        let custom: Currency = "MYTOKEN".parse().unwrap();
        assert_eq!(custom, Currency::Other("MYTOKEN".to_string()));
        assert_eq!(custom.to_string(), "MYTOKEN");
    }

    #[test]
    fn test_display() {
        assert_eq!(Currency::Usdt.to_string(), "USDT");
        assert_eq!(Currency::Btc.to_string(), "BTC");
        assert_eq!(Currency::Other("XYZ".to_string()).to_string(), "XYZ");
    }

    #[test]
    fn test_is_stablecoin() {
        assert!(Currency::Usdt.is_stablecoin());
        assert!(Currency::Usdc.is_stablecoin());
        assert!(!Currency::Btc.is_stablecoin());
        assert!(!Currency::Usd.is_stablecoin());
    }

    #[test]
    fn test_is_fiat() {
        assert!(Currency::Usd.is_fiat());
        assert!(Currency::Vnd.is_fiat());
        assert!(!Currency::Usdt.is_fiat());
        assert!(!Currency::Btc.is_fiat());
    }

    #[test]
    fn test_is_crypto() {
        assert!(Currency::Btc.is_crypto());
        assert!(Currency::Eth.is_crypto());
        assert!(!Currency::Usdt.is_crypto());
        assert!(!Currency::Usd.is_crypto());
    }

    #[test]
    fn test_empty_code_error() {
        let result: Result<Currency, _> = "".parse();
        assert!(matches!(result, Err(CurrencyError::EmptyCode)));
    }

    #[test]
    fn test_too_long_error() {
        let result: Result<Currency, _> = "VERYLONGCURRENCYNAME".parse();
        assert!(matches!(result, Err(CurrencyError::TooLong(_))));
    }

    #[test]
    fn test_invalid_format_error() {
        let result: Result<Currency, _> = "BTC-USD".parse();
        assert!(matches!(result, Err(CurrencyError::InvalidFormat(_))));
    }

    #[test]
    fn test_serde_roundtrip() {
        let currencies = vec![
            Currency::Usdt,
            Currency::Btc,
            Currency::Usd,
            Currency::Other("XYZ".to_string()),
        ];

        for currency in currencies {
            let json = serde_json::to_string(&currency).unwrap();
            let parsed: Currency = serde_json::from_str(&json).unwrap();
            assert_eq!(currency, parsed);
        }
    }

    #[test]
    fn test_from_str_trait() {
        let currency: Currency = "ETH".into();
        assert_eq!(currency, Currency::Eth);
    }
}

```

## File ..\bibank\crates\core\src\lib.rs:
```rust
//! BiBank Core - Domain types
//!
//! This crate contains the fundamental types used across BiBank:
//! - `Amount`: Non-negative decimal wrapper for financial amounts
//! - `Currency`: Type-safe currency/asset codes

pub mod amount;
pub mod currency;

pub use amount::Amount;
pub use currency::Currency;

```

## File ..\bibank\crates\dsl\src\lib.rs:
```rust
//! BiBank DSL - Domain specific language macros
//!
//! Phase 1: Placeholder
//! Phase 2+: banking_scenario! and rule! macros

// Placeholder for future DSL implementation
pub fn placeholder() {}

```

## File ..\bibank\crates\events\src\error.rs:
```rust
//! Event store errors

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EventError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Event store not initialized")]
    NotInitialized,

    #[error("Invalid event file: {0}")]
    InvalidFile(String),
}

```

## File ..\bibank\crates\events\src\lib.rs:
```rust
//! BiBank Events - JSONL event store
//!
//! This crate handles persistence of journal entries to JSONL files.
//! JSONL is the Source of Truth - SQLite projections are disposable.

pub mod error;
pub mod reader;
pub mod store;

pub use error::EventError;
pub use reader::EventReader;
pub use store::EventStore;

```

## File ..\bibank\crates\events\src\reader.rs:
```rust
//! JSONL event reader - sequential reader for replay

use crate::error::EventError;
use bibank_ledger::JournalEntry;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Sequential event reader for replay
pub struct EventReader {
    files: Vec<std::path::PathBuf>,
}

impl EventReader {
    /// Create a new reader from a directory
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, EventError> {
        let path = path.as_ref();
        let mut files = Vec::new();

        if path.exists() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let file_path = entry.path();
                if file_path.extension().map_or(false, |ext| ext == "jsonl") {
                    files.push(file_path);
                }
            }
        }

        files.sort();

        Ok(Self { files })
    }

    /// Read all entries from all files in order
    pub fn read_all(&self) -> Result<Vec<JournalEntry>, EventError> {
        let mut entries = Vec::new();

        for file_path in &self.files {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let entry: JournalEntry = serde_json::from_str(&line)?;
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Get the last sequence number from all files
    pub fn last_sequence(&self) -> Result<Option<u64>, EventError> {
        if self.files.is_empty() {
            return Ok(None);
        }

        // Read from the last file
        let last_file = &self.files[self.files.len() - 1];
        let file = File::open(last_file)?;
        let reader = BufReader::new(file);

        let mut last_seq = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: JournalEntry = serde_json::from_str(&line)?;
            last_seq = Some(entry.sequence);
        }

        Ok(last_seq)
    }

    /// Get the last entry (for prev_hash)
    pub fn last_entry(&self) -> Result<Option<JournalEntry>, EventError> {
        let entries = self.read_all()?;
        Ok(entries.into_iter().last())
    }

    /// Count total entries across all files
    pub fn count(&self) -> Result<usize, EventError> {
        let mut count = 0;

        for file_path in &self.files {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if !line.trim().is_empty() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

```

## File ..\bibank\crates\events\src\store.rs:
```rust
//! JSONL event store - append-only writer

use crate::error::EventError;
use bibank_ledger::JournalEntry;
use chrono::Utc;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Append-only JSONL event store
pub struct EventStore {
    base_path: PathBuf,
    current_file: Option<BufWriter<File>>,
    current_date: Option<String>,
}

impl EventStore {
    /// Create a new event store at the given path
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self, EventError> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;

        Ok(Self {
            base_path,
            current_file: None,
            current_date: None,
        })
    }

    /// Append a journal entry to the store
    pub fn append(&mut self, entry: &JournalEntry) -> Result<(), EventError> {
        let date = entry.timestamp.format("%Y-%m-%d").to_string();

        // Rotate file if date changed
        if self.current_date.as_ref() != Some(&date) {
            self.rotate_file(&date)?;
        }

        // Write entry as JSON line
        if let Some(ref mut writer) = self.current_file {
            let json = serde_json::to_string(entry)?;
            writeln!(writer, "{}", json)?;
            writer.flush()?;
        }

        Ok(())
    }

    /// Rotate to a new file for the given date
    fn rotate_file(&mut self, date: &str) -> Result<(), EventError> {
        // Flush current file
        if let Some(ref mut writer) = self.current_file {
            writer.flush()?;
        }

        // Open new file
        let file_path = self.base_path.join(format!("{}.jsonl", date));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        self.current_file = Some(BufWriter::new(file));
        self.current_date = Some(date.to_string());

        Ok(())
    }

    /// Get the path to today's file
    pub fn today_file_path(&self) -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        self.base_path.join(format!("{}.jsonl", date))
    }

    /// List all JSONL files in the store
    pub fn list_files(&self) -> Result<Vec<PathBuf>, EventError> {
        let mut files = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "jsonl") {
                files.push(path);
            }
        }

        files.sort();
        Ok(files)
    }

    /// Flush and close the current file
    pub fn close(&mut self) -> Result<(), EventError> {
        if let Some(ref mut writer) = self.current_file {
            writer.flush()?;
        }
        self.current_file = None;
        self.current_date = None;
        Ok(())
    }
}

impl Drop for EventStore {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

```

## File ..\bibank\crates\ledger\src\account.rs:
```rust
//! Ledger Account - Hierarchical account identifiers
//!
//! Format: CATEGORY:SEGMENT:ID:ASSET:SUB_ACCOUNT
//! Example: LIAB:USER:ALICE:USDT:AVAILABLE

use crate::entry::Side;
use crate::error::LedgerError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use strum_macros::{Display, EnumString};

/// Account category following standard accounting principles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountCategory {
    /// Assets - Resources owned by the system (Cash, Crypto vault)
    #[strum(serialize = "ASSET")]
    Asset,

    /// Liabilities - Obligations to users (User balances)
    #[strum(serialize = "LIAB")]
    Liability,

    /// Equity - Owner's stake in the system
    #[strum(serialize = "EQUITY")]
    Equity,

    /// Revenue - Income earned (Fees collected)
    #[strum(serialize = "REV")]
    Revenue,

    /// Expenses - Costs incurred
    #[strum(serialize = "EXP")]
    Expense,
}

impl AccountCategory {
    /// Returns the normal balance side for this category.
    ///
    /// - Assets and Expenses increase on Debit
    /// - Liabilities, Equity, and Revenue increase on Credit
    pub fn normal_balance(&self) -> Side {
        match self {
            AccountCategory::Asset | AccountCategory::Expense => Side::Debit,
            AccountCategory::Liability | AccountCategory::Equity | AccountCategory::Revenue => {
                Side::Credit
            }
        }
    }

    /// Short code for serialization
    pub fn code(&self) -> &'static str {
        match self {
            AccountCategory::Asset => "ASSET",
            AccountCategory::Liability => "LIAB",
            AccountCategory::Equity => "EQUITY",
            AccountCategory::Revenue => "REV",
            AccountCategory::Expense => "EXP",
        }
    }
}

/// Hierarchical ledger account key
///
/// Format: `CATEGORY:SEGMENT:ID:ASSET:SUB_ACCOUNT`
///
/// # Examples
/// - `ASSET:SYSTEM:VAULT:USDT:MAIN` - System USDT vault
/// - `LIAB:USER:ALICE:USDT:AVAILABLE` - Alice's available USDT balance
/// - `REV:SYSTEM:FEE:USDT:REVENUE` - USDT fee revenue
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountKey {
    /// Accounting category (ASSET, LIAB, EQUITY, REV, EXP)
    pub category: AccountCategory,

    /// Segment (USER, SYSTEM)
    pub segment: String,

    /// Entity identifier (ALICE, VAULT, FEE)
    pub id: String,

    /// Asset/currency code (USDT, BTC, USD)
    pub asset: String,

    /// Sub-account type (AVAILABLE, LOCKED, MAIN, VAULT, REVENUE)
    pub sub_account: String,
}

impl AccountKey {
    /// Create a new AccountKey
    pub fn new(
        category: AccountCategory,
        segment: impl Into<String>,
        id: impl Into<String>,
        asset: impl Into<String>,
        sub_account: impl Into<String>,
    ) -> Self {
        Self {
            category,
            segment: segment.into().to_uppercase(),
            id: id.into().to_uppercase(),
            asset: asset.into().to_uppercase(),
            sub_account: sub_account.into().to_uppercase(),
        }
    }

    /// Create a user available balance account
    pub fn user_available(user_id: impl Into<String>, asset: impl Into<String>) -> Self {
        Self::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "AVAILABLE",
        )
    }

    /// Create a user locked balance account
    pub fn user_locked(user_id: impl Into<String>, asset: impl Into<String>) -> Self {
        Self::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOCKED",
        )
    }

    /// Create a system vault account
    pub fn system_vault(asset: impl Into<String>) -> Self {
        Self::new(AccountCategory::Asset, "SYSTEM", "VAULT", asset, "MAIN")
    }

    /// Create a fee revenue account
    pub fn fee_revenue(asset: impl Into<String>) -> Self {
        Self::new(AccountCategory::Revenue, "SYSTEM", "FEE", asset, "REVENUE")
    }
}

impl fmt::Display for AccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.category.code(),
            self.segment,
            self.id,
            self.asset,
            self.sub_account
        )
    }
}

impl FromStr for AccountKey {
    type Err = LedgerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 5 {
            return Err(LedgerError::InvalidAccountFormat(format!(
                "Expected 5 parts separated by ':', got {}: {}",
                parts.len(),
                s
            )));
        }

        let category = match parts[0].to_uppercase().as_str() {
            "ASSET" => AccountCategory::Asset,
            "LIAB" => AccountCategory::Liability,
            "EQUITY" => AccountCategory::Equity,
            "REV" => AccountCategory::Revenue,
            "EXP" => AccountCategory::Expense,
            other => return Err(LedgerError::UnknownCategory(other.to_string())),
        };

        Ok(AccountKey {
            category,
            segment: parts[1].to_uppercase(),
            id: parts[2].to_uppercase(),
            asset: parts[3].to_uppercase(),
            sub_account: parts[4].to_uppercase(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_account_key() {
        let key: AccountKey = "LIAB:USER:ALICE:USDT:AVAILABLE".parse().unwrap();
        assert_eq!(key.category, AccountCategory::Liability);
        assert_eq!(key.segment, "USER");
        assert_eq!(key.id, "ALICE");
        assert_eq!(key.asset, "USDT");
        assert_eq!(key.sub_account, "AVAILABLE");
    }

    #[test]
    fn test_account_key_display() {
        let key = AccountKey::user_available("ALICE", "USDT");
        assert_eq!(key.to_string(), "LIAB:USER:ALICE:USDT:AVAILABLE");
    }

    #[test]
    fn test_account_key_roundtrip() {
        let original = "ASSET:SYSTEM:VAULT:BTC:MAIN";
        let key: AccountKey = original.parse().unwrap();
        assert_eq!(key.to_string(), original);
    }

    #[test]
    fn test_normal_balance() {
        assert_eq!(AccountCategory::Asset.normal_balance(), Side::Debit);
        assert_eq!(AccountCategory::Liability.normal_balance(), Side::Credit);
        assert_eq!(AccountCategory::Revenue.normal_balance(), Side::Credit);
        assert_eq!(AccountCategory::Expense.normal_balance(), Side::Debit);
    }

    #[test]
    fn test_invalid_format() {
        let result: Result<AccountKey, _> = "LIAB:USER:ALICE".parse();
        assert!(matches!(result, Err(LedgerError::InvalidAccountFormat(_))));
    }
}

```

## File ..\bibank\crates\ledger\src\entry.rs:
```rust
//! Journal Entry - The atomic unit of financial state change

use crate::account::AccountKey;
use crate::error::LedgerError;
use crate::signature::EntrySignature;
use bibank_core::Amount;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transaction intent - Financial primitive (NOT workflow)
///
/// Each intent represents a specific type of financial operation.
/// The ledger validates entries based on their intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionIntent {
    /// System initialization - creates initial balances
    Genesis,

    /// External money entering the system
    Deposit,

    /// External money leaving the system
    Withdrawal,

    /// Internal transfer between accounts
    Transfer,

    /// Exchange between different assets
    Trade,

    /// Fee collection
    Fee,

    /// Manual adjustment (audit-heavy, requires approval)
    Adjustment,
}

/// Posting side - Debit or Credit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Debit - increases assets/expenses, decreases liabilities/equity/revenue
    Debit,

    /// Credit - decreases assets/expenses, increases liabilities/equity/revenue
    Credit,
}

/// A single posting within a journal entry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posting {
    /// The ledger account being affected
    pub account: AccountKey,

    /// The amount (always positive)
    pub amount: Amount,

    /// Debit or Credit
    pub side: Side,
}

impl Posting {
    /// Create a new posting
    pub fn new(account: AccountKey, amount: Amount, side: Side) -> Self {
        Self {
            account,
            amount,
            side,
        }
    }

    /// Create a debit posting
    pub fn debit(account: AccountKey, amount: Amount) -> Self {
        Self::new(account, amount, Side::Debit)
    }

    /// Create a credit posting
    pub fn credit(account: AccountKey, amount: Amount) -> Self {
        Self::new(account, amount, Side::Credit)
    }

    /// Get the signed amount for balance calculation
    /// Debit = positive, Credit = negative
    pub fn signed_amount(&self) -> Decimal {
        match self.side {
            Side::Debit => self.amount.value(),
            Side::Credit => -self.amount.value(),
        }
    }
}

/// Journal Entry - The atomic unit of financial state change
///
/// Every entry MUST be double-entry balanced (zero-sum per asset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    // === Ordering & Integrity ===
    /// Global sequence number (strictly increasing)
    pub sequence: u64,

    /// SHA256 hash of previous entry (or "GENESIS" for first entry)
    pub prev_hash: String,

    /// SHA256 hash of this entry's content
    pub hash: String,

    /// Timestamp when the entry was created
    pub timestamp: DateTime<Utc>,

    // === Semantics ===
    /// The type of financial operation
    pub intent: TransactionIntent,

    // === Tracing ===
    /// Request UUID from API/CLI (REQUIRED, never generated by ledger)
    pub correlation_id: String,

    /// Optional link to parent entry that caused this entry
    pub causality_id: Option<String>,

    // === Financial Data ===
    /// The list of postings (debits and credits)
    pub postings: Vec<Posting>,

    // === Metadata ===
    /// Additional metadata (opaque to ledger, used by audit/projection)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    // === Digital Signatures (Phase 2) ===
    /// Signatures from system and/or operators
    #[serde(default)]
    pub signatures: Vec<EntrySignature>,
}

impl JournalEntry {
    /// Validate the entry according to ledger invariants
    ///
    /// # Rules
    /// 1. At least 2 postings (double-entry)
    /// 2. Zero-sum per asset group
    /// 3. Non-empty correlation_id
    /// 4. Genesis entries have special requirements
    pub fn validate(&self) -> Result<(), LedgerError> {
        // Rule: correlation_id cannot be empty
        if self.correlation_id.is_empty() {
            return Err(LedgerError::EmptyCorrelationId);
        }

        // Rule: At least 2 postings
        if self.postings.len() < 2 {
            return Err(LedgerError::InsufficientPostings);
        }

        // Rule: Genesis entry special requirements
        if self.intent == TransactionIntent::Genesis {
            if self.sequence != 1 {
                return Err(LedgerError::InvalidGenesisSequence);
            }
            if self.prev_hash != "GENESIS" {
                return Err(LedgerError::InvalidGenesisPrevHash);
            }
        }

        // Rule: Zero-sum per asset
        let mut sums: HashMap<String, Decimal> = HashMap::new();
        for posting in &self.postings {
            let asset = &posting.account.asset;
            *sums.entry(asset.clone()).or_default() += posting.signed_amount();
        }

        for (asset, sum) in sums {
            if !sum.is_zero() {
                return Err(LedgerError::UnbalancedEntry {
                    asset,
                    imbalance: sum,
                });
            }
        }

        Ok(())
    }

    /// Get all unique assets in this entry
    pub fn assets(&self) -> Vec<String> {
        let mut assets: Vec<_> = self
            .postings
            .iter()
            .map(|p| p.account.asset.clone())
            .collect();
        assets.sort();
        assets.dedup();
        assets
    }
}

/// Builder for creating JournalEntry with fluent API
#[derive(Debug, Default)]
pub struct JournalEntryBuilder {
    intent: Option<TransactionIntent>,
    correlation_id: Option<String>,
    causality_id: Option<String>,
    postings: Vec<Posting>,
    metadata: HashMap<String, serde_json::Value>,
}

impl JournalEntryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intent(mut self, intent: TransactionIntent) -> Self {
        self.intent = Some(intent);
        self
    }

    pub fn correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    pub fn causality_id(mut self, id: impl Into<String>) -> Self {
        self.causality_id = Some(id.into());
        self
    }

    pub fn posting(mut self, posting: Posting) -> Self {
        self.postings.push(posting);
        self
    }

    pub fn debit(mut self, account: AccountKey, amount: Amount) -> Self {
        self.postings.push(Posting::debit(account, amount));
        self
    }

    pub fn credit(mut self, account: AccountKey, amount: Amount) -> Self {
        self.postings.push(Posting::credit(account, amount));
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Build the entry (sequence, prev_hash, hash, timestamp will be set by ledger)
    pub fn build_unsigned(self) -> Result<UnsignedEntry, LedgerError> {
        let intent = self.intent.unwrap_or(TransactionIntent::Transfer);
        let correlation_id = self
            .correlation_id
            .ok_or(LedgerError::EmptyCorrelationId)?;

        if correlation_id.is_empty() {
            return Err(LedgerError::EmptyCorrelationId);
        }

        if self.postings.len() < 2 {
            return Err(LedgerError::InsufficientPostings);
        }

        let unsigned = UnsignedEntry {
            intent,
            correlation_id,
            causality_id: self.causality_id,
            postings: self.postings,
            metadata: self.metadata,
        };

        // Validate double-entry balance before returning
        unsigned.validate_balance()?;

        Ok(unsigned)
    }
}

/// An entry that hasn't been signed with sequence/hash yet
#[derive(Debug, Clone)]
pub struct UnsignedEntry {
    pub intent: TransactionIntent,
    pub correlation_id: String,
    pub causality_id: Option<String>,
    pub postings: Vec<Posting>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl UnsignedEntry {
    /// Validate double-entry balance
    pub fn validate_balance(&self) -> Result<(), LedgerError> {
        let mut sums: HashMap<String, Decimal> = HashMap::new();
        for posting in &self.postings {
            let asset = &posting.account.asset;
            *sums.entry(asset.clone()).or_default() += posting.signed_amount();
        }

        for (asset, sum) in sums {
            if !sum.is_zero() {
                return Err(LedgerError::UnbalancedEntry {
                    asset,
                    imbalance: sum,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    #[test]
    fn test_balanced_entry() {
        let entry = JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Genesis,
            correlation_id: "test-123".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(1000)),
                Posting::credit(
                    AccountKey::new(
                        crate::AccountCategory::Equity,
                        "SYSTEM",
                        "CAPITAL",
                        "USDT",
                        "MAIN",
                    ),
                    amount(1000),
                ),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        assert!(entry.validate().is_ok());
    }

    #[test]
    fn test_unbalanced_entry() {
        let entry = JournalEntry {
            sequence: 2,
            prev_hash: "abc".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test-123".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(50)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let result = entry.validate();
        assert!(matches!(result, Err(LedgerError::UnbalancedEntry { .. })));
    }

    #[test]
    fn test_multi_asset_trade() {
        let entry = JournalEntry {
            sequence: 2,
            prev_hash: "abc".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Trade,
            correlation_id: "test-123".to_string(),
            causality_id: None,
            postings: vec![
                // USDT leg: Alice pays, Bob receives
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                // BTC leg: Bob pays, Alice receives
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        assert!(entry.validate().is_ok());
        assert_eq!(entry.assets(), vec!["BTC", "USDT"]);
    }

    #[test]
    fn test_empty_correlation_id() {
        let entry = JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Genesis,
            correlation_id: "".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(1000)),
                Posting::credit(
                    AccountKey::new(
                        crate::AccountCategory::Equity,
                        "SYSTEM",
                        "CAPITAL",
                        "USDT",
                        "MAIN",
                    ),
                    amount(1000),
                ),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        assert!(matches!(
            entry.validate(),
            Err(LedgerError::EmptyCorrelationId)
        ));
    }

    #[test]
    fn test_builder() {
        let unsigned = JournalEntryBuilder::new()
            .intent(TransactionIntent::Deposit)
            .correlation_id("req-123")
            .debit(AccountKey::system_vault("USDT"), amount(100))
            .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .build_unsigned()
            .unwrap();

        assert_eq!(unsigned.intent, TransactionIntent::Deposit);
        assert_eq!(unsigned.postings.len(), 2);
        assert!(unsigned.validate_balance().is_ok());
    }
}

```

## File ..\bibank\crates\ledger\src\error.rs:
```rust
//! Ledger errors

use rust_decimal::Decimal;
use thiserror::Error;

/// Errors that can occur in ledger operations
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    #[error("Invalid account format: {0}")]
    InvalidAccountFormat(String),

    #[error("Unknown account category: {0}")]
    UnknownCategory(String),

    #[error("Entry must have at least 2 postings for double-entry")]
    InsufficientPostings,

    #[error("Entry unbalanced for asset {asset}: imbalance {imbalance}")]
    UnbalancedEntry { asset: String, imbalance: Decimal },

    #[error("correlation_id cannot be empty")]
    EmptyCorrelationId,

    #[error("Genesis entry must have sequence = 1")]
    InvalidGenesisSequence,

    #[error("Genesis entry must have prev_hash = 'GENESIS'")]
    InvalidGenesisPrevHash,

    #[error("Broken hash chain at sequence {sequence}: expected {expected}, got {actual}")]
    BrokenHashChain {
        sequence: u64,
        expected: String,
        actual: String,
    },

    #[error("Sequence must be strictly increasing: expected {expected}, got {actual}")]
    InvalidSequence { expected: u64, actual: u64 },

    // === Phase 2: Intent-specific validation errors ===

    #[error("Invalid {intent} posting on {account}: {reason}")]
    InvalidIntentPosting {
        intent: &'static str,
        account: String,
        reason: &'static str,
    },

    #[error("Trade requires {expected} postings, got {actual}: {reason}")]
    InvalidTradePostings {
        expected: usize,
        actual: usize,
        reason: &'static str,
    },

    #[error("Trade requires exactly {expected} assets, got {actual}: {assets:?}")]
    InvalidTradeAssets {
        expected: usize,
        actual: usize,
        assets: Vec<String>,
    },

    // === Phase 2: Signature errors ===

    #[error("Missing system signature")]
    MissingSystemSignature,

    #[error("Invalid signature from {signer}: {reason}")]
    InvalidSignature {
        signer: String,
        reason: String,
    },

    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),
}

```

## File ..\bibank\crates\ledger\src\hash.rs:
```rust
//! Hash chain utilities for ledger integrity

use crate::entry::JournalEntry;
use sha2::{Digest, Sha256};

/// Calculate SHA256 hash of entry content (excluding the hash field itself)
pub fn calculate_entry_hash(entry: &JournalEntry) -> String {
    let mut hasher = Sha256::new();

    // Include all fields except `hash`
    hasher.update(entry.sequence.to_le_bytes());
    hasher.update(entry.prev_hash.as_bytes());
    hasher.update(entry.timestamp.to_rfc3339().as_bytes());
    hasher.update(format!("{:?}", entry.intent).as_bytes());
    hasher.update(entry.correlation_id.as_bytes());

    if let Some(ref causality_id) = entry.causality_id {
        hasher.update(causality_id.as_bytes());
    }

    // Hash postings
    for posting in &entry.postings {
        hasher.update(posting.account.to_string().as_bytes());
        hasher.update(posting.amount.value().to_string().as_bytes());
        hasher.update(format!("{:?}", posting.side).as_bytes());
    }

    // Hash metadata keys (sorted for determinism)
    let mut keys: Vec<_> = entry.metadata.keys().collect();
    keys.sort();
    for key in keys {
        hasher.update(key.as_bytes());
        if let Some(value) = entry.metadata.get(key) {
            hasher.update(value.to_string().as_bytes());
        }
    }

    hex::encode(hasher.finalize())
}

/// Verify hash chain integrity
pub fn verify_chain(entries: &[JournalEntry]) -> Result<(), ChainError> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut prev_hash = "GENESIS".to_string();

    for (i, entry) in entries.iter().enumerate() {
        // Verify prev_hash links correctly
        if entry.prev_hash != prev_hash {
            return Err(ChainError::BrokenLink {
                sequence: entry.sequence,
                expected: prev_hash,
                actual: entry.prev_hash.clone(),
            });
        }

        // Verify hash is correct
        let calculated = calculate_entry_hash(entry);
        if entry.hash != calculated {
            return Err(ChainError::InvalidHash {
                sequence: entry.sequence,
                expected: calculated,
                actual: entry.hash.clone(),
            });
        }

        // Verify sequence is strictly increasing
        if i > 0 && entry.sequence != entries[i - 1].sequence + 1 {
            return Err(ChainError::InvalidSequence {
                expected: entries[i - 1].sequence + 1,
                actual: entry.sequence,
            });
        }

        prev_hash = entry.hash.clone();
    }

    Ok(())
}

/// Errors in hash chain verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    BrokenLink {
        sequence: u64,
        expected: String,
        actual: String,
    },
    InvalidHash {
        sequence: u64,
        expected: String,
        actual: String,
    },
    InvalidSequence {
        expected: u64,
        actual: u64,
    },
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainError::BrokenLink {
                sequence,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Broken link at seq {}: expected prev_hash '{}', got '{}'",
                    sequence, expected, actual
                )
            }
            ChainError::InvalidHash {
                sequence,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Invalid hash at seq {}: expected '{}', got '{}'",
                    sequence, expected, actual
                )
            }
            ChainError::InvalidSequence { expected, actual } => {
                write!(
                    f,
                    "Invalid sequence: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for ChainError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountKey, Posting, TransactionIntent};
    use bibank_core::Amount;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    fn create_entry(sequence: u64, prev_hash: &str) -> JournalEntry {
        let amount = Amount::new(Decimal::new(100, 0)).unwrap();
        let mut entry = JournalEntry {
            sequence,
            prev_hash: prev_hash.to_string(),
            hash: String::new(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: format!("test-{}", sequence),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };
        entry.hash = calculate_entry_hash(&entry);
        entry
    }

    #[test]
    fn test_hash_deterministic() {
        let entry = create_entry(1, "GENESIS");
        let hash1 = calculate_entry_hash(&entry);
        let hash2 = calculate_entry_hash(&entry);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_verify_valid_chain() {
        let entry1 = create_entry(1, "GENESIS");
        let entry2 = create_entry(2, &entry1.hash);
        let entry3 = create_entry(3, &entry2.hash);

        let entries = vec![entry1, entry2, entry3];
        assert!(verify_chain(&entries).is_ok());
    }

    #[test]
    fn test_verify_broken_chain() {
        let entry1 = create_entry(1, "GENESIS");
        let entry2 = create_entry(2, "wrong_hash");

        let entries = vec![entry1, entry2];
        let result = verify_chain(&entries);
        assert!(matches!(result, Err(ChainError::BrokenLink { .. })));
    }
}

```

## File ..\bibank\crates\ledger\src\lib.rs:
```rust
//! BiBank Ledger - Double-entry accounting core
//!
//! This is the HEART of BiBank. All financial state changes go through this crate.
//!
//! # Key Types
//! - `AccountKey`: Hierarchical account identifier (CAT:SEGMENT:ID:ASSET:SUB_ACCOUNT)
//! - `JournalEntry`: Atomic unit of financial state change
//! - `Posting`: Single debit/credit in an entry
//! - `Side`: Debit or Credit

pub mod account;
pub mod entry;
pub mod error;
pub mod hash;
pub mod signature;
pub mod validation;

pub use account::{AccountCategory, AccountKey};
pub use entry::{JournalEntry, JournalEntryBuilder, Posting, Side, TransactionIntent, UnsignedEntry};
pub use error::LedgerError;
pub use signature::{EntrySignature, SignatureAlgorithm, SignablePayload, Signer, SystemSigner};
pub use validation::validate_intent;

```

## File ..\bibank\crates\ledger\src\signature.rs:
```rust
//! Digital signatures for journal entries
//!
//! Each entry is signed by the system key, and optionally by operator keys
//! for Adjustment entries requiring human approval.

use crate::entry::{JournalEntry, Posting, TransactionIntent};
use crate::error::LedgerError;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer as DalekSigner, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Signature algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureAlgorithm {
    /// Ed25519 (default)
    Ed25519,
    /// Secp256k1 (for future blockchain compatibility)
    Secp256k1,
}

impl Default for SignatureAlgorithm {
    fn default() -> Self {
        Self::Ed25519
    }
}

/// Digital signature attached to a journal entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySignature {
    /// Signer identifier ("SYSTEM" or operator ID)
    pub signer_id: String,

    /// Signature algorithm used
    pub algorithm: SignatureAlgorithm,

    /// Public key (hex-encoded)
    pub public_key: String,

    /// Signature bytes (hex-encoded)
    pub signature: String,

    /// Timestamp when signature was created
    pub signed_at: DateTime<Utc>,
}

impl EntrySignature {
    /// Verify this signature against a payload
    pub fn verify(&self, payload: &[u8]) -> Result<(), LedgerError> {
        match self.algorithm {
            SignatureAlgorithm::Ed25519 => {
                let pk_bytes = hex::decode(&self.public_key).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid public key hex: {}", e),
                    }
                })?;

                let sig_bytes = hex::decode(&self.signature).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid signature hex: {}", e),
                    }
                })?;

                let pk_array: [u8; 32] = pk_bytes.try_into().map_err(|_| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: "Public key must be 32 bytes".to_string(),
                    }
                })?;

                let sig_array: [u8; 64] = sig_bytes.try_into().map_err(|_| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: "Signature must be 64 bytes".to_string(),
                    }
                })?;

                let verifying_key = VerifyingKey::from_bytes(&pk_array).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid public key: {}", e),
                    }
                })?;

                let signature = Signature::from_bytes(&sig_array);

                verifying_key.verify(payload, &signature).map_err(|e| {
                    LedgerError::SignatureVerificationFailed(format!(
                        "Signature from {} failed: {}",
                        self.signer_id, e
                    ))
                })?;

                Ok(())
            }
            SignatureAlgorithm::Secp256k1 => {
                // Future: implement secp256k1 verification
                Err(LedgerError::SignatureVerificationFailed(
                    "Secp256k1 not yet implemented".to_string(),
                ))
            }
        }
    }
}

/// Signable payload - the 8 fields that are signed
///
/// This is a deterministic representation of the entry for signing.
/// Order matters for consistent hashing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePayload {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub intent: TransactionIntent,
    pub postings: Vec<Posting>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub prev_hash: String,
    pub hash: String,
    pub signed_at: DateTime<Utc>,
}

impl SignablePayload {
    /// Create from a journal entry and signing timestamp
    pub fn from_entry(entry: &JournalEntry, signed_at: DateTime<Utc>) -> Self {
        Self {
            sequence: entry.sequence,
            timestamp: entry.timestamp,
            intent: entry.intent,
            postings: entry.postings.clone(),
            metadata: entry.metadata.clone(),
            prev_hash: entry.prev_hash.clone(),
            hash: entry.hash.clone(),
            signed_at,
        }
    }

    /// Serialize to canonical JSON bytes for signing
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("SignablePayload serialization should never fail")
    }
}

/// Trait for signers
pub trait Signer: Send + Sync {
    /// Get the signer ID
    fn signer_id(&self) -> &str;

    /// Get the public key (hex-encoded)
    fn public_key_hex(&self) -> String;

    /// Sign a payload and return the signature
    fn sign(&self, entry: &JournalEntry) -> EntrySignature;
}

/// System signer using Ed25519
pub struct SystemSigner {
    signing_key: SigningKey,
}

impl SystemSigner {
    /// Create from a 32-byte seed (hex-encoded in env var)
    pub fn from_hex(hex_seed: &str) -> Result<Self, LedgerError> {
        let bytes = hex::decode(hex_seed).map_err(|e| {
            LedgerError::InvalidSignature {
                signer: "SYSTEM".to_string(),
                reason: format!("Invalid key hex: {}", e),
            }
        })?;

        let seed: [u8; 32] = bytes.try_into().map_err(|_| {
            LedgerError::InvalidSignature {
                signer: "SYSTEM".to_string(),
                reason: "Key must be 32 bytes".to_string(),
            }
        })?;

        Ok(Self {
            signing_key: SigningKey::from_bytes(&seed),
        })
    }

    /// Generate a new random signing key
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            signing_key: SigningKey::generate(&mut rng),
        }
    }

    /// Export the seed as hex (for storage)
    pub fn seed_hex(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }
}

impl Signer for SystemSigner {
    fn signer_id(&self) -> &str {
        "SYSTEM"
    }

    fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    fn sign(&self, entry: &JournalEntry) -> EntrySignature {
        let signed_at = Utc::now();
        let payload = SignablePayload::from_entry(entry, signed_at);
        let payload_bytes = payload.to_bytes();

        let signature = self.signing_key.sign(&payload_bytes);

        EntrySignature {
            signer_id: self.signer_id().to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: self.public_key_hex(),
            signature: hex::encode(signature.to_bytes()),
            signed_at,
        }
    }
}

impl JournalEntry {
    /// Verify all signatures on this entry
    pub fn verify_signatures(&self) -> Result<(), LedgerError> {
        if self.signatures.is_empty() {
            // Phase 1 entries have no signatures - that's OK
            return Ok(());
        }

        for sig in &self.signatures {
            let payload = SignablePayload::from_entry(self, sig.signed_at);
            sig.verify(&payload.to_bytes())?;
        }

        Ok(())
    }

    /// Check if entry has a system signature
    pub fn has_system_signature(&self) -> bool {
        self.signatures.iter().any(|s| s.signer_id == "SYSTEM")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountKey;
    use bibank_core::Amount;
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    fn make_test_entry() -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "abc123".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_system_signer_sign_and_verify() {
        let signer = SystemSigner::generate();
        let entry = make_test_entry();

        let signature = signer.sign(&entry);

        assert_eq!(signature.signer_id, "SYSTEM");
        assert_eq!(signature.algorithm, SignatureAlgorithm::Ed25519);

        // Verify the signature
        let payload = SignablePayload::from_entry(&entry, signature.signed_at);
        assert!(signature.verify(&payload.to_bytes()).is_ok());
    }

    #[test]
    fn test_signature_roundtrip() {
        let signer = SystemSigner::generate();
        let seed = signer.seed_hex();

        // Recreate signer from seed
        let signer2 = SystemSigner::from_hex(&seed).unwrap();
        assert_eq!(signer.public_key_hex(), signer2.public_key_hex());
    }

    #[test]
    fn test_entry_with_signature() {
        let signer = SystemSigner::generate();
        let mut entry = make_test_entry();

        let signature = signer.sign(&entry);
        entry.signatures.push(signature);

        assert!(entry.verify_signatures().is_ok());
        assert!(entry.has_system_signature());
    }

    #[test]
    fn test_tampered_entry_fails_verification() {
        let signer = SystemSigner::generate();
        let mut entry = make_test_entry();

        let signature = signer.sign(&entry);
        entry.signatures.push(signature);

        // Tamper with the entry
        entry.sequence = 999;

        // Verification should fail
        assert!(entry.verify_signatures().is_err());
    }
}

```

## File ..\bibank\crates\ledger\src\validation.rs:
```rust
//! Intent-specific validation rules
//!
//! Each TransactionIntent has specific validation rules beyond basic double-entry.

use crate::account::AccountCategory;
use crate::entry::{Posting, Side, TransactionIntent, UnsignedEntry};
use crate::error::LedgerError;

/// Validation result with detailed error
pub type ValidationResult = Result<(), LedgerError>;

/// Validate an unsigned entry according to its intent
pub fn validate_intent(entry: &UnsignedEntry) -> ValidationResult {
    match entry.intent {
        TransactionIntent::Genesis => validate_genesis(entry),
        TransactionIntent::Deposit => validate_deposit(entry),
        TransactionIntent::Withdrawal => validate_withdrawal(entry),
        TransactionIntent::Transfer => validate_transfer(entry),
        TransactionIntent::Trade => validate_trade(entry),
        TransactionIntent::Fee => validate_fee(entry),
        TransactionIntent::Adjustment => validate_adjustment(entry),
    }
}

/// Genesis: ASSET ↑, EQUITY ↑
fn validate_genesis(entry: &UnsignedEntry) -> ValidationResult {
    for posting in &entry.postings {
        match posting.account.category {
            AccountCategory::Asset | AccountCategory::Equity => {}
            _ => {
                return Err(LedgerError::InvalidIntentPosting {
                    intent: "Genesis",
                    account: posting.account.to_string(),
                    reason: "Genesis only allows ASSET and EQUITY accounts",
                });
            }
        }
    }
    Ok(())
}

/// Deposit: ASSET ↑, LIAB ↑
fn validate_deposit(entry: &UnsignedEntry) -> ValidationResult {
    let has_asset_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset && p.side == Side::Debit
    });
    let has_liab_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Credit
    });

    if !has_asset_debit || !has_liab_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Deposit",
            account: String::new(),
            reason: "Deposit requires ASSET debit and LIAB credit",
        });
    }
    Ok(())
}

/// Withdrawal: ASSET ↓, LIAB ↓
fn validate_withdrawal(entry: &UnsignedEntry) -> ValidationResult {
    let has_asset_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Asset && p.side == Side::Credit
    });
    let has_liab_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Debit
    });

    if !has_asset_credit || !has_liab_debit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Withdrawal",
            account: String::new(),
            reason: "Withdrawal requires ASSET credit and LIAB debit",
        });
    }
    Ok(())
}

/// Transfer: LIAB only
fn validate_transfer(entry: &UnsignedEntry) -> ValidationResult {
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Transfer",
                account: posting.account.to_string(),
                reason: "Transfer only allows LIAB accounts",
            });
        }
    }
    Ok(())
}

/// Trade: LIAB only, exactly 2 assets, min 4 postings, zero-sum per asset
pub fn validate_trade(entry: &UnsignedEntry) -> ValidationResult {
    // Rule 1: Min 4 postings (2 legs × 2 sides)
    if entry.postings.len() < 4 {
        return Err(LedgerError::InvalidTradePostings {
            expected: 4,
            actual: entry.postings.len(),
            reason: "Trade requires at least 4 postings (2 assets × 2 sides)",
        });
    }

    // Rule 2: LIAB accounts only
    for posting in &entry.postings {
        if posting.account.category != AccountCategory::Liability {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Trade",
                account: posting.account.to_string(),
                reason: "Trade only allows LIAB accounts",
            });
        }
    }

    // Rule 3: Exactly 2 assets
    let assets = collect_assets(&entry.postings);
    if assets.len() != 2 {
        return Err(LedgerError::InvalidTradeAssets {
            expected: 2,
            actual: assets.len(),
            assets: assets.into_iter().collect(),
        });
    }

    // Rule 4: Zero-sum per asset (already checked in validate_balance)
    // Rule 5: At least 2 users (implicit from 4 postings with different accounts)

    Ok(())
}

/// Fee: LIAB ↓, REV ↑
pub fn validate_fee(entry: &UnsignedEntry) -> ValidationResult {
    let has_liab_debit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Liability && p.side == Side::Debit
    });
    let has_rev_credit = entry.postings.iter().any(|p| {
        p.account.category == AccountCategory::Revenue && p.side == Side::Credit
    });

    if !has_liab_debit || !has_rev_credit {
        return Err(LedgerError::InvalidIntentPosting {
            intent: "Fee",
            account: String::new(),
            reason: "Fee requires LIAB debit and REV credit",
        });
    }

    // All postings must be either LIAB debit or REV credit
    for posting in &entry.postings {
        let valid = match (&posting.account.category, &posting.side) {
            (AccountCategory::Liability, Side::Debit) => true,
            (AccountCategory::Revenue, Side::Credit) => true,
            _ => false,
        };
        if !valid {
            return Err(LedgerError::InvalidIntentPosting {
                intent: "Fee",
                account: posting.account.to_string(),
                reason: "Fee only allows LIAB debit or REV credit",
            });
        }
    }

    Ok(())
}

/// Adjustment: Any accounts (audit-heavy)
fn validate_adjustment(_entry: &UnsignedEntry) -> ValidationResult {
    // Adjustment allows any accounts but requires approval flag
    // Approval is checked at RPC layer
    Ok(())
}

/// Collect unique assets from postings
fn collect_assets(postings: &[Posting]) -> std::collections::HashSet<String> {
    postings.iter().map(|p| p.account.asset.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountKey;
    use bibank_core::Amount;
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    #[test]
    fn test_validate_trade_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // USDT leg
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                // BTC leg
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_trade(&entry).is_ok());
    }

    #[test]
    fn test_validate_trade_insufficient_postings() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidTradePostings { .. })));
    }

    #[test]
    fn test_validate_trade_wrong_asset_count() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Only USDT, no second asset
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(50)),
                Posting::debit(AccountKey::user_available("CHARLIE", "USDT"), amount(50)),
                Posting::credit(AccountKey::user_available("DAVE", "USDT"), amount(100)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidTradeAssets { .. })));
    }

    #[test]
    fn test_validate_trade_non_liab_account() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Trade,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // ASSET account not allowed in Trade
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: Default::default(),
        };

        let result = validate_trade(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { .. })));
    }

    #[test]
    fn test_validate_fee_success() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Fee,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(1)),
                Posting::credit(AccountKey::fee_revenue("USDT"), amount(1)),
            ],
            metadata: Default::default(),
        };

        assert!(validate_fee(&entry).is_ok());
    }

    #[test]
    fn test_validate_fee_wrong_direction() {
        let entry = UnsignedEntry {
            intent: TransactionIntent::Fee,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                // Wrong: LIAB credit instead of debit
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(1)),
                Posting::debit(AccountKey::fee_revenue("USDT"), amount(1)),
            ],
            metadata: Default::default(),
        };

        let result = validate_fee(&entry);
        assert!(matches!(result, Err(LedgerError::InvalidIntentPosting { .. })));
    }
}

```

## File ..\bibank\crates\projection\src\balance.rs:
```rust
//! Balance projection - tracks account balances from events

use bibank_ledger::JournalEntry;
use rust_decimal::Decimal;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

/// Balance projection - tracks account balances
pub struct BalanceProjection {
    pool: SqlitePool,
}

impl BalanceProjection {
    /// Create a new balance projection
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the schema
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS balances (
                account_key TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                segment TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                asset TEXT NOT NULL,
                sub_account TEXT NOT NULL,
                balance TEXT NOT NULL DEFAULT '0',
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_balances_entity
            ON balances(entity_id, asset)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Apply a journal entry to update balances
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), sqlx::Error> {
        for posting in &entry.postings {
            let key = posting.account.to_string();
            let normal_side = posting.account.category.normal_balance();

            let delta = if posting.side == normal_side {
                posting.amount.value()
            } else {
                -posting.amount.value()
            };

            // Upsert balance
            sqlx::query(
                r#"
                INSERT INTO balances (account_key, category, segment, entity_id, asset, sub_account, balance, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(account_key) DO UPDATE SET
                    balance = CAST((CAST(balance AS REAL) + CAST(? AS REAL)) AS TEXT),
                    updated_at = ?
                "#,
            )
            .bind(&key)
            .bind(posting.account.category.code())
            .bind(&posting.account.segment)
            .bind(&posting.account.id)
            .bind(&posting.account.asset)
            .bind(&posting.account.sub_account)
            .bind(delta.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .bind(delta.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get balance for a specific account
    pub async fn get_balance(&self, account_key: &str) -> Result<Decimal, sqlx::Error> {
        let row = sqlx::query("SELECT balance FROM balances WHERE account_key = ?")
            .bind(account_key)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let balance_str: String = row.get("balance");
                Ok(balance_str.parse().unwrap_or(Decimal::ZERO))
            }
            None => Ok(Decimal::ZERO),
        }
    }

    /// Get all balances for a user
    pub async fn get_user_balances(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, Decimal>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT asset, balance
            FROM balances
            WHERE segment = 'USER' AND entity_id = ? AND sub_account = 'AVAILABLE'
            "#,
        )
        .bind(user_id.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        let mut balances = HashMap::new();
        for row in rows {
            let asset: String = row.get("asset");
            let balance_str: String = row.get("balance");
            let balance: Decimal = balance_str.parse().unwrap_or(Decimal::ZERO);
            balances.insert(asset, balance);
        }

        Ok(balances)
    }

    /// Clear all balances (for replay)
    pub async fn clear(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM balances")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

```

## File ..\bibank\crates\projection\src\engine.rs:
```rust
//! Projection engine - coordinates replay and updates

use crate::balance::BalanceProjection;
use crate::error::ProjectionError;
use crate::trade::TradeProjection;
use bibank_bus::EventBus;
use bibank_ledger::JournalEntry;
use sqlx::SqlitePool;
use std::path::Path;

/// Projection engine - coordinates replay and updates
pub struct ProjectionEngine {
    pub balance: BalanceProjection,
    pub trade: TradeProjection,
}

impl ProjectionEngine {
    /// Create a new projection engine
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self, ProjectionError> {
        let db_url = format!("sqlite:{}?mode=rwc", db_path.as_ref().display());
        let pool = SqlitePool::connect(&db_url).await?;

        let balance = BalanceProjection::new(pool.clone());
        balance.init().await?;

        let trade = TradeProjection::new(pool);
        trade.init().await?;

        Ok(Self { balance, trade })
    }

    /// Apply a single entry
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), ProjectionError> {
        self.balance.apply(entry).await?;
        self.trade.apply(entry).await?;
        Ok(())
    }

    /// Replay all events from the bus
    pub async fn replay(&self, bus: &EventBus) -> Result<usize, ProjectionError> {
        let reader = bus.reader()?;
        let entries = reader.read_all()?;

        self.balance.clear().await?;
        self.trade.clear().await?;

        let count = entries.len();
        for entry in &entries {
            self.balance.apply(entry).await?;
            self.trade.apply(entry).await?;
        }

        Ok(count)
    }

    /// Get the balance projection
    pub fn balance(&self) -> &BalanceProjection {
        &self.balance
    }

    /// Get the trade projection
    pub fn trade(&self) -> &TradeProjection {
        &self.trade
    }
}

```

## File ..\bibank\crates\projection\src\error.rs:
```rust
//! Projection errors

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProjectionError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Event error: {0}")]
    Event(#[from] bibank_events::EventError),

    #[error("Projection not initialized")]
    NotInitialized,
}

```

## File ..\bibank\crates\projection\src\lib.rs:
```rust
//! BiBank Projection - Event to SQLite views
//!
//! Projections are DISPOSABLE - they can be rebuilt from events at any time.

pub mod balance;
pub mod engine;
pub mod error;
pub mod trade;

pub use balance::BalanceProjection;
pub use engine::ProjectionEngine;
pub use error::ProjectionError;
pub use trade::{TradeProjection, TradeRecord};

```

## File ..\bibank\crates\projection\src\trade.rs:
```rust
//! Trade projection - tracks trade history from events

use bibank_ledger::{JournalEntry, TransactionIntent};
use rust_decimal::Decimal;
use sqlx::{Row, SqlitePool};

/// Trade record from projection
#[derive(Debug, Clone)]
pub struct TradeRecord {
    /// Trade ID (sequence number)
    pub trade_id: u64,
    /// Seller user ID
    pub seller: String,
    /// Buyer user ID
    pub buyer: String,
    /// Asset being sold
    pub sell_asset: String,
    /// Amount being sold
    pub sell_amount: Decimal,
    /// Asset being bought
    pub buy_asset: String,
    /// Amount being bought
    pub buy_amount: Decimal,
    /// Trade timestamp
    pub timestamp: String,
    /// Entry hash
    pub hash: String,
}

/// Trade projection - tracks trade history
pub struct TradeProjection {
    pool: SqlitePool,
}

impl TradeProjection {
    /// Create a new trade projection
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the schema
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                trade_id INTEGER PRIMARY KEY,
                seller TEXT NOT NULL,
                buyer TEXT NOT NULL,
                sell_asset TEXT NOT NULL,
                sell_amount TEXT NOT NULL,
                buy_asset TEXT NOT NULL,
                buy_amount TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                hash TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_seller
            ON trades(seller)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_buyer
            ON trades(buyer)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_assets
            ON trades(sell_asset, buy_asset)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Apply a journal entry to update trades
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), sqlx::Error> {
        // Only process Trade entries
        if entry.intent != TransactionIntent::Trade {
            return Ok(());
        }

        // Extract trade info from postings
        // Trade has 4+ postings:
        // - Seller DEBIT (loses sell_asset)
        // - Seller CREDIT (gains buy_asset)
        // - Buyer DEBIT (loses buy_asset)
        // - Buyer CREDIT (gains sell_asset)

        let mut seller: Option<String> = None;
        let mut buyer: Option<String> = None;
        let mut sell_asset: Option<String> = None;
        let mut sell_amount: Option<Decimal> = None;
        let mut buy_asset: Option<String> = None;
        let mut buy_amount: Option<Decimal> = None;

        for posting in &entry.postings {
            // Only look at user LIAB accounts
            if posting.account.segment != "USER" {
                continue;
            }

            let user_id = &posting.account.id;
            let asset = &posting.account.asset;

            // DEBIT on LIAB = user is paying (losing)
            // CREDIT on LIAB = user is receiving (gaining)
            use bibank_ledger::entry::Side;

            match posting.side {
                Side::Debit => {
                    // User is losing this asset
                    if seller.is_none() || seller.as_ref() == Some(user_id) {
                        seller = Some(user_id.clone());
                        sell_asset = Some(asset.clone());
                        sell_amount = Some(posting.amount.value());
                    } else {
                        buyer = Some(user_id.clone());
                        buy_asset = Some(asset.clone());
                        buy_amount = Some(posting.amount.value());
                    }
                }
                Side::Credit => {
                    // User is gaining this asset
                    if buyer.is_none() || buyer.as_ref() == Some(user_id) {
                        buyer = Some(user_id.clone());
                    } else {
                        seller = Some(user_id.clone());
                    }
                }
            }
        }

        // Insert trade record
        if let (
            Some(seller),
            Some(buyer),
            Some(sell_asset),
            Some(sell_amount),
            Some(buy_asset),
            Some(buy_amount),
        ) = (seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount)
        {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO trades
                (trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(entry.sequence as i64)
            .bind(&seller)
            .bind(&buyer)
            .bind(&sell_asset)
            .bind(sell_amount.to_string())
            .bind(&buy_asset)
            .bind(buy_amount.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .bind(&entry.hash)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get all trades for a user (as seller or buyer)
    pub async fn get_user_trades(&self, user_id: &str) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            WHERE seller = ? OR buyer = ?
            ORDER BY trade_id DESC
            "#,
        )
        .bind(user_id.to_uppercase())
        .bind(user_id.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get trades for a trading pair
    pub async fn get_pair_trades(
        &self,
        base_asset: &str,
        quote_asset: &str,
    ) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            WHERE (sell_asset = ? AND buy_asset = ?) OR (sell_asset = ? AND buy_asset = ?)
            ORDER BY trade_id DESC
            "#,
        )
        .bind(base_asset.to_uppercase())
        .bind(quote_asset.to_uppercase())
        .bind(quote_asset.to_uppercase())
        .bind(base_asset.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get recent trades (all)
    pub async fn get_recent_trades(&self, limit: u32) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            ORDER BY trade_id DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get trade count
    pub async fn count(&self) -> Result<u64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM trades")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64, _>("count") as u64)
    }

    /// Clear all trades (for replay)
    pub async fn clear(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM trades")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

```

## File ..\bibank\crates\risk\src\engine.rs:
```rust
//! Risk engine implementation

use crate::error::RiskError;
use crate::state::RiskState;
use bibank_ledger::{JournalEntry, UnsignedEntry};

/// Risk Engine - Pre-commit gatekeeper
///
/// Validates transactions before they are committed to the ledger.
/// Maintains in-memory state rebuilt from event replay.
pub struct RiskEngine {
    state: RiskState,
}

impl RiskEngine {
    /// Create a new empty risk engine
    pub fn new() -> Self {
        Self {
            state: RiskState::new(),
        }
    }

    /// Get reference to internal state
    pub fn state(&self) -> &RiskState {
        &self.state
    }

    /// Get mutable reference to internal state
    pub fn state_mut(&mut self) -> &mut RiskState {
        &mut self.state
    }

    /// Check if an unsigned entry passes all risk checks
    ///
    /// Returns Ok(()) if the entry is allowed, Err otherwise.
    pub fn check(&self, entry: &UnsignedEntry) -> Result<(), RiskError> {
        // Build a temporary JournalEntry for checking
        let temp_entry = JournalEntry {
            sequence: 0,
            prev_hash: String::new(),
            hash: String::new(),
            timestamp: chrono::Utc::now(),
            intent: entry.intent,
            correlation_id: entry.correlation_id.clone(),
            causality_id: entry.causality_id.clone(),
            postings: entry.postings.clone(),
            metadata: entry.metadata.clone(),
            signatures: Vec::new(),
        };

        self.check_entry(&temp_entry)
    }

    /// Check if a journal entry passes all risk checks
    pub fn check_entry(&self, entry: &JournalEntry) -> Result<(), RiskError> {
        // Rule 1: Check sufficient balance for liability accounts
        let violations = self.state.check_sufficient_balance(entry);

        if let Some((account, _balance)) = violations.first() {
            // Find the required amount from postings
            let required = entry
                .postings
                .iter()
                .find(|p| p.account.to_string() == *account)
                .map(|p| p.amount.value())
                .unwrap_or_default();

            let available = self.state.get_balance(
                &account
                    .parse()
                    .map_err(|_| RiskError::AccountNotFound(account.clone()))?,
            );

            return Err(RiskError::InsufficientBalance {
                account: account.clone(),
                available: available.to_string(),
                required: required.to_string(),
            });
        }

        Ok(())
    }

    /// Apply a committed entry to update internal state
    ///
    /// This should be called AFTER the entry is committed to ledger.
    pub fn apply(&mut self, entry: &JournalEntry) {
        self.state.apply_entry(entry);
    }

    /// Rebuild state from a sequence of entries (replay)
    pub fn replay<'a>(&mut self, entries: impl Iterator<Item = &'a JournalEntry>) {
        self.state.clear();
        for entry in entries {
            self.state.apply_entry(entry);
        }
    }
}

impl Default for RiskEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_core::Amount;
    use bibank_ledger::{AccountKey, JournalEntryBuilder, TransactionIntent};
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    #[test]
    fn test_deposit_always_allowed() {
        let engine = RiskEngine::new();

        let entry = JournalEntryBuilder::new()
            .intent(TransactionIntent::Deposit)
            .correlation_id("test-1")
            .debit(AccountKey::system_vault("USDT"), amount(100))
            .credit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .build_unsigned()
            .unwrap();

        assert!(engine.check(&entry).is_ok());
    }

    #[test]
    fn test_withdrawal_blocked_on_insufficient_balance() {
        let engine = RiskEngine::new();

        // No prior deposits, try to withdraw
        let entry = JournalEntryBuilder::new()
            .intent(TransactionIntent::Withdrawal)
            .correlation_id("test-1")
            .debit(AccountKey::user_available("ALICE", "USDT"), amount(100))
            .credit(AccountKey::system_vault("USDT"), amount(100))
            .build_unsigned()
            .unwrap();

        let result = engine.check(&entry);
        assert!(matches!(result, Err(RiskError::InsufficientBalance { .. })));
    }
}

```

## File ..\bibank\crates\risk\src\error.rs:
```rust
//! Risk engine errors

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum RiskError {
    #[error("Insufficient balance for {account}: available {available}, required {required}")]
    InsufficientBalance {
        account: String,
        available: String,
        required: String,
    },

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Risk check failed: {0}")]
    CheckFailed(String),
}

```

## File ..\bibank\crates\risk\src\lib.rs:
```rust
//! BiBank Risk Engine - Pre-commit gatekeeper
//!
//! The Risk Engine validates transactions BEFORE they are committed to the ledger.
//! It maintains in-memory state rebuilt from event replay on startup.

pub mod engine;
pub mod error;
pub mod state;

pub use engine::RiskEngine;
pub use error::RiskError;
pub use state::RiskState;

```

## File ..\bibank\crates\risk\src\state.rs:
```rust
//! In-memory risk state
//!
//! This state is rebuilt from ledger replay on startup.
//! It tracks balances for pre-commit validation.

use bibank_ledger::{AccountKey, JournalEntry, Posting};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// In-memory balance state for risk checking
#[derive(Debug, Default)]
pub struct RiskState {
    /// Balance per account (AccountKey string -> Decimal)
    /// Note: Can be negative during calculation, but post-check should be >= 0
    balances: HashMap<String, Decimal>,
}

impl RiskState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get balance for an account (returns 0 if not found)
    pub fn get_balance(&self, account: &AccountKey) -> Decimal {
        self.balances
            .get(&account.to_string())
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Apply a journal entry to update balances
    ///
    /// For liability accounts (user balances):
    /// - Credit increases balance (money owed to user)
    /// - Debit decreases balance (user spending)
    pub fn apply_entry(&mut self, entry: &JournalEntry) {
        for posting in &entry.postings {
            self.apply_posting(posting);
        }
    }

    /// Apply a single posting
    fn apply_posting(&mut self, posting: &Posting) {
        let key = posting.account.to_string();
        let current = self.balances.entry(key).or_insert(Decimal::ZERO);

        // For LIAB accounts: Credit = increase, Debit = decrease
        // For ASSET accounts: Debit = increase, Credit = decrease
        let normal_side = posting.account.category.normal_balance();

        let delta = if posting.side == normal_side {
            // Same as normal balance = increase
            posting.amount.value()
        } else {
            // Opposite = decrease
            -posting.amount.value()
        };

        *current += delta;
    }

    /// Calculate projected balances after applying an entry (without mutating state)
    pub fn project_balances(&self, entry: &JournalEntry) -> HashMap<String, Decimal> {
        let mut projected = self.balances.clone();

        for posting in &entry.postings {
            let key = posting.account.to_string();
            let current = projected.entry(key).or_insert(Decimal::ZERO);

            let normal_side = posting.account.category.normal_balance();
            let delta = if posting.side == normal_side {
                posting.amount.value()
            } else {
                -posting.amount.value()
            };

            *current += delta;
        }

        projected
    }

    /// Check if all LIAB accounts would have non-negative balance after applying entry
    pub fn check_sufficient_balance(&self, entry: &JournalEntry) -> Vec<(String, Decimal)> {
        let projected = self.project_balances(entry);
        let mut violations = Vec::new();

        for posting in &entry.postings {
            // Only check liability accounts (user balances)
            if posting.account.category == bibank_ledger::AccountCategory::Liability {
                let key = posting.account.to_string();
                if let Some(&balance) = projected.get(&key) {
                    if balance < Decimal::ZERO {
                        violations.push((key, balance));
                    }
                }
            }
        }

        violations
    }

    /// Get all account balances (for debugging/testing)
    pub fn all_balances(&self) -> &HashMap<String, Decimal> {
        &self.balances
    }

    /// Clear all state (for testing)
    pub fn clear(&mut self) {
        self.balances.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bibank_core::Amount;
    use bibank_ledger::TransactionIntent;
    use chrono::Utc;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    fn deposit_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "USDT"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_deposit_increases_user_balance() {
        let mut state = RiskState::new();
        let entry = deposit_entry("ALICE", 100);

        state.apply_entry(&entry);

        let alice_balance = state.get_balance(&AccountKey::user_available("ALICE", "USDT"));
        assert_eq!(alice_balance, Decimal::new(100, 0));
    }

    #[test]
    fn test_transfer_balance_check() {
        let mut state = RiskState::new();

        // Alice deposits 100
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Alice tries to transfer 150 to Bob
        let transfer = JournalEntry {
            sequence: 2,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Transfer,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(150)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(150)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&transfer);
        assert!(!violations.is_empty());
        assert!(violations[0].0.contains("ALICE"));
    }

    #[test]
    fn test_valid_transfer() {
        let mut state = RiskState::new();

        // Alice deposits 100
        state.apply_entry(&deposit_entry("ALICE", 100));

        // Alice transfers 50 to Bob
        let transfer = JournalEntry {
            sequence: 2,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Transfer,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(50)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(50)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&transfer);
        assert!(violations.is_empty());
    }

    // === Phase 2: Trade tests ===

    fn deposit_btc_entry(user: &str, val: i64) -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("BTC"), amount(val)),
                Posting::credit(AccountKey::user_available(user, "BTC"), amount(val)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_trade_balance_check_both_users() {
        let mut state = RiskState::new();

        // Alice deposits 100 USDT
        state.apply_entry(&deposit_entry("ALICE", 100));
        // Bob deposits 1 BTC
        state.apply_entry(&deposit_btc_entry("BOB", 1));

        // Trade: Alice sells 100 USDT for 1 BTC
        let trade = JournalEntry {
            sequence: 3,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Trade,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                // USDT leg
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                // BTC leg
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&trade);
        assert!(violations.is_empty(), "Valid trade should pass risk check");
    }

    #[test]
    fn test_trade_insufficient_balance_seller() {
        let mut state = RiskState::new();

        // Alice only has 50 USDT
        state.apply_entry(&deposit_entry("ALICE", 50));
        // Bob deposits 1 BTC
        state.apply_entry(&deposit_btc_entry("BOB", 1));

        // Trade: Alice tries to sell 100 USDT (insufficient)
        let trade = JournalEntry {
            sequence: 3,
            prev_hash: "test".to_string(),
            hash: "test".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Trade,
            correlation_id: "test".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("BOB", "USDT"), amount(100)),
                Posting::debit(AccountKey::user_available("BOB", "BTC"), amount(1)),
                Posting::credit(AccountKey::user_available("ALICE", "BTC"), amount(1)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        };

        let violations = state.check_sufficient_balance(&trade);
        assert!(!violations.is_empty(), "Trade should fail - Alice has insufficient USDT");
        assert!(violations.iter().any(|(acc, _)| acc.contains("ALICE")));
    }
}

```

## File ..\bibank\crates\rpc\src\commands.rs:
```rust
//! CLI commands

use bibank_core::Amount;
use bibank_ledger::{AccountCategory, AccountKey, JournalEntryBuilder, TransactionIntent, validate_intent};
use rust_decimal::Decimal;
use serde_json::json;

use crate::context::AppContext;

/// Initialize the system with Genesis entry
pub async fn init(ctx: &mut AppContext, correlation_id: &str) -> Result<(), anyhow::Error> {
    if ctx.is_initialized() {
        anyhow::bail!("System already initialized (sequence = {})", ctx.last_sequence());
    }

    // Create Genesis entry with initial system capital
    let initial_capital = Amount::new(Decimal::new(1_000_000_000, 0))?; // 1 billion units

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Genesis)
        .correlation_id(correlation_id)
        .debit(AccountKey::system_vault("USDT"), initial_capital)
        .credit(
            AccountKey::new(AccountCategory::Equity, "SYSTEM", "CAPITAL", "USDT", "MAIN"),
            initial_capital,
        )
        .build_unsigned()?;

    ctx.commit(entry).await?;

    println!("✅ System initialized with Genesis entry");
    Ok(())
}

/// Deposit funds to a user
pub async fn deposit(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Deposit)
        .correlation_id(correlation_id)
        .debit(AccountKey::system_vault(asset), amount)
        .credit(AccountKey::user_available(user_id, asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Deposited {} {} to {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Transfer funds between users
pub async fn transfer(
    ctx: &mut AppContext,
    from_user: &str,
    to_user: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Transfer)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(from_user, asset), amount)
        .credit(AccountKey::user_available(to_user, asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Transferred {} {} from {} to {} (seq: {})",
        amount, asset, from_user, to_user, committed.sequence
    );
    Ok(())
}

/// Withdraw funds from a user
pub async fn withdraw(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Withdrawal)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(user_id, asset), amount)
        .credit(AccountKey::system_vault(asset), amount)
        .build_unsigned()?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Withdrew {} {} from {} (seq: {})",
        amount, asset, user_id, committed.sequence
    );
    Ok(())
}

/// Get balance for a user
pub async fn balance(ctx: &AppContext, user_id: &str) -> Result<(), anyhow::Error> {
    let account = AccountKey::user_available(user_id, "USDT");
    let balance = ctx.risk.state().get_balance(&account);

    println!("Balance for {}: {} USDT", user_id, balance);

    // Check other common assets
    for asset in ["BTC", "ETH", "USD"] {
        let account = AccountKey::user_available(user_id, asset);
        let bal = ctx.risk.state().get_balance(&account);
        if !bal.is_zero() {
            println!("              {} {}", bal, asset);
        }
    }

    Ok(())
}

// === Phase 2: Trade and Fee commands ===

/// Execute a trade between two users
///
/// Alice sells `sell_amount` of `sell_asset` and buys `buy_amount` of `buy_asset` from Bob.
pub async fn trade(
    ctx: &mut AppContext,
    maker: &str,        // Alice - the one selling
    taker: &str,        // Bob - the one buying
    sell_amount: Decimal,
    sell_asset: &str,
    buy_amount: Decimal,
    buy_asset: &str,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let sell_amt = Amount::new(sell_amount)?;
    let buy_amt = Amount::new(buy_amount)?;

    // Calculate price for metadata
    let price = sell_amount / buy_amount;

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Trade)
        .correlation_id(correlation_id)
        // Sell leg: Maker pays sell_asset, Taker receives
        .debit(AccountKey::user_available(maker, sell_asset), sell_amt)
        .credit(AccountKey::user_available(taker, sell_asset), sell_amt)
        // Buy leg: Taker pays buy_asset, Maker receives
        .debit(AccountKey::user_available(taker, buy_asset), buy_amt)
        .credit(AccountKey::user_available(maker, buy_asset), buy_amt)
        // Metadata
        .metadata("trade_id", json!(correlation_id))
        .metadata("base_asset", json!(buy_asset))
        .metadata("quote_asset", json!(sell_asset))
        .metadata("price", json!(price.to_string()))
        .metadata("base_amount", json!(buy_amount.to_string()))
        .metadata("quote_amount", json!(sell_amount.to_string()))
        .metadata("maker", json!(maker))
        .metadata("taker", json!(taker))
        .build_unsigned()?;

    // Validate trade-specific rules
    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Trade executed: {} sells {} {} for {} {} from {} (seq: {})",
        maker, sell_amount, sell_asset, buy_amount, buy_asset, taker, committed.sequence
    );
    Ok(())
}

/// Charge a fee from a user
pub async fn fee(
    ctx: &mut AppContext,
    user_id: &str,
    amount: Decimal,
    asset: &str,
    fee_type: &str,  // "trading", "withdrawal", etc.
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    let amount = Amount::new(amount)?;

    // Fee account: REV:SYSTEM:FEE:<ASSET>:<FEE_TYPE>
    let fee_account = AccountKey::new(
        AccountCategory::Revenue,
        "SYSTEM",
        "FEE",
        asset,
        fee_type.to_uppercase(),
    );

    let entry = JournalEntryBuilder::new()
        .intent(TransactionIntent::Fee)
        .correlation_id(correlation_id)
        .debit(AccountKey::user_available(user_id, asset), amount)
        .credit(fee_account, amount)
        .metadata("fee_amount", json!(amount.to_string()))
        .metadata("fee_asset", json!(asset))
        .metadata("fee_type", json!(fee_type))
        .build_unsigned()?;

    // Validate fee-specific rules
    validate_intent(&entry)?;

    let committed = ctx.commit(entry).await?;

    println!(
        "✅ Fee charged: {} {} {} from {} (seq: {})",
        amount, asset, fee_type, user_id, committed.sequence
    );
    Ok(())
}

/// Execute a trade with fee (atomic: Trade + Fee entries)
pub async fn trade_with_fee(
    ctx: &mut AppContext,
    maker: &str,
    taker: &str,
    sell_amount: Decimal,
    sell_asset: &str,
    buy_amount: Decimal,
    buy_asset: &str,
    fee_amount: Decimal,
    correlation_id: &str,
) -> Result<(), anyhow::Error> {
    // First execute the trade
    trade(
        ctx, maker, taker,
        sell_amount, sell_asset,
        buy_amount, buy_asset,
        correlation_id,
    ).await?;

    // Then charge the fee (separate entry, atomic in business sense)
    let fee_correlation = format!("{}-fee", correlation_id);
    fee(ctx, maker, fee_amount, sell_asset, "trading", &fee_correlation).await?;

    Ok(())
}

// === Phase 2.1: Trade History ===

/// List trade history
pub async fn trades(
    ctx: &AppContext,
    user: Option<&str>,
    pair: Option<(&str, &str)>,
    limit: u32,
) -> Result<(), anyhow::Error> {
    let Some(ref projection) = ctx.projection else {
        anyhow::bail!("Projection not available");
    };

    let trades = if let Some(user_id) = user {
        projection.trade.get_user_trades(user_id).await?
    } else if let Some((base, quote)) = pair {
        projection.trade.get_pair_trades(base, quote).await?
    } else {
        projection.trade.get_recent_trades(limit).await?
    };

    if trades.is_empty() {
        println!("No trades found");
        return Ok(());
    }

    println!("Trade History ({} trades):", trades.len());
    println!("{:-<80}", "");
    println!(
        "{:>6} | {:>8} | {:>8} | {:>12} {:>6} | {:>12} {:>6}",
        "ID", "Seller", "Buyer", "Sold", "Asset", "Bought", "Asset"
    );
    println!("{:-<80}", "");

    for trade in trades.iter().take(limit as usize) {
        println!(
            "{:>6} | {:>8} | {:>8} | {:>12} {:>6} | {:>12} {:>6}",
            trade.trade_id,
            trade.seller,
            trade.buyer,
            trade.sell_amount,
            trade.sell_asset,
            trade.buy_amount,
            trade.buy_asset,
        );
    }

    Ok(())
}

```

## File ..\bibank\crates\rpc\src\context.rs:
```rust
//! Application context - wires everything together

use bibank_bus::EventBus;
use bibank_events::{EventReader, EventStore};
use bibank_ledger::{hash::calculate_entry_hash, JournalEntry, Signer, SystemSigner, UnsignedEntry};
use bibank_projection::ProjectionEngine;
use bibank_risk::{RiskEngine, RiskError};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Application context - wires together all components
pub struct AppContext {
    pub risk: RiskEngine,
    pub event_store: EventStore,
    pub bus: EventBus,
    pub projection: Option<ProjectionEngine>,
    pub signer: Option<Arc<dyn Signer>>,
    journal_path: PathBuf,
    projection_path: PathBuf,
    last_sequence: u64,
    last_hash: String,
}

impl AppContext {
    /// Create a new application context
    pub async fn new(data_path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let data_path = data_path.as_ref();
        let journal_path = data_path.join("journal");
        let projection_path = data_path.join("projection.db");

        // Create directories
        std::fs::create_dir_all(&journal_path)?;

        // Initialize components
        let event_store = EventStore::new(&journal_path)?;
        let bus = EventBus::new(&journal_path);
        let mut risk = RiskEngine::new();

        // Replay events to rebuild state
        let reader = EventReader::from_directory(&journal_path)?;
        let entries = reader.read_all()?;

        let (last_sequence, last_hash) = if let Some(last) = entries.last() {
            (last.sequence, last.hash.clone())
        } else {
            (0, "GENESIS".to_string())
        };

        // Rebuild risk state from events
        risk.replay(entries.iter());

        // Initialize projection
        let projection = ProjectionEngine::new(&projection_path).await.ok();

        // Replay projection if available
        if let Some(ref proj) = projection {
            proj.replay(&bus).await.ok();
        }

        // Initialize system signer from env var (Phase 2)
        let signer: Option<Arc<dyn Signer>> = std::env::var("BIBANK_SYSTEM_KEY")
            .ok()
            .and_then(|key| SystemSigner::from_hex(&key).ok())
            .map(|s| Arc::new(s) as Arc<dyn Signer>);

        Ok(Self {
            risk,
            event_store,
            bus,
            projection,
            signer,
            journal_path,
            projection_path,
            last_sequence,
            last_hash,
        })
    }

    /// Commit an unsigned entry
    ///
    /// Flow: Risk Check → Sign → Append → Apply
    pub async fn commit(&mut self, unsigned: UnsignedEntry) -> Result<JournalEntry, CommitError> {
        // 1. Validate double-entry balance
        unsigned.validate_balance().map_err(CommitError::Ledger)?;

        // 2. Risk check (pre-commit gatekeeper)
        self.risk.check(&unsigned).map_err(CommitError::Risk)?;

        // 3. Sign the entry (add sequence, prev_hash, hash, timestamp)
        let sequence = self.last_sequence + 1;
        let prev_hash = self.last_hash.clone();
        let timestamp = Utc::now();

        let mut entry = JournalEntry {
            sequence,
            prev_hash,
            hash: String::new(),
            timestamp,
            intent: unsigned.intent,
            correlation_id: unsigned.correlation_id,
            causality_id: unsigned.causality_id,
            postings: unsigned.postings,
            metadata: unsigned.metadata,
            signatures: Vec::new(), // Phase 2: Will be signed after hash calculation
        };

        entry.hash = calculate_entry_hash(&entry);

        // 4. Sign the entry with system key (Phase 2)
        if let Some(ref signer) = self.signer {
            let signature = signer.sign(&entry);
            entry.signatures.push(signature);
        }

        // 5. Validate the signed entry
        entry.validate().map_err(CommitError::Ledger)?;

        // 6. Append to event store (Source of Truth)
        self.event_store
            .append(&entry)
            .map_err(CommitError::Event)?;

        // 7. Update risk state
        self.risk.apply(&entry);

        // 8. Update projection (if available)
        if let Some(ref projection) = self.projection {
            projection.apply(&entry).await.ok();
        }

        // 9. Update last sequence/hash
        self.last_sequence = entry.sequence;
        self.last_hash = entry.hash.clone();

        Ok(entry)
    }

    /// Get journal path
    pub fn journal_path(&self) -> &Path {
        &self.journal_path
    }

    /// Get projection path
    pub fn projection_path(&self) -> &Path {
        &self.projection_path
    }

    /// Check if system is initialized (has Genesis entry)
    pub fn is_initialized(&self) -> bool {
        self.last_sequence > 0
    }

    /// Get last sequence number
    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
}

/// Errors during commit
#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error("Ledger error: {0}")]
    Ledger(#[from] bibank_ledger::LedgerError),

    #[error("Risk error: {0}")]
    Risk(RiskError),

    #[error("Event store error: {0}")]
    Event(#[from] bibank_events::EventError),
}

```

## File ..\bibank\crates\rpc\src\lib.rs:
```rust
//! BiBank RPC - API/CLI orchestrator
//!
//! This crate provides the CLI binary and command orchestration.

pub mod commands;
pub mod context;

pub use context::AppContext;

```

## File ..\bibank\crates\rpc\src\main.rs:
```rust
//! BiBank CLI - Main entry point

use bibank_rpc::{commands, AppContext};
use clap::{Parser, Subcommand};
use rust_decimal::Decimal;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "bibank")]
#[command(about = "BiBank - Financial State OS", long_about = None)]
struct Cli {
    /// Data directory path
    #[arg(short, long, default_value = "./data")]
    data: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the system with Genesis entry
    Init,

    /// Deposit funds to a user
    Deposit {
        /// User ID (will be uppercased)
        user: String,
        /// Amount to deposit
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Transfer funds between users
    Transfer {
        /// Source user ID
        from: String,
        /// Destination user ID
        to: String,
        /// Amount to transfer
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Withdraw funds from a user
    Withdraw {
        /// User ID
        user: String,
        /// Amount to withdraw
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Check balance for a user
    Balance {
        /// User ID
        user: String,
    },

    /// Replay events (rebuild projections)
    Replay {
        /// Drop projections before replay
        #[arg(long)]
        reset: bool,
    },

    /// Audit the ledger (verify hash chain)
    Audit {
        /// Also verify digital signatures
        #[arg(long)]
        verify_signatures: bool,
    },

    // === Phase 2: Trade and Fee ===

    /// Execute a trade between two users
    Trade {
        /// Maker user ID (seller)
        maker: String,
        /// Taker user ID (buyer)
        taker: String,
        /// Amount to sell
        #[arg(long)]
        sell: Decimal,
        /// Asset to sell
        #[arg(long)]
        sell_asset: String,
        /// Amount to buy
        #[arg(long)]
        buy: Decimal,
        /// Asset to buy
        #[arg(long)]
        buy_asset: String,
        /// Optional fee amount (charged to maker)
        #[arg(long)]
        fee: Option<Decimal>,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Charge a fee from a user
    Fee {
        /// User ID
        user: String,
        /// Fee amount
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Fee type (trading, withdrawal, etc.)
        #[arg(long, default_value = "trading")]
        fee_type: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Generate a new system key
    Keygen {
        /// Output file path
        #[arg(long, default_value = "system.key")]
        output: PathBuf,
    },

    // === Phase 2.1: Trade History ===

    /// List trade history
    Trades {
        /// Filter by user ID
        #[arg(long)]
        user: Option<String>,
        /// Filter by base asset (requires --quote)
        #[arg(long)]
        base: Option<String>,
        /// Filter by quote asset (requires --base)
        #[arg(long)]
        quote: Option<String>,
        /// Maximum number of trades to show
        #[arg(long, default_value = "20")]
        limit: u32,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Create application context
    let mut ctx = AppContext::new(&cli.data).await?;

    match cli.command {
        Commands::Init => {
            let correlation_id = Uuid::new_v4().to_string();
            commands::init(&mut ctx, &correlation_id).await?;
        }

        Commands::Deposit {
            user,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::deposit(&mut ctx, &user, amount, &asset, &correlation_id).await?;
        }

        Commands::Transfer {
            from,
            to,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::transfer(&mut ctx, &from, &to, amount, &asset, &correlation_id).await?;
        }

        Commands::Withdraw {
            user,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::withdraw(&mut ctx, &user, amount, &asset, &correlation_id).await?;
        }

        Commands::Balance { user } => {
            commands::balance(&ctx, &user).await?;
        }

        Commands::Replay { reset } => {
            let projection_path = ctx.projection_path().to_path_buf();
            let data_path = cli.data.clone();

            // Drop existing context to release SQLite connection
            drop(ctx);

            if reset {
                println!("🗑️  Dropping projections...");
                if projection_path.exists() {
                    std::fs::remove_file(&projection_path)?;
                    println!("   Deleted {}", projection_path.display());
                }
            }

            // Recreate context to replay
            let new_ctx = AppContext::new(&data_path).await?;
            println!(
                "✅ Replayed {} entries",
                new_ctx.last_sequence()
            );
        }

        Commands::Audit { verify_signatures } => {
            use bibank_events::EventReader;
            use bibank_ledger::hash::verify_chain;

            let reader = EventReader::from_directory(ctx.journal_path())?;
            let entries = reader.read_all()?;

            // Verify hash chain
            match verify_chain(&entries) {
                Ok(()) => {
                    println!("✅ Hash chain verified ({} entries)", entries.len());
                }
                Err(e) => {
                    println!("❌ Hash chain broken: {}", e);
                    return Ok(());
                }
            }

            // Verify signatures if requested
            if verify_signatures {
                let mut signed_count = 0;
                let mut unsigned_count = 0;

                for entry in &entries {
                    if entry.signatures.is_empty() {
                        unsigned_count += 1;
                    } else {
                        match entry.verify_signatures() {
                            Ok(()) => signed_count += 1,
                            Err(e) => {
                                println!("❌ Signature verification failed at seq {}: {}", entry.sequence, e);
                                return Ok(());
                            }
                        }
                    }
                }

                println!("✅ Signatures verified: {} signed, {} unsigned (Phase 1)", signed_count, unsigned_count);
            }
        }

        Commands::Trade {
            maker,
            taker,
            sell,
            sell_asset,
            buy,
            buy_asset,
            fee,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());

            if let Some(fee_amount) = fee {
                commands::trade_with_fee(
                    &mut ctx, &maker, &taker,
                    sell, &sell_asset,
                    buy, &buy_asset,
                    fee_amount,
                    &correlation_id,
                ).await?;
            } else {
                commands::trade(
                    &mut ctx, &maker, &taker,
                    sell, &sell_asset,
                    buy, &buy_asset,
                    &correlation_id,
                ).await?;
            }
        }

        Commands::Fee {
            user,
            amount,
            asset,
            fee_type,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::fee(&mut ctx, &user, amount, &asset, &fee_type, &correlation_id).await?;
        }

        Commands::Keygen { output } => {
            use bibank_ledger::{Signer, SystemSigner};

            let signer = SystemSigner::generate();
            let seed = signer.seed_hex();
            let pubkey = signer.public_key_hex();

            std::fs::write(&output, &seed)?;
            println!("✅ Generated system key");
            println!("   Private key saved to: {}", output.display());
            println!("   Public key: {}", pubkey);
            println!("");
            println!("To use: export BIBANK_SYSTEM_KEY={}", seed);
        }

        Commands::Trades {
            user,
            base,
            quote,
            limit,
        } => {
            let pair = match (&base, &quote) {
                (Some(b), Some(q)) => Some((b.as_str(), q.as_str())),
                _ => None,
            };
            commands::trades(&ctx, user.as_deref(), pair, limit).await?;
        }
    }

    Ok(())
}

```

## Cargo.toml dependencies:
```toml
members = [
resolver = "2"
version = "0.1.0"
edition = "2021"
authors = ["BiBank Team"]
license = "MIT"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rust_decimal = { version = "1.33", features = ["serde-with-str", "maths"] }
thiserror = "2.0"
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
hex = "0.4"
ed25519-dalek = { version = "2.1", features = ["rand_core"] }
rand = "0.8"
async-trait = "0.1"
strum = { version = "0.26", features = ["derive"] }
strum_macros = "0.26"
tokio = { version = "1.36", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1.7", features = ["serde", "v4"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono"] }
clap = { version = "4.4", features = ["derive"] }
tempfile = "3.24"
bibank-core = { path = "./crates/core" }
bibank-ledger = { path = "./crates/ledger" }
bibank-risk = { path = "./crates/risk" }
bibank-events = { path = "./crates/events" }
bibank-bus = { path = "./crates/bus" }
bibank-projection = { path = "./crates/projection" }
bibank-rpc = { path = "./crates/rpc" }
bibank-dsl = { path = "./crates/dsl" }
```

