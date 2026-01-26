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

