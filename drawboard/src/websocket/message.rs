use axum::extract::ws::Message;

/// Message types sent from server to client
#[derive(Debug, Clone)]
pub enum ServerMessage {
    /// '0': Error message
    Error(String),
    /// '1': Draw message(s)
    DrawMessage(String),
    /// '2': Image message with player count (followed by binary PNG)
    ImageMessage(usize),
    /// '3': Player changed (true = joined, false = left)
    PlayerChanged(bool),
}

impl ServerMessage {
    pub fn to_ws_message(&self) -> Message {
        match self {
            ServerMessage::Error(msg) => Message::Text(format!("0{}", msg)),
            ServerMessage::DrawMessage(data) => Message::Text(format!("1{}", data)),
            ServerMessage::ImageMessage(count) => Message::Text(format!("2{}", count)),
            ServerMessage::PlayerChanged(joined) => {
                Message::Text(format!("3{}", if *joined { "+" } else { "-" }))
            }
        }
    }
}

/// Message types sent from client to server
#[derive(Debug, Clone)]
pub enum ClientMessage {
    /// '0': Pong (keepalive)
    Pong,
    /// '1': Draw message with ID
    Draw { msg_id: u64, data: String },
}

impl ClientMessage {
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }

        let msg_type = text.chars().next()?;
        let content = &text[1..];

        match msg_type {
            '0' => Some(ClientMessage::Pong),
            '1' => {
                let (id, data) = content.split_once('|')?;
                Some(ClientMessage::Draw {
                    msg_id: id.parse().ok()?,
                    data: data.to_string(),
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pong() {
        let msg = ClientMessage::parse("0").unwrap();
        assert!(matches!(msg, ClientMessage::Pong));
    }

    #[test]
    fn test_parse_draw() {
        let msg = ClientMessage::parse("11|1,255,0,0,255,5,10,10,50,50").unwrap();
        if let ClientMessage::Draw { msg_id, data } = msg {
            assert_eq!(msg_id, 1);
            assert_eq!(data, "1,255,0,0,255,5,10,10,50,50");
        } else {
            panic!("Expected Draw message");
        }
    }

    #[test]
    fn test_server_message_error() {
        let msg = ServerMessage::Error("Test error".to_string());
        if let Message::Text(text) = msg.to_ws_message() {
            assert_eq!(text, "0Test error");
        }
    }

    #[test]
    fn test_server_message_player_changed() {
        let join = ServerMessage::PlayerChanged(true);
        let leave = ServerMessage::PlayerChanged(false);

        if let Message::Text(text) = join.to_ws_message() {
            assert_eq!(text, "3+");
        }
        if let Message::Text(text) = leave.to_ws_message() {
            assert_eq!(text, "3-");
        }
    }
}
