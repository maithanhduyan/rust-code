//! Protocol messages for WebSocket communication

use serde::{Deserialize, Serialize};

use crate::game::direction::Direction;

/// Messages sent from client to server
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ClientMessage {
    /// Direction change command
    Direction(Direction),
    /// Ping to keep connection alive
    Ping(String),
}

impl ClientMessage {
    /// Parse a client message from a string
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();

        // Check for direction commands
        if let Some(dir) = Direction::from_str(s) {
            return Some(ClientMessage::Direction(dir));
        }

        // Check for ping
        if s == "ping" {
            return Some(ClientMessage::Ping("pong".to_string()));
        }

        None
    }
}

/// Player information for join messages
#[derive(Debug, Clone, Serialize)]
pub struct PlayerInfo {
    pub id: u32,
    pub color: String,
}

/// Snake body position
#[derive(Debug, Clone, Serialize)]
pub struct SnakePosition {
    pub x: i32,
    pub y: i32,
}

/// Snake body data for update messages
#[derive(Debug, Clone, Serialize)]
pub struct SnakeData {
    pub id: u32,
    pub color: String,
    pub body: Vec<SnakePosition>,
}

/// Food position data
#[derive(Debug, Clone, Serialize)]
pub struct FoodData {
    pub x: i32,
    pub y: i32,
}

/// Messages sent from server to client
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerMessage {
    /// Player joined the game
    Join {
        data: Vec<PlayerInfo>,
    },
    /// Game state update
    Update {
        data: Vec<SnakeData>,
        food: Vec<FoodData>,
    },
    /// Player left the game
    Leave {
        id: u32,
    },
    /// Current player died
    Dead,
    /// Current player killed someone
    Kill,
}

impl ServerMessage {
    /// Serialize message to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_direction() {
        assert!(matches!(
            ClientMessage::parse("north"),
            Some(ClientMessage::Direction(Direction::North))
        ));
        assert!(matches!(
            ClientMessage::parse("south"),
            Some(ClientMessage::Direction(Direction::South))
        ));
    }

    #[test]
    fn test_parse_ping() {
        assert!(matches!(
            ClientMessage::parse("ping"),
            Some(ClientMessage::Ping(_))
        ));
    }

    #[test]
    fn test_server_message_json() {
        let msg = ServerMessage::Dead;
        assert_eq!(msg.to_json(), r#"{"type":"dead"}"#);
    }
}
