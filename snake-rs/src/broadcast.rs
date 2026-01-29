//! Broadcaster abstraction for message broadcasting
//!
//! Current implementation uses in-memory tokio broadcast channel.
//! Can be swapped with Redis pub/sub for horizontal scaling.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::debug;

use crate::config::BROADCAST_CAPACITY;
use crate::protocol::ServerMessage;

/// Receiver type for broadcast messages
pub type BroadcastReceiver = broadcast::Receiver<Arc<ServerMessage>>;

/// Trait for broadcasting messages to all connected clients
#[async_trait]
pub trait Broadcaster: Send + Sync {
    /// Subscribe to receive broadcast messages
    fn subscribe(&self) -> BroadcastReceiver;

    /// Send a message to all subscribers
    async fn send(&self, msg: ServerMessage);

    /// Get the number of active subscribers
    fn subscriber_count(&self) -> usize;
}

/// In-memory broadcaster using tokio broadcast channel
pub struct InMemoryBroadcaster {
    tx: broadcast::Sender<Arc<ServerMessage>>,
}

impl InMemoryBroadcaster {
    /// Create a new in-memory broadcaster
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self { tx }
    }

    /// Create with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }
}

impl Default for InMemoryBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Broadcaster for InMemoryBroadcaster {
    fn subscribe(&self) -> BroadcastReceiver {
        self.tx.subscribe()
    }

    async fn send(&self, msg: ServerMessage) {
        // Wrap in Arc for zero-copy broadcast
        let msg = Arc::new(msg);

        // send() returns error if there are no receivers, which is fine
        if let Err(e) = self.tx.send(msg) {
            debug!("Broadcast send (no receivers): {}", e);
        }
    }

    fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

// =============================================================================
// Future Redis implementation (placeholder)
// =============================================================================

/// Redis broadcaster for horizontal scaling (placeholder)
///
/// To implement:
/// 1. Add `redis = "0.24"` to Cargo.toml
/// 2. Implement the Broadcaster trait with Redis pub/sub
///
/// ```ignore
/// pub struct RedisBroadcaster {
///     client: redis::Client,
///     channel: String,
///     local_tx: broadcast::Sender<Arc<ServerMessage>>,
/// }
///
/// impl RedisBroadcaster {
///     pub async fn new(redis_url: &str, channel: &str) -> Result<Self, redis::RedisError> {
///         let client = redis::Client::open(redis_url)?;
///         let (local_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
///
///         // Spawn subscriber task to receive from Redis and forward locally
///         // ...
///
///         Ok(Self { client, channel: channel.to_string(), local_tx })
///     }
/// }
///
/// #[async_trait]
/// impl Broadcaster for RedisBroadcaster {
///     fn subscribe(&self) -> BroadcastReceiver {
///         self.local_tx.subscribe()
///     }
///
///     async fn send(&self, msg: ServerMessage) {
///         let json = msg.to_json();
///         // Publish to Redis channel
///         // The subscriber task will receive and broadcast locally
///     }
///
///     fn subscriber_count(&self) -> usize {
///         self.local_tx.receiver_count()
///     }
/// }
/// ```

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::PlayerInfo;

    #[tokio::test]
    async fn test_broadcast_message() {
        let broadcaster = InMemoryBroadcaster::new();
        let mut rx1 = broadcaster.subscribe();
        let mut rx2 = broadcaster.subscribe();

        let msg = ServerMessage::Join {
            data: vec![PlayerInfo {
                id: 1,
                color: "#FF0000".to_string(),
            }],
        };

        broadcaster.send(msg).await;

        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        // Both receivers should get the same message
        assert!(matches!(received1.as_ref(), ServerMessage::Join { .. }));
        assert!(matches!(received2.as_ref(), ServerMessage::Join { .. }));
    }

    #[tokio::test]
    async fn test_subscriber_count() {
        let broadcaster = InMemoryBroadcaster::new();
        assert_eq!(broadcaster.subscriber_count(), 0);

        let _rx1 = broadcaster.subscribe();
        assert_eq!(broadcaster.subscriber_count(), 1);

        let _rx2 = broadcaster.subscribe();
        assert_eq!(broadcaster.subscriber_count(), 2);
    }
}
