use axum::extract::ws::Message;
use std::collections::HashMap;
use uuid::Uuid;

use crate::drawing::{Canvas, DrawMessage};
use crate::room::Player;

/// Maximum number of players allowed in a room
pub const MAX_PLAYER_COUNT: usize = 100;

pub struct Room {
    players: HashMap<Uuid, Player>,
    canvas: Canvas,
    closed: bool,
}

impl Room {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            players: HashMap::new(),
            canvas: Canvas::new(width, height),
            closed: false,
        }
    }

    /// Add a player to the room
    pub fn add_player(&mut self, id: Uuid, player: Player) -> bool {
        if self.closed || self.players.len() >= MAX_PLAYER_COUNT {
            return false;
        }
        self.players.insert(id, player);
        true
    }

    /// Remove a player from the room
    pub fn remove_player(&mut self, id: &Uuid) -> Option<Player> {
        self.players.remove(id)
    }

    /// Get the current player count
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Check if room is full
    pub fn is_full(&self) -> bool {
        self.players.len() >= MAX_PLAYER_COUNT
    }

    /// Get the current canvas as PNG bytes
    pub fn get_canvas_png(&self) -> Vec<u8> {
        self.canvas.to_png()
    }

    /// Handle a draw message from a player
    pub fn handle_draw_message(&mut self, player_id: Uuid, msg: DrawMessage, msg_id: u64) {
        if self.closed {
            return;
        }

        // Draw on server canvas
        self.canvas.draw(&msg);

        // Update player's last message id
        if let Some(player) = self.players.get_mut(&player_id) {
            player.last_received_message_id = msg_id;
        }

        // Buffer message for all players
        let msg_string = msg.to_string();
        for player in self.players.values_mut() {
            player.buffer_message(msg_string.clone());
        }
    }

    /// Broadcast a simple text message to all players
    pub fn broadcast_message(&self, message: &str) {
        for player in self.players.values() {
            let _ = player.send(Message::Text(message.to_string()));
        }
    }

    /// Flush buffered draw messages for all players
    pub fn flush_buffered_messages(&mut self) {
        for player in self.players.values_mut() {
            if player.has_buffered_messages() {
                let messages = player.take_buffered_messages();
                let batched: Vec<String> = messages
                    .into_iter()
                    .map(|m| format!("{},{}", player.last_received_message_id, m))
                    .collect();

                let message = format!("1{}", batched.join("|"));
                let _ = player.send(Message::Text(message));
            }
        }
    }

    /// Shutdown the room
    pub fn shutdown(&mut self) {
        self.closed = true;
        self.players.clear();
    }

    /// Check if room is closed
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Get canvas dimensions
    pub fn canvas_size(&self) -> (u32, u32) {
        (self.canvas.width(), self.canvas.height())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::DrawType;
    use tokio::sync::mpsc;

    #[test]
    fn test_new_room() {
        let room = Room::new(900, 600);
        assert_eq!(room.player_count(), 0);
        assert!(!room.is_closed());
    }

    #[test]
    fn test_add_remove_player() {
        let mut room = Room::new(900, 600);
        let (tx, _rx) = mpsc::unbounded_channel();
        let player = Player::new(tx);
        let id = Uuid::new_v4();

        assert!(room.add_player(id, player));
        assert_eq!(room.player_count(), 1);

        room.remove_player(&id);
        assert_eq!(room.player_count(), 0);
    }

    #[test]
    fn test_room_shutdown() {
        let mut room = Room::new(100, 100);
        let (tx, _rx) = mpsc::unbounded_channel();
        let player = Player::new(tx);
        let id = Uuid::new_v4();

        room.add_player(id, player);
        assert_eq!(room.player_count(), 1);

        room.shutdown();
        assert!(room.is_closed());
        assert_eq!(room.player_count(), 0);
    }

    #[test]
    fn test_handle_draw_message() {
        let mut room = Room::new(100, 100);
        let (tx, _rx) = mpsc::unbounded_channel();
        let player = Player::new(tx);
        let id = Uuid::new_v4();

        room.add_player(id, player);

        let msg = DrawMessage::new(
            DrawType::Line,
            (255, 0, 0, 255),
            5.0,
            (10.0, 10.0),
            (50.0, 50.0),
        );

        room.handle_draw_message(id, msg, 1);
        // Message should be buffered
    }
}
