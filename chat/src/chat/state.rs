use axum::extract::ws::Message;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, RwLock};
use uuid::Uuid;

/// Represents a connected client
#[derive(Debug, Clone)]
pub struct Client {
    pub nickname: String,
    pub sender: UnboundedSender<Message>,
}

impl Client {
    pub fn new(nickname: String, sender: UnboundedSender<Message>) -> Self {
        Self { nickname, sender }
    }

    /// Send a text message to this client
    /// Returns true if successful, false if the channel is closed
    pub fn send(&self, message: &str) -> bool {
        self.sender
            .send(Message::Text(message.to_string()))
            .is_ok()
    }
}

/// Shared state for the chat application
#[derive(Clone)]
pub struct ChatState {
    clients: Arc<RwLock<HashMap<Uuid, Client>>>,
    next_guest_id: Arc<AtomicU64>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatState {
    /// Create a new ChatState
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            next_guest_id: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Generate a unique nickname (Guest0, Guest1, ...)
    pub fn generate_nickname(&self) -> String {
        let id = self.next_guest_id.fetch_add(1, Ordering::SeqCst);
        format!("Guest{}", id)
    }

    /// Add a new client to the chat
    pub async fn add_client(&self, id: Uuid, client: Client) {
        let mut clients = self.clients.write().await;
        clients.insert(id, client);
    }

    /// Remove a client from the chat
    pub async fn remove_client(&self, id: &Uuid) -> Option<Client> {
        let mut clients = self.clients.write().await;
        clients.remove(id)
    }

    /// Get the number of connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Broadcast a message to all connected clients
    pub async fn broadcast(&self, message: &str) {
        let clients = self.clients.read().await;
        let mut failed_ids = Vec::new();

        for (id, client) in clients.iter() {
            if !client.send(message) {
                failed_ids.push(*id);
            }
        }

        drop(clients); // Release read lock before write

        // Remove failed clients and notify others
        for id in failed_ids {
            if let Some(client) = self.remove_client(&id).await {
                tracing::info!("Removed disconnected client: {}", client.nickname);
                let msg = format!("* {} has been disconnected.", client.nickname);
                // Use Box::pin to avoid infinite recursion in async
                Box::pin(self.broadcast(&msg)).await;
            }
        }
    }

    /// Broadcast a message to all clients except the sender
    pub async fn broadcast_except(&self, message: &str, sender_id: &Uuid) {
        let clients = self.clients.read().await;

        for (id, client) in clients.iter() {
            if id != sender_id {
                let _ = client.send(message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_nickname_generation() {
        let state = ChatState::new();
        assert_eq!(state.generate_nickname(), "Guest0");
        assert_eq!(state.generate_nickname(), "Guest1");
        assert_eq!(state.generate_nickname(), "Guest2");
    }

    #[tokio::test]
    async fn test_add_and_remove_client() {
        let state = ChatState::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = Client::new("Guest0".to_string(), tx);
        let id = Uuid::new_v4();

        state.add_client(id, client).await;
        assert_eq!(state.client_count().await, 1);

        let removed = state.remove_client(&id).await;
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().nickname, "Guest0");
        assert_eq!(state.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast_to_multiple_clients() {
        let state = ChatState::new();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();

        let client1 = Client::new("Guest0".to_string(), tx1);
        let client2 = Client::new("Guest1".to_string(), tx2);

        state.add_client(Uuid::new_v4(), client1).await;
        state.add_client(Uuid::new_v4(), client2).await;

        state.broadcast("Hello everyone!").await;

        let msg1 = rx1.try_recv();
        let msg2 = rx2.try_recv();

        assert!(msg1.is_ok());
        assert!(msg2.is_ok());

        if let Message::Text(text) = msg1.unwrap() {
            assert_eq!(text, "Hello everyone!");
        }
    }

    #[tokio::test]
    async fn test_client_send() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client = Client::new("TestUser".to_string(), tx);

        assert!(client.send("Test message"));

        let received = rx.try_recv();
        assert!(received.is_ok());
        if let Message::Text(text) = received.unwrap() {
            assert_eq!(text, "Test message");
        }
    }

    #[tokio::test]
    async fn test_broadcast_except() {
        let state = ChatState::new();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        state
            .add_client(id1, Client::new("Guest0".to_string(), tx1))
            .await;
        state
            .add_client(id2, Client::new("Guest1".to_string(), tx2))
            .await;

        state.broadcast_except("Message from Guest0", &id1).await;

        // Guest0 should NOT receive the message
        assert!(rx1.try_recv().is_err());
        // Guest1 SHOULD receive the message
        assert!(rx2.try_recv().is_ok());
    }
}
