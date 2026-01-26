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
