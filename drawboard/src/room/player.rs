use axum::extract::ws::Message;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct Player {
    pub sender: UnboundedSender<Message>,
    pub last_received_message_id: u64,
    pub buffered_messages: Vec<String>,
}

impl Player {
    pub fn new(sender: UnboundedSender<Message>) -> Self {
        Self {
            sender,
            last_received_message_id: 0,
            buffered_messages: Vec::new(),
        }
    }

    /// Send a message to this player
    pub fn send(&self, message: Message) -> bool {
        self.sender.send(message).is_ok()
    }

    /// Buffer a draw message for later broadcast
    pub fn buffer_message(&mut self, msg: String) {
        self.buffered_messages.push(msg);
    }

    /// Take all buffered messages
    pub fn take_buffered_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.buffered_messages)
    }

    /// Check if there are buffered messages
    pub fn has_buffered_messages(&self) -> bool {
        !self.buffered_messages.is_empty()
    }
}
