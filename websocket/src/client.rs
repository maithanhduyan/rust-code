//! WebSocket Client
//! 
//! A simple WebSocket client that connects to the server,
//! sends messages, and receives broadcasts from other clients.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Message structure for chat communication
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    username: String,
    content: String,
    timestamp: String,
    message_type: MessageType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MessageType {
    Chat,
    Join,
    Leave,
    System,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "ws://127.0.0.1:8080";
    
    println!("🔌 Connecting to WebSocket server at {}...", url);
    
    let (ws_stream, _) = connect_async(url).await?;
    println!("✅ Connected successfully!\n");
    println!("📝 Commands:");
    println!("   /name <new_name> - Change your username");
    println!("   /quit            - Exit the chat");
    println!("   Just type        - Send a message\n");
    println!("─────────────────────────────────────────\n");

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Task to handle incoming messages
    let receive_task = tokio::spawn(async move {
        while let Some(result) = ws_receiver.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    if let Ok(msg) = serde_json::from_str::<ChatMessage>(&text) {
                        print_message(&msg);
                    } else {
                        println!("📩 {}", text);
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("\n🔌 Server closed the connection");
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("\n❌ Error: {}", e);
                    break;
                }
            }
        }
    });

    // Task to handle user input
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

            if input == "/quit" {
                println!("👋 Goodbye!");
                let _ = ws_sender.send(Message::Close(None)).await;
                break;
            }

            if ws_sender.send(Message::Text(input.to_string())).await.is_err() {
                eprintln!("❌ Failed to send message");
                break;
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = receive_task => {}
        _ = send_task => {}
    }

    Ok(())
}

fn print_message(msg: &ChatMessage) {
    match msg.message_type {
        MessageType::Chat => {
            println!("\r[{}] {}: {}", msg.timestamp, msg.username, msg.content);
        }
        MessageType::Join => {
            println!("\r[{}] 🟢 {}", msg.timestamp, msg.content);
        }
        MessageType::Leave => {
            println!("\r[{}] 🔴 {}", msg.timestamp, msg.content);
        }
        MessageType::System => {
            println!("\r[{}] ℹ️  {}", msg.timestamp, msg.content);
        }
    }
    print!("> ");
    io::stdout().flush().unwrap();
}
