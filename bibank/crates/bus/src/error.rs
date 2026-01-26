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
