//! WebSocket Server
//! 
//! A simple WebSocket server that handles multiple client connections,
//! broadcasts messages, and supports basic chat functionality.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};

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

/// Connected client information
#[derive(Debug, Clone)]
struct Client {
    addr: SocketAddr,
    username: String,
}

/// Shared state between all connections
struct AppState {
    clients: Mutex<HashMap<SocketAddr, Client>>,
    tx: broadcast::Sender<String>,
}

impl AppState {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            clients: Mutex::new(HashMap::new()),
            tx,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await?;
    
    println!("🚀 WebSocket Server running on ws://{}", addr);
    println!("   Waiting for connections...\n");

    let state = Arc::new(AppState::new());

    while let Ok((stream, addr)) = listener.accept().await {
        let state = Arc::clone(&state);
        tokio::spawn(handle_connection(stream, addr, state));
    }

    Ok(())
}

async fn handle_connection(stream: TcpStream, addr: SocketAddr, state: Arc<AppState>) {
    println!("📥 New connection from: {}", addr);

    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("❌ WebSocket handshake failed for {}: {}", addr, e);
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut rx = state.tx.subscribe();

    // Default username based on address
    let default_username = format!("User_{}", addr.port());
    
    // Register client
    {
        let mut clients = state.clients.lock().await;
        clients.insert(addr, Client {
            addr,
            username: default_username.clone(),
        });
    }

    // Broadcast join message
    let join_msg = ChatMessage {
        username: "System".to_string(),
        content: format!("{} has joined the chat!", default_username),
        timestamp: get_timestamp(),
        message_type: MessageType::Join,
    };
    let _ = state.tx.send(serde_json::to_string(&join_msg).unwrap());

    // Send welcome message to the new client
    let welcome = ChatMessage {
        username: "System".to_string(),
        content: format!("Welcome to the chat, {}! Type your messages to chat.", default_username),
        timestamp: get_timestamp(),
        message_type: MessageType::System,
    };
    let _ = ws_sender.send(Message::Text(serde_json::to_string(&welcome).unwrap())).await;

    // Task to forward broadcast messages to this client
    let forward_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from this client
    let state_clone = Arc::clone(&state);
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                println!("📨 Message from {}: {}", addr, text);
                
                // Get current username
                let username = {
                    let clients = state_clone.clients.lock().await;
                    clients.get(&addr)
                        .map(|c| c.username.clone())
                        .unwrap_or_else(|| default_username.clone())
                };

                // Check if it's a command
                if text.starts_with("/name ") {
                    let new_name = text.trim_start_matches("/name ").trim().to_string();
                    if !new_name.is_empty() {
                        let old_name = {
                            let mut clients = state_clone.clients.lock().await;
                            if let Some(client) = clients.get_mut(&addr) {
                                let old = client.username.clone();
                                client.username = new_name.clone();
                                old
                            } else {
                                continue;
                            }
                        };
                        
                        let rename_msg = ChatMessage {
                            username: "System".to_string(),
                            content: format!("{} is now known as {}", old_name, new_name),
                            timestamp: get_timestamp(),
                            message_type: MessageType::System,
                        };
                        let _ = state_clone.tx.send(serde_json::to_string(&rename_msg).unwrap());
                    }
                } else {
                    // Regular chat message
                    let chat_msg = ChatMessage {
                        username,
                        content: text,
                        timestamp: get_timestamp(),
                        message_type: MessageType::Chat,
                    };
                    let _ = state_clone.tx.send(serde_json::to_string(&chat_msg).unwrap());
                }
            }
            Ok(Message::Close(_)) => {
                println!("👋 Client {} requested close", addr);
                break;
            }
            Ok(Message::Ping(data)) => {
                println!("🏓 Ping from {}", addr);
                // Pong is handled automatically by tungstenite
                let _ = data; // acknowledge the ping data
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("❌ Error receiving message from {}: {}", addr, e);
                break;
            }
        }
    }

    // Client disconnected - cleanup
    let username = {
        let mut clients = state.clients.lock().await;
        clients.remove(&addr).map(|c| c.username)
    };

    if let Some(name) = username {
        let leave_msg = ChatMessage {
            username: "System".to_string(),
            content: format!("{} has left the chat.", name),
            timestamp: get_timestamp(),
            message_type: MessageType::Leave,
        };
        let _ = state.tx.send(serde_json::to_string(&leave_msg).unwrap());
    }

    forward_task.abort();
    println!("🔌 Connection closed: {}", addr);
}

fn get_timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}
