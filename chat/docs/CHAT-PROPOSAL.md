# Chat WebSocket - Chuyển đổi từ Java Tomcat sang Rust

## Mục lục

1. [Tổng quan](#1-tổng-quan)
2. [Phân tích Java Implementation](#2-phân-tích-java-implementation)
3. [Giao thức tin nhắn](#3-giao-thức-tin-nhắn)
4. [Kiến trúc Rust đề xuất](#4-kiến-trúc-rust-đề-xuất)
5. [Implementation Rust](#5-implementation-rust)
6. [Client HTML/CSS/JS thuần](#6-client-htmlcssjs-thuần)
7. [Testing Strategy](#7-testing-strategy)
8. [Deployment](#8-deployment)

---

## 1. Tổng quan

### 1.1 Mô tả ứng dụng

Chat là ứng dụng chat room đơn giản, cho phép nhiều người dùng gửi tin nhắn real-time qua WebSocket. Mỗi người dùng được gán nickname tự động (Guest0, Guest1, ...).

### 1.2 Tính năng chính

- **Real-time messaging**: Tin nhắn được gửi và nhận ngay lập tức
- **Auto nickname**: Tự động gán tên Guest + số tăng dần
- **Broadcast**: Tin nhắn được gửi đến tất cả người dùng
- **Join/Leave notifications**: Thông báo khi có người vào/rời phòng
- **HTML filtering**: Lọc HTML để tránh XSS attacks
- **Message backlog**: Queue tin nhắn khi đang gửi message khác

### 1.3 Công nghệ chuyển đổi

| Component | Java Tomcat | Rust |
|-----------|-------------|------|
| WebSocket Server | `@ServerEndpoint` annotation | `axum` + WebSocket |
| Async Runtime | Thread Pool | `tokio` |
| Connection Set | `CopyOnWriteArraySet` | `Arc<RwLock<HashMap>>` |
| Message Queue | `ArrayDeque` | `mpsc::unbounded_channel` |
| HTML Filter | Custom `HTMLFilter` | `ammonia` crate |
| Atomic Counter | `AtomicInteger` | `AtomicU64` |

---

## 2. Phân tích Java Implementation

### 2.1 Cấu trúc file gốc

```
websocket/chat/
├── ChatAnnotation.java      # WebSocket endpoint với annotations
└── (uses util.HTMLFilter)   # HTML sanitization utility
```

### 2.2 WebSocket Endpoint (ChatAnnotation.java)

```java
@ServerEndpoint(value = "/websocket/chat")
public class ChatAnnotation {
    
    // Shared state across all connections (STATIC)
    private static final String GUEST_PREFIX = "Guest";
    private static final AtomicInteger connectionIds = new AtomicInteger(0);
    private static final Set<ChatAnnotation> connections = new CopyOnWriteArraySet<>();

    // Per-connection state (INSTANCE)
    private final String nickname;
    private Session session;
    private Queue<String> messageBacklog = new ArrayDeque<>();
    private boolean messageInProgress = false;

    public ChatAnnotation() {
        // Assign unique nickname on creation
        nickname = GUEST_PREFIX + connectionIds.getAndIncrement();
    }

    @OnOpen
    public void start(Session session) {
        this.session = session;
        connections.add(this);
        broadcast("* " + nickname + " has joined.");
    }

    @OnClose
    public void end() {
        connections.remove(this);
        broadcast("* " + nickname + " has disconnected.");
    }

    @OnMessage
    public void incoming(String message) {
        // Filter HTML to prevent XSS
        String filteredMessage = nickname + ": " + HTMLFilter.filter(message);
        broadcast(filteredMessage);
    }

    @OnError
    public void onError(Throwable t) {
        log.error("Chat Error: " + t.toString(), t);
    }
}
```

### 2.3 Message Queue Pattern

Java sử dụng message backlog để tránh blocking khi gửi nhiều tin nhắn:

```java
private void sendMessage(String msg) throws IOException {
    synchronized (this) {
        if (messageInProgress) {
            // Queue message if another send is in progress
            messageBacklog.add(msg);
            return;
        } else {
            messageInProgress = true;
        }
    }

    // Send messages until queue is empty
    boolean queueHasMessages = true;
    String messageToSend = msg;
    
    do {
        session.getBasicRemote().sendText(messageToSend);
        
        synchronized (this) {
            messageToSend = messageBacklog.poll();
            if (messageToSend == null) {
                messageInProgress = false;
                queueHasMessages = false;
            }
        }
    } while (queueHasMessages);
}
```

### 2.4 Broadcast Pattern

```java
private static void broadcast(String msg) {
    for (ChatAnnotation client : connections) {
        try {
            client.sendMessage(msg);
        } catch (IOException ioe) {
            // Remove failed client and notify others
            if (connections.remove(client)) {
                client.session.close();
                broadcast("* " + client.nickname + " has been disconnected.");
            }
        }
    }
}
```

### 2.5 Key Observations

| Aspect | Java Implementation | Rust Equivalent |
|--------|---------------------|-----------------|
| Connection tracking | Static `CopyOnWriteArraySet` | `Arc<RwLock<HashMap<Uuid, Client>>>` |
| Nickname generation | `AtomicInteger` counter | `AtomicU64` counter |
| Thread safety | `synchronized` blocks | Async channels (no blocking) |
| Message ordering | Queue per connection | `mpsc::unbounded_channel` |
| Error handling | Remove client + broadcast disconnect | Same pattern |
| XSS prevention | `HTMLFilter.filter()` | `ammonia::clean()` |

---

## 3. Giao thức tin nhắn

### 3.1 Message Format

Không có protocol phức tạp - chỉ là plain text:

| Direction | Format | Example |
|-----------|--------|---------|
| Client → Server | `<message>` | `Hello everyone!` |
| Server → Client (chat) | `<nickname>: <message>` | `Guest0: Hello everyone!` |
| Server → Client (join) | `* <nickname> has joined.` | `* Guest1 has joined.` |
| Server → Client (leave) | `* <nickname> has disconnected.` | `* Guest0 has disconnected.` |

### 3.2 Sequence Diagram

```
Client A                    Server                    Client B
    |                          |                          |
    |------ Connect ---------->|                          |
    |                          |-- "* Guest0 has joined." |
    |<-- "* Guest0 has joined."|                          |
    |                          |                          |
    |                          |<-------- Connect --------|
    |<- "* Guest1 has joined." |-- "* Guest1 has joined." |
    |                          |                          |
    |--- "Hello!" ------------>|                          |
    |<-- "Guest0: Hello!" -----|--- "Guest0: Hello!" ---->|
    |                          |                          |
    |                          |<------- "Hi!" -----------|
    |<---- "Guest1: Hi!" ------|------ "Guest1: Hi!" ---->|
    |                          |                          |
    |                          |<------- Disconnect ------|
    |<- "* Guest1 disconnected"|                          |
```

---

## 4. Kiến trúc Rust đề xuất

### 4.1 Cấu trúc project

```
chat/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point + server setup
│   ├── lib.rs               # Library exports
│   ├── chat/
│   │   ├── mod.rs
│   │   ├── handler.rs       # WebSocket connection handler
│   │   └── state.rs         # Shared chat state
│   └── error.rs             # Error types
├── static/
│   ├── index.html
│   ├── style.css
│   └── app.js
└── docs/
    └── CHAT-PROPOSAL.md     # This file
```

### 4.2 Dependencies (Cargo.toml)

```toml
[package]
name = "chat-rs"
version = "0.1.0"
edition = "2021"
description = "A simple WebSocket chat application"

[dependencies]
# Web framework
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.5", features = ["fs"] }

# WebSocket
futures-util = "0.3"

# Utilities
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4"] }

# HTML sanitization (thay thế HTMLFilter)
ammonia = "4"

[dev-dependencies]
tokio-test = "0.4"
tokio-tungstenite = "0.21"
```

### 4.3 Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                      Axum Server                         │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐                     │
│  │   Router    │───>│  WS Handler │                     │
│  └─────────────┘    └─────────────┘                     │
│         │                  │                            │
│         v                  v                            │
│  ┌─────────────┐    ┌───────────────────────────────┐  │
│  │Static Files│    │  ChatState (Arc<RwLock>)       │  │
│  └─────────────┘    │  ┌─────────────────────────┐  │  │
│                     │  │ clients: HashMap<       │  │  │
│                     │  │   Uuid,                 │  │  │
│                     │  │   mpsc::UnboundedSender │  │  │
│                     │  │ >                       │  │  │
│                     │  ├─────────────────────────┤  │  │
│                     │  │ next_id: AtomicU64      │  │  │
│                     │  └─────────────────────────┘  │  │
│                     └───────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 4.4 Data Flow

```
1. Client connects via WebSocket
   └─> ws_handler() receives upgrade request
       └─> handle_socket() spawns for connection
           ├─> Generate nickname (Guest0, Guest1, ...)
           ├─> Create mpsc channel for outgoing messages
           ├─> Add client to ChatState.clients
           └─> Broadcast "has joined"

2. Client sends message
   └─> handle_socket() receives Message::Text
       └─> Sanitize with ammonia::clean()
           └─> Broadcast to all clients in ChatState

3. Client disconnects
   └─> handle_socket() receives close/error
       └─> Remove client from ChatState
           └─> Broadcast "has disconnected"
```

---

## 5. Implementation Rust

### 5.1 Main Entry Point (src/main.rs)

```rust
use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod chat;
mod error;

use chat::ChatState;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "chat=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create shared chat state
    let state = ChatState::new();

    // Build router
    let app = Router::new()
        .route("/ws/chat", get(chat::handler::ws_handler))
        .nest_service("/", ServeDir::new("static"))
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("💬 Chat server running on http://localhost:8080");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### 5.2 Chat State (src/chat/state.rs)

```rust
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

    pub fn send(&self, message: &str) -> bool {
        self.sender.send(Message::Text(message.to_string())).is_ok()
    }
}

/// Shared state for the chat application
#[derive(Clone)]
pub struct ChatState {
    clients: Arc<RwLock<HashMap<Uuid, Client>>>,
    next_guest_id: Arc<AtomicU64>,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            next_guest_id: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Generate a unique nickname
    pub fn generate_nickname(&self) -> String {
        let id = self.next_guest_id.fetch_add(1, Ordering::SeqCst);
        format!("Guest{}", id)
    }

    /// Add a new client
    pub async fn add_client(&self, id: Uuid, client: Client) {
        let mut clients = self.clients.write().await;
        clients.insert(id, client);
    }

    /// Remove a client
    pub async fn remove_client(&self, id: &Uuid) -> Option<Client> {
        let mut clients = self.clients.write().await;
        clients.remove(id)
    }

    /// Get number of connected clients
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

        // Remove failed clients
        for id in failed_ids {
            if let Some(client) = self.remove_client(&id).await {
                tracing::info!("Removed disconnected client: {}", client.nickname);
                let msg = format!("* {} has been disconnected.", client.nickname);
                Box::pin(self.broadcast(&msg)).await;
            }
        }
    }
}
```

### 5.3 WebSocket Handler (src/chat/handler.rs)

```rust
use axum::{
    extract::{ws::{Message, WebSocket}, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::state::{ChatState, Client};

/// Sanitize HTML to prevent XSS attacks
fn sanitize_html(input: &str) -> String {
    ammonia::clean(input)
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<ChatState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, state: ChatState) {
    let (mut sender, mut receiver) = socket.split();

    // Create channel for outgoing messages
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Generate unique ID and nickname
    let client_id = Uuid::new_v4();
    let nickname = state.generate_nickname();

    tracing::info!("{} connected", nickname);

    // Add client to state
    let client = Client::new(nickname.clone(), tx);
    state.add_client(client_id, client).await;

    // Broadcast join message
    let join_msg = format!("* {} has joined.", nickname);
    state.broadcast(&join_msg).await;

    // Spawn task for sending outgoing messages
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                let sanitized = sanitize_html(&text);
                if !sanitized.is_empty() {
                    let chat_msg = format!("{}: {}", nickname, sanitized);
                    state.broadcast(&chat_msg).await;
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {} // Ignore binary, ping, pong
            Err(e) => {
                tracing::warn!("WebSocket error for {}: {}", nickname, e);
                break;
            }
        }
    }

    // Cleanup
    if state.remove_client(&client_id).await.is_some() {
        let disconnect_msg = format!("* {} has disconnected.", nickname);
        state.broadcast(&disconnect_msg).await;
    }

    send_task.abort();
    tracing::info!("{} disconnected", nickname);
}
```

### 5.4 Module Exports

```rust
// src/chat/mod.rs
mod state;
pub mod handler;

pub use state::{ChatState, Client};
```

```rust
// src/lib.rs
pub mod chat;
pub mod error;
```

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChatError {
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
}
```

---

## 6. Client HTML/CSS/JS thuần

### 6.1 HTML (static/index.html)

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Chat - Rust WebSocket</title>
    <link rel="stylesheet" href="style.css">
</head>
<body>
    <div class="container">
        <h1>💬 WebSocket Chat</h1>
        
        <div id="status-bar">
            <span id="connection-status" class="status disconnected">Disconnected</span>
        </div>
        
        <div id="chat-container">
            <div id="messages"></div>
        </div>
        
        <div id="input-container">
            <input type="text" id="message-input" 
                   placeholder="Type a message and press Enter..."
                   autocomplete="off" disabled>
            <button id="send-button" disabled>Send</button>
        </div>
    </div>

    <script src="app.js"></script>
</body>
</html>
```

### 6.2 CSS (static/style.css)

```css
* {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #f0f2f5;
    min-height: 100vh;
    display: flex;
    justify-content: center;
    align-items: center;
    padding: 20px;
}

.container {
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
    width: 100%;
    max-width: 500px;
    overflow: hidden;
}

h1 {
    background: #4a90d9;
    color: white;
    padding: 20px;
    text-align: center;
}

#status-bar {
    padding: 10px 20px;
    background: #f8f9fa;
    border-bottom: 1px solid #e0e0e0;
}

.status {
    padding: 4px 12px;
    border-radius: 12px;
    font-size: 0.9em;
}

.status.connected { background: #d4edda; color: #155724; }
.status.disconnected { background: #f8d7da; color: #721c24; }

#chat-container {
    height: 400px;
    overflow-y: auto;
    padding: 15px;
}

#messages {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.message {
    padding: 8px 12px;
    border-radius: 8px;
    max-width: 85%;
}

.message.system {
    background: #e9ecef;
    color: #495057;
    font-style: italic;
    align-self: center;
}

.message.chat {
    background: #e3f2fd;
}

.message.own {
    background: #4a90d9;
    color: white;
    align-self: flex-end;
}

#input-container {
    display: flex;
    padding: 15px;
    gap: 10px;
    background: #f8f9fa;
    border-top: 1px solid #e0e0e0;
}

#message-input {
    flex: 1;
    padding: 12px 15px;
    border: 1px solid #ddd;
    border-radius: 24px;
    outline: none;
}

#send-button {
    padding: 12px 24px;
    background: #4a90d9;
    color: white;
    border: none;
    border-radius: 24px;
    cursor: pointer;
}

#send-button:disabled { background: #ccc; }
```

### 6.3 JavaScript (static/app.js)

```javascript
"use strict";

(function() {
    let socket = null;
    let myNickname = null;

    const messages = document.getElementById('messages');
    const chatContainer = document.getElementById('chat-container');
    const messageInput = document.getElementById('message-input');
    const sendButton = document.getElementById('send-button');
    const status = document.getElementById('connection-status');

    document.addEventListener('DOMContentLoaded', connect);

    function connect() {
        const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const url = `${protocol}//${location.host}/ws/chat`;

        socket = new WebSocket(url);

        socket.onopen = () => {
            status.textContent = 'Connected';
            status.className = 'status connected';
            messageInput.disabled = false;
            sendButton.disabled = false;
            messageInput.focus();
        };

        socket.onclose = () => {
            status.textContent = 'Disconnected';
            status.className = 'status disconnected';
            messageInput.disabled = true;
            sendButton.disabled = true;
            setTimeout(connect, 3000); // Reconnect
        };

        socket.onmessage = (event) => handleMessage(event.data);
    }

    function handleMessage(data) {
        const div = document.createElement('div');
        
        if (data.startsWith('* ')) {
            div.className = 'message system';
            div.textContent = data.substring(2);
        } else {
            const colonIdx = data.indexOf(': ');
            if (colonIdx !== -1) {
                const nick = data.substring(0, colonIdx);
                const isOwn = nick === myNickname;
                div.className = `message chat${isOwn ? ' own' : ''}`;
                div.innerHTML = `<strong>${nick}:</strong> ${data.substring(colonIdx + 2)}`;
                
                // Detect own nickname
                if (!myNickname && lastSent && data.includes(lastSent)) {
                    myNickname = nick;
                    lastSent = null;
                }
            } else {
                div.className = 'message system';
                div.textContent = data;
            }
        }
        
        messages.appendChild(div);
        chatContainer.scrollTop = chatContainer.scrollHeight;
        
        // Limit history
        while (messages.children.length > 100) {
            messages.removeChild(messages.firstChild);
        }
    }

    let lastSent = null;

    function sendMessage() {
        const text = messageInput.value.trim();
        if (text && socket?.readyState === WebSocket.OPEN) {
            lastSent = text;
            socket.send(text);
            messageInput.value = '';
        }
    }

    messageInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') sendMessage();
    });
    
    sendButton.addEventListener('click', sendMessage);
})();
```

---

## 7. Testing Strategy

### 7.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_nickname_generation() {
        let state = ChatState::new();
        assert_eq!(state.generate_nickname(), "Guest0");
        assert_eq!(state.generate_nickname(), "Guest1");
    }

    #[tokio::test]
    async fn test_client_lifecycle() {
        let state = ChatState::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = Client::new("Guest0".to_string(), tx);
        let id = Uuid::new_v4();

        state.add_client(id, client).await;
        assert_eq!(state.client_count().await, 1);

        state.remove_client(&id).await;
        assert_eq!(state.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast() {
        let state = ChatState::new();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        
        state.add_client(Uuid::new_v4(), Client::new("A".into(), tx1)).await;
        state.add_client(Uuid::new_v4(), Client::new("B".into(), tx2)).await;
        
        state.broadcast("Hello").await;
        
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn test_html_sanitization() {
        let input = "<script>alert('xss')</script>";
        let output = ammonia::clean(input);
        assert!(!output.contains("<script>"));
    }
}
```

### 7.2 Integration Tests

```rust
#[tokio::test]
async fn test_websocket_chat() {
    // Start server
    let state = ChatState::new();
    let app = Router::new()
        .route("/ws/chat", get(handler::ws_handler))
        .with_state(state);
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, app));
    
    // Connect clients and test messaging
    let (mut ws1, _) = connect_async(format!("ws://{}/ws/chat", addr)).await.unwrap();
    let (mut ws2, _) = connect_async(format!("ws://{}/ws/chat", addr)).await.unwrap();
    
    // Verify join messages and chat broadcast
    // ...
}
```

### 7.3 Test Commands

```bash
cargo test                    # All tests
cargo test state_test         # Specific module
cargo test -- --nocapture     # With output
```

---

## 8. Deployment

### 8.1 Build & Run

```bash
# Development
cargo run

# Release
cargo build --release
./target/release/chat-rs
```

### 8.2 Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/chat-rs /usr/local/bin/
COPY --from=builder /app/static /app/static
WORKDIR /app
EXPOSE 8080
CMD ["chat-rs"]
```

---

## 9. So sánh Java vs Rust

| Aspect | Java (Tomcat) | Rust (Axum) |
|--------|---------------|-------------|
| Concurrency | `synchronized` blocks | `async/await` + channels |
| Connection Set | `CopyOnWriteArraySet` | `Arc<RwLock<HashMap>>` |
| Message Queue | `ArrayDeque` per client | `mpsc::unbounded_channel` |
| HTML Filter | Custom `HTMLFilter` | `ammonia` crate |
| Memory | GC managed | Zero-cost abstractions |
| Lines of Code | ~120 | ~150 |

---

## 10. Implementation Checklist

- [ ] Create `Cargo.toml` with dependencies
- [ ] Implement `src/main.rs` - server setup
- [ ] Implement `src/chat/state.rs` - ChatState + Client
- [ ] Implement `src/chat/handler.rs` - WebSocket handler
- [ ] Implement `src/chat/mod.rs` - module exports
- [ ] Implement `src/lib.rs` - library exports
- [ ] Create `static/index.html` - UI
- [ ] Create `static/style.css` - styling
- [ ] Create `static/app.js` - client logic
- [ ] Write unit tests
- [ ] Write integration tests
- [ ] Test manually
- [ ] Create Dockerfile

---

*Tài liệu này được tạo để hướng dẫn chuyển đổi ứng dụng Chat WebSocket từ Java Tomcat sang Rust.*
