//! Binary Protocol WebSocket Client

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    Chat = 1,
    Join = 2,
    Leave = 3,
    System = 4,
    Ping = 5,
    Pong = 6,
    Ack = 7,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: u64,
    pub msg_type: MessageType,
    pub user_id: u64,
    pub username: String,
    pub payload: Vec<u8>,
    pub timestamp: i64,
    pub room: String,
}

impl ChatMessage {
    pub fn new(msg_type: MessageType, payload: &str) -> Self {
        static MSG_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: MSG_ID.fetch_add(1, Ordering::Relaxed),
            msg_type,
            user_id: 0,
            username: String::new(),
            payload: payload.as_bytes().to_vec(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            room: "general".to_string(),
        }
    }

    pub fn encode(&self) -> Bytes {
        Bytes::from(bincode::serialize(self).expect("Failed to encode"))
    }

    pub fn decode(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }

    pub fn payload_str(&self) -> String {
        String::from_utf8_lossy(&self.payload).to_string()
    }
}

fn format_message(msg: &ChatMessage) -> String {
    let time = chrono::DateTime::from_timestamp_millis(msg.timestamp)
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_default();

    match msg.msg_type {
        MessageType::Chat => format!("[{}] {}: {}", time, msg.username, msg.payload_str()),
        MessageType::Join => format!("[{}] 🟢 {}", time, msg.payload_str()),
        MessageType::Leave => format!("[{}] 🔴 {}", time, msg.payload_str()),
        MessageType::System => format!("[{}] ℹ️  {}", time, msg.payload_str()),
        MessageType::Pong => format!("[{}] 🏓 Pong", time),
        _ => format!("[{}] {}", time, msg.payload_str()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ws://127.0.0.1:8080".to_string());

    println!("🔌 Connecting to {}...", url);

    let (ws_stream, _) = connect_async(&url).await?;
    println!("✅ Connected! (Binary Protocol)\n");
    println!("Commands: /name <n>, /ping, /quit\n");
    println!("─────────────────────────────────────────\n");

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Receive task
    let receive_task = tokio::spawn(async move {
        while let Some(result) = ws_receiver.next().await {
            match result {
                Ok(WsMessage::Binary(data)) => {
                    if let Ok(msg) = ChatMessage::decode(&data) {
                        println!("\r{}", format_message(&msg));
                        print!("> ");
                        io::stdout().flush().unwrap();
                    }
                }
                Ok(WsMessage::Text(text)) => {
                    println!("\r📩 {}", text);
                    print!("> ");
                    io::stdout().flush().unwrap();
                }
                Ok(WsMessage::Close(_)) => {
                    println!("\n🔌 Disconnected");
                    break;
                }
                _ => {}
            }
        }
    });

    // Send task
    let send_task = tokio::spawn(async move {
        let stdin = io::stdin();
        loop {
            print!("> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if stdin.read_line(&mut input).is_err() {
                break;
            }

            let input = input.trim();
            if input.is_empty() {
                continue;
            }

            match input {
                "/quit" => {
                    println!("👋 Bye!");
                    let _ = ws_sender.send(WsMessage::Close(None)).await;
                    break;
                }
                "/ping" => {
                    let msg = ChatMessage::new(MessageType::Ping, "");
                    let _ = ws_sender.send(WsMessage::Binary(msg.encode().to_vec())).await;
                }
                _ => {
                    let msg = ChatMessage::new(MessageType::Chat, input);
                    let _ = ws_sender.send(WsMessage::Binary(msg.encode().to_vec())).await;
                }
            }
        }
    });

    tokio::select! {
        _ = receive_task => {}
        _ = send_task => {}
    }

    Ok(())
}
