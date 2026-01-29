# Drawboard WebSocket - Chuyển đổi từ Java Tomcat sang Rust

## Mục lục

1. [Tổng quan](#1-tổng-quan)
2. [Kiến trúc hiện tại (Java Tomcat)](#2-kiến-trúc-hiện-tại-java-tomcat)
3. [Giao thức tin nhắn WebSocket](#3-giao-thức-tin-nhắn-websocket)
4. [Kiến trúc Rust đề xuất](#4-kiến-trúc-rust-đề-xuất)
5. [Implementation Rust](#5-implementation-rust)
6. [Client HTML/CSS/JS thuần](#6-client-htmlcssjs-thuần)
7. [Testing Strategy](#7-testing-strategy)
8. [Deployment](#8-deployment)

---

## 1. Tổng quan

### 1.1 Mô tả ứng dụng

Drawboard là ứng dụng vẽ collaborative real-time, cho phép nhiều người dùng cùng vẽ trên một canvas chung. Mọi thay đổi được đồng bộ ngay lập tức qua WebSocket.

### 1.2 Tính năng chính

- **Real-time drawing**: Vẽ brush, line, rectangle, ellipse
- **Multi-user**: Hỗ trợ tối đa 100 người dùng đồng thời
- **State sync**: Người dùng mới nhận được trạng thái canvas hiện tại (PNG)
- **Message buffering**: Gộp tin nhắn vẽ để tối ưu băng thông (30ms interval)

### 1.3 Công nghệ chuyển đổi

| Component | Java Tomcat | Rust |
|-----------|-------------|------|
| WebSocket Server | Jakarta WebSocket API | `axum` + `tokio-tungstenite` |
| Async Runtime | Thread Pool | `tokio` |
| Synchronization | `ReentrantLock` | `tokio::sync::RwLock` |
| Image Processing | `java.awt.BufferedImage` | `image` + `tiny-skia` |
| Timer | `java.util.Timer` | `tokio::time::interval` |

---

## 2. Kiến trúc hiện tại (Java Tomcat)

### 2.1 Cấu trúc file

```
websocket/drawboard/
├── DrawboardEndpoint.java      # WebSocket endpoint handler
├── DrawboardContextListener.java # Servlet context lifecycle
├── Room.java                   # Room logic + Player inner class
├── Client.java                 # Async message sender
├── DrawMessage.java            # Drawing message model
└── wsmessages/
    ├── AbstractWebsocketMessage.java
    ├── StringWebsocketMessage.java
    ├── BinaryWebsocketMessage.java
    └── CloseWebsocketMessage.java
```

### 2.2 Lifecycle WebSocket Endpoint

```java
public final class DrawboardEndpoint extends Endpoint {
    private static volatile Room room = null;
    private Room.Player player;

    @Override
    public void onOpen(Session session, EndpointConfig config) {
        session.setMaxTextMessageBufferSize(10000);
        session.addMessageHandler(stringHandler);
        
        Client client = new Client(session);
        Room room = getRoom(true);
        
        room.invokeAndWait(() -> {
            player = room.createAndAddPlayer(client);
        });
    }

    @Override
    public void onClose(Session session, CloseReason closeReason) {
        if (room != null && player != null) {
            room.invokeAndWait(() -> {
                player.removeFromRoom();
            });
        }
    }

    @Override
    public void onMessage(String message) {
        // Parse và xử lý draw messages
    }
}
```

### 2.3 Synchronization Pattern

Java sử dụng `ReentrantLock` với pattern `invokeAndWait`:

```java
public void invokeAndWait(Runnable task) {
    roomLock.lock();
    try {
        if (!closed) {
            task.run();
        }
    } finally {
        roomLock.unlock();
    }
}
```

### 2.4 Message Buffering

Draw messages được buffer và gửi theo batch mỗi 30ms:

```java
private static final int TIMER_DELAY = 30; // ms

private void broadcastTimerTick() {
    for (Player p : players) {
        List<DrawMessage> messages = p.getBufferedDrawMessages();
        if (!messages.isEmpty()) {
            // Gộp messages và gửi
            String batched = messages.stream()
                .map(m -> lastMsgId + "," + m.toString())
                .collect(Collectors.joining("|"));
            p.sendRoomMessage(MessageType.DRAW_MESSAGE, batched);
        }
    }
}
```

---

## 3. Giao thức tin nhắn WebSocket

### 3.1 Message Types (Server → Client)

| Type | Prefix | Mô tả | Format |
|------|--------|-------|--------|
| ERROR | `'0'` | Thông báo lỗi | `0<error_message>` |
| DRAW_MESSAGE | `'1'` | Dữ liệu vẽ | `1<lastMsgId>,<drawData>` hoặc `1<msg1>\|<msg2>\|...` |
| IMAGE_MESSAGE | `'2'` | Trạng thái ban đầu | `2<player_count>` + Binary PNG |
| PLAYER_CHANGED | `'3'` | Thay đổi số người | `3+` (join) hoặc `3-` (leave) |

### 3.2 Message Types (Client → Server)

| Type | Prefix | Mô tả | Format |
|------|--------|-------|--------|
| Pong | `'0'` | Keepalive | `0` |
| Draw | `'1'` | Vẽ | `1<msgId>\|<type>,<R>,<G>,<B>,<A>,<thickness>,<x1>,<y1>,<x2>,<y2>` |

### 3.3 DrawMessage Format

```
type,colorR,colorG,colorB,colorA,thickness,x1,y1,x2,y2
```

**Fields:**
- `type`: 1=Brush, 2=Line, 3=Rectangle, 4=Ellipse
- `colorR,G,B,A`: 0-255
- `thickness`: 0-100
- `x1,y1,x2,y2`: Tọa độ (double)

**Ví dụ:** `1,255,0,0,255,5.0,100.5,200.5,150.5,250.5`

### 3.4 Sequence Diagram

```
Client A                    Server                    Client B
    |                          |                          |
    |------ Connect ---------->|                          |
    |<----- "2<count>" --------|                          |
    |<----- PNG Binary --------|                          |
    |                          |                          |
    |--- "1<id>|<draw>" ------>|                          |
    |                          |------ "3+" ------------->|
    |                          |                          |
    |                          |  (buffer 30ms)           |
    |                          |                          |
    |<---- "1<id>,<draw>" -----|------ "1<id>,<draw>" --->|
    |                          |                          |
```

---

## 4. Kiến trúc Rust đề xuất

### 4.1 Cấu trúc project

```
drawboard-rs/
├── Cargo.toml
├── src/
│   ├── main.rs                 # Entry point + server setup
│   ├── lib.rs                  # Library exports
│   ├── websocket/
│   │   ├── mod.rs
│   │   ├── handler.rs          # WebSocket connection handler
│   │   └── message.rs          # Message types
│   ├── room/
│   │   ├── mod.rs
│   │   ├── room.rs             # Room state management
│   │   ├── player.rs           # Player struct
│   │   └── broadcaster.rs      # Message broadcasting
│   ├── drawing/
│   │   ├── mod.rs
│   │   ├── draw_message.rs     # DrawMessage struct
│   │   └── canvas.rs           # Server-side canvas
│   └── error.rs                # Error types
├── static/
│   ├── index.html
│   ├── style.css
│   └── app.js
└── tests/
    ├── integration/
    │   └── websocket_test.rs
    └── unit/
        ├── draw_message_test.rs
        └── room_test.rs
```

### 4.2 Dependencies (Cargo.toml)

```toml
[package]
name = "drawboard-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
# Web framework
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors"] }

# WebSocket
tokio-tungstenite = "0.21"
futures-util = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Image processing
image = "0.25"
tiny-skia = "0.11"

# Utilities
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1", features = ["v4"] }
parking_lot = "0.12"

[dev-dependencies]
tokio-test = "0.4"
```

### 4.3 Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Axum Server                               │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │
│  │   Router    │───>│  WS Handler │───>│  Room (Arc<RwLock>) │  │
│  └─────────────┘    └─────────────┘    └─────────────────────┘  │
│         │                  │                      │              │
│         │                  │           ┌──────────┴──────────┐  │
│         v                  v           v                     v  │
│  ┌─────────────┐    ┌───────────┐  ┌────────┐         ┌────────┐│
│  │Static Files│    │  Players  │  │ Canvas │         │Broadcast││
│  └─────────────┘    │(HashMap)  │  │(tiny-  │         │ Timer  ││
│                     └───────────┘  │ skia)  │         │ (30ms) ││
│                           │        └────────┘         └────────┘│
│                           v                                     │
│                    ┌────────────┐                               │
│                    │ mpsc::Sender│ (per player)                 │
│                    └────────────┘                               │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. Implementation Rust

### 5.1 Main Entry Point

```rust
// src/main.rs
use axum::{
    Router,
    routing::get,
    extract::WebSocketUpgrade,
    response::IntoResponse,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tracing_subscriber;

mod websocket;
mod room;
mod drawing;
mod error;

use room::Room;

#[derive(Clone)]
pub struct AppState {
    pub room: Arc<RwLock<Room>>,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::init();

    // Create shared room state
    let room = Arc::new(RwLock::new(Room::new(900, 600)));
    let state = AppState { room: room.clone() };

    // Start broadcast timer
    let broadcast_room = room.clone();
    tokio::spawn(async move {
        room::broadcaster::start_broadcast_timer(broadcast_room).await;
    });

    // Build router
    let app = Router::new()
        .route("/ws/drawboard", get(websocket::handler::ws_handler))
        .nest_service("/", ServeDir::new("static"))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("Server running on http://localhost:8080");
    axum::serve(listener, app).await.unwrap();
}
```

### 5.2 WebSocket Handler

```rust
// src/websocket/handler.rs
use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{AppState, room::Player, drawing::DrawMessage};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    
    // Create channel for outgoing messages
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    
    // Generate player ID
    let player_id = Uuid::new_v4();
    
    // Add player to room
    let (player_count, image_data) = {
        let mut room = state.room.write().await;
        
        if room.player_count() >= 100 {
            let _ = sender.send(Message::Text("0Maximum player count reached".into())).await;
            let _ = sender.close().await;
            return;
        }
        
        // Notify existing players
        room.broadcast_message("3+").await;
        
        // Add new player
        room.add_player(player_id, Player::new(tx.clone()));
        
        // Get current state
        let count = room.player_count();
        let image = room.get_canvas_png();
        
        (count, image)
    };
    
    // Send initial state
    let _ = sender.send(Message::Text(format!("2{}", player_count))).await;
    let _ = sender.send(Message::Binary(image_data)).await;
    
    // Spawn task for sending outgoing messages
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                handle_text_message(&state, player_id, &text).await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    
    // Cleanup: remove player
    {
        let mut room = state.room.write().await;
        room.remove_player(&player_id);
        room.broadcast_message("3-").await;
    }
    
    send_task.abort();
}

async fn handle_text_message(state: &AppState, player_id: Uuid, text: &str) {
    if text.is_empty() {
        return;
    }
    
    let msg_type = &text[0..1];
    let content = &text[1..];
    
    match msg_type {
        "0" => {
            // Pong - keepalive, no action needed
        }
        "1" => {
            // Draw message: "1<msgId>|<drawData>"
            if let Some((msg_id_str, draw_data)) = content.split_once('|') {
                if let Ok(msg_id) = msg_id_str.parse::<u64>() {
                    if let Ok(draw_msg) = DrawMessage::parse(draw_data) {
                        let mut room = state.room.write().await;
                        room.handle_draw_message(player_id, draw_msg, msg_id);
                    }
                }
            }
        }
        _ => {}
    }
}
```

### 5.3 WebSocket Message Types

```rust
// src/websocket/message.rs
use axum::extract::ws::Message;

#[derive(Debug, Clone)]
pub enum ServerMessage {
    Error(String),
    DrawMessage(String),
    ImageMessage(u32),
    PlayerChanged(bool), // true = joined, false = left
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

#[derive(Debug, Clone)]
pub enum ClientMessage {
    Pong,
    Draw { msg_id: u64, data: String },
}

impl ClientMessage {
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        
        let msg_type = &text[0..1];
        let content = &text[1..];
        
        match msg_type {
            "0" => Some(ClientMessage::Pong),
            "1" => {
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
```

### 5.4 DrawMessage

```rust
// src/drawing/draw_message.rs
use crate::error::DrawboardError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawType {
    Brush = 1,
    Line = 2,
    Rectangle = 3,
    Ellipse = 4,
}

impl TryFrom<i32> for DrawType {
    type Error = DrawboardError;
    
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(DrawType::Brush),
            2 => Ok(DrawType::Line),
            3 => Ok(DrawType::Rectangle),
            4 => Ok(DrawType::Ellipse),
            _ => Err(DrawboardError::InvalidDrawType(value)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrawMessage {
    pub draw_type: DrawType,
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
    pub color_a: u8,
    pub thickness: f64,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl DrawMessage {
    pub fn new(
        draw_type: DrawType,
        color: (u8, u8, u8, u8),
        thickness: f64,
        start: (f64, f64),
        end: (f64, f64),
    ) -> Self {
        Self {
            draw_type,
            color_r: color.0,
            color_g: color.1,
            color_b: color.2,
            color_a: color.3,
            thickness,
            x1: start.0,
            y1: start.1,
            x2: end.0,
            y2: end.1,
        }
    }
    
    /// Parse from string format: "type,R,G,B,A,thickness,x1,y1,x2,y2"
    pub fn parse(s: &str) -> Result<Self, DrawboardError> {
        let parts: Vec<&str> = s.split(',').collect();
        
        if parts.len() != 10 {
            return Err(DrawboardError::ParseError(
                format!("Expected 10 elements, got {}", parts.len())
            ));
        }
        
        let draw_type: DrawType = parts[0]
            .parse::<i32>()
            .map_err(|_| DrawboardError::ParseError("Invalid type".into()))?
            .try_into()?;
        
        let color_r = parts[1].parse::<u8>()
            .map_err(|_| DrawboardError::ParseError("Invalid colorR".into()))?;
        let color_g = parts[2].parse::<u8>()
            .map_err(|_| DrawboardError::ParseError("Invalid colorG".into()))?;
        let color_b = parts[3].parse::<u8>()
            .map_err(|_| DrawboardError::ParseError("Invalid colorB".into()))?;
        let color_a = parts[4].parse::<u8>()
            .map_err(|_| DrawboardError::ParseError("Invalid colorA".into()))?;
        
        let thickness = parts[5].parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid thickness".into()))?;
        
        if thickness < 0.0 || thickness > 100.0 || thickness.is_nan() {
            return Err(DrawboardError::ParseError(
                format!("Thickness out of range: {}", thickness)
            ));
        }
        
        let x1 = parts[6].parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid x1".into()))?;
        let y1 = parts[7].parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid y1".into()))?;
        let x2 = parts[8].parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid x2".into()))?;
        let y2 = parts[9].parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid y2".into()))?;
        
        // Validate coordinates
        for coord in [x1, y1, x2, y2] {
            if coord.is_nan() {
                return Err(DrawboardError::ParseError("NaN coordinate".into()));
            }
        }
        
        Ok(Self {
            draw_type,
            color_r,
            color_g,
            color_b,
            color_a,
            thickness,
            x1,
            y1,
            x2,
            y2,
        })
    }
    
    /// Serialize to string format
    pub fn to_string(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{}",
            self.draw_type as i32,
            self.color_r,
            self.color_g,
            self.color_b,
            self.color_a,
            self.thickness,
            self.x1,
            self.y1,
            self.x2,
            self.y2
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_valid_message() {
        let msg = DrawMessage::parse("1,255,0,0,255,5.0,100.5,200.5,150.5,250.5").unwrap();
        assert_eq!(msg.draw_type, DrawType::Brush);
        assert_eq!(msg.color_r, 255);
        assert_eq!(msg.color_g, 0);
        assert_eq!(msg.color_b, 0);
        assert_eq!(msg.color_a, 255);
        assert_eq!(msg.thickness, 5.0);
        assert_eq!(msg.x1, 100.5);
        assert_eq!(msg.y1, 200.5);
        assert_eq!(msg.x2, 150.5);
        assert_eq!(msg.y2, 250.5);
    }
    
    #[test]
    fn test_parse_invalid_type() {
        let result = DrawMessage::parse("5,255,0,0,255,5.0,100,200,150,250");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_roundtrip() {
        let original = DrawMessage::new(
            DrawType::Rectangle,
            (100, 150, 200, 128),
            10.0,
            (50.0, 60.0),
            (200.0, 300.0),
        );
        let serialized = original.to_string();
        let parsed = DrawMessage::parse(&serialized).unwrap();
        
        assert_eq!(parsed.draw_type, original.draw_type);
        assert_eq!(parsed.color_r, original.color_r);
        assert_eq!(parsed.thickness, original.thickness);
    }
}
```

### 5.5 Canvas (Server-side Drawing)

```rust
// src/drawing/canvas.rs
use tiny_skia::{
    Pixmap, Paint, PathBuilder, Stroke, LineCap, LineJoin,
    Transform, Color,
};
use image::{ImageBuffer, Rgba, ImageFormat};
use std::io::Cursor;

use crate::drawing::DrawMessage;
use crate::drawing::draw_message::DrawType;

pub struct Canvas {
    pixmap: Pixmap,
    width: u32,
    height: u32,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        let mut pixmap = Pixmap::new(width, height).unwrap();
        
        // Fill with white background
        pixmap.fill(Color::WHITE);
        
        Self { pixmap, width, height }
    }
    
    pub fn draw(&mut self, msg: &DrawMessage) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(
            msg.color_r,
            msg.color_g,
            msg.color_b,
            msg.color_a,
        ));
        paint.anti_alias = true;
        
        let stroke = Stroke {
            width: msg.thickness as f32,
            line_cap: LineCap::Round,
            line_join: LineJoin::Miter,
            ..Default::default()
        };
        
        match msg.draw_type {
            DrawType::Brush | DrawType::Line => {
                self.draw_line(msg, &paint, &stroke);
            }
            DrawType::Rectangle => {
                self.draw_rectangle(msg, &paint, &stroke);
            }
            DrawType::Ellipse => {
                self.draw_ellipse(msg, &paint, &stroke);
            }
        }
    }
    
    fn draw_line(&mut self, msg: &DrawMessage, paint: &Paint, stroke: &Stroke) {
        let mut pb = PathBuilder::new();
        
        if (msg.x1 - msg.x2).abs() < f64::EPSILON && (msg.y1 - msg.y2).abs() < f64::EPSILON {
            // Draw a point (small circle)
            pb.push_circle(msg.x1 as f32, msg.y1 as f32, 0.5);
        } else {
            pb.move_to(msg.x1 as f32, msg.y1 as f32);
            pb.line_to(msg.x2 as f32, msg.y2 as f32);
        }
        
        if let Some(path) = pb.finish() {
            self.pixmap.stroke_path(&path, paint, stroke, Transform::identity(), None);
        }
    }
    
    fn draw_rectangle(&mut self, msg: &DrawMessage, paint: &Paint, stroke: &Stroke) {
        let (x1, x2) = if msg.x1 > msg.x2 { (msg.x2, msg.x1) } else { (msg.x1, msg.x2) };
        let (y1, y2) = if msg.y1 > msg.y2 { (msg.y2, msg.y1) } else { (msg.y1, msg.y2) };
        
        let mut pb = PathBuilder::new();
        pb.push_rect(
            tiny_skia::Rect::from_xywh(
                x1 as f32,
                y1 as f32,
                (x2 - x1) as f32,
                (y2 - y1) as f32,
            ).unwrap_or(tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap())
        );
        
        if let Some(path) = pb.finish() {
            self.pixmap.stroke_path(&path, paint, stroke, Transform::identity(), None);
        }
    }
    
    fn draw_ellipse(&mut self, msg: &DrawMessage, paint: &Paint, stroke: &Stroke) {
        let (x1, x2) = if msg.x1 > msg.x2 { (msg.x2, msg.x1) } else { (msg.x1, msg.x2) };
        let (y1, y2) = if msg.y1 > msg.y2 { (msg.y2, msg.y1) } else { (msg.y1, msg.y2) };
        
        let cx = (x1 + x2) / 2.0;
        let cy = (y1 + y2) / 2.0;
        let rx = (x2 - x1) / 2.0;
        let ry = (y2 - y1) / 2.0;
        
        let mut pb = PathBuilder::new();
        pb.push_oval(
            tiny_skia::Rect::from_xywh(
                (cx - rx) as f32,
                (cy - ry) as f32,
                (rx * 2.0) as f32,
                (ry * 2.0) as f32,
            ).unwrap_or(tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap())
        );
        
        if let Some(path) = pb.finish() {
            self.pixmap.stroke_path(&path, paint, stroke, Transform::identity(), None);
        }
    }
    
    /// Export canvas to PNG bytes
    pub fn to_png(&self) -> Vec<u8> {
        // Convert tiny-skia Pixmap to image crate format
        let data = self.pixmap.data();
        
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = 
            ImageBuffer::new(self.width, self.height);
        
        for (i, pixel) in img.pixels_mut().enumerate() {
            let offset = i * 4;
            // tiny-skia uses RGBA premultiplied, need to unpremultiply
            let a = data[offset + 3] as f32 / 255.0;
            if a > 0.0 {
                *pixel = Rgba([
                    (data[offset] as f32 / a).min(255.0) as u8,
                    (data[offset + 1] as f32 / a).min(255.0) as u8,
                    (data[offset + 2] as f32 / a).min(255.0) as u8,
                    data[offset + 3],
                ]);
            } else {
                *pixel = Rgba([255, 255, 255, 255]); // White for transparent
            }
        }
        
        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, ImageFormat::Png).unwrap();
        buffer.into_inner()
    }
    
    pub fn width(&self) -> u32 {
        self.width
    }
    
    pub fn height(&self) -> u32 {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_new_canvas() {
        let canvas = Canvas::new(900, 600);
        assert_eq!(canvas.width(), 900);
        assert_eq!(canvas.height(), 600);
    }
    
    #[test]
    fn test_draw_line() {
        let mut canvas = Canvas::new(100, 100);
        let msg = DrawMessage::new(
            DrawType::Line,
            (255, 0, 0, 255),
            2.0,
            (10.0, 10.0),
            (50.0, 50.0),
        );
        canvas.draw(&msg);
        
        let png = canvas.to_png();
        assert!(!png.is_empty());
    }
    
    #[test]
    fn test_export_png() {
        let canvas = Canvas::new(100, 100);
        let png = canvas.to_png();
        
        // PNG magic bytes
        assert_eq!(&png[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    }
}
```

### 5.6 Room Management

```rust
// src/room/room.rs
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;
use axum::extract::ws::Message;
use uuid::Uuid;

use crate::drawing::{DrawMessage, Canvas};
use crate::room::Player;

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
    
    pub fn add_player(&mut self, id: Uuid, player: Player) {
        if !self.closed {
            self.players.insert(id, player);
        }
    }
    
    pub fn remove_player(&mut self, id: &Uuid) -> Option<Player> {
        self.players.remove(id)
    }
    
    pub fn player_count(&self) -> usize {
        self.players.len()
    }
    
    pub fn get_canvas_png(&self) -> Vec<u8> {
        self.canvas.to_png()
    }
    
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
            player.buffered_messages.push(msg_string.clone());
        }
    }
    
    /// Broadcast a simple text message to all players
    pub async fn broadcast_message(&self, message: &str) {
        for player in self.players.values() {
            let _ = player.sender.send(Message::Text(message.to_string()));
        }
    }
    
    /// Flush buffered draw messages for all players
    pub fn flush_buffered_messages(&mut self) {
        for player in self.players.values_mut() {
            if !player.buffered_messages.is_empty() {
                let batched: Vec<String> = player.buffered_messages
                    .drain(..)
                    .map(|m| format!("{},{}", player.last_received_message_id, m))
                    .collect();
                
                let message = format!("1{}", batched.join("|"));
                let _ = player.sender.send(Message::Text(message));
            }
        }
    }
    
    pub fn shutdown(&mut self) {
        self.closed = true;
        self.players.clear();
    }
    
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        
        room.add_player(id, player);
        assert_eq!(room.player_count(), 1);
        
        room.remove_player(&id);
        assert_eq!(room.player_count(), 0);
    }
    
    #[test]
    fn test_max_players() {
        let room = Room::new(900, 600);
        // Room enforces max in handler, not internally
        assert_eq!(room.player_count(), 0);
    }
}
```

### 5.7 Player

```rust
// src/room/player.rs
use tokio::sync::mpsc::UnboundedSender;
use axum::extract::ws::Message;

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
}
```

### 5.8 Broadcaster

```rust
// src/room/broadcaster.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

use crate::room::Room;

const BROADCAST_INTERVAL_MS: u64 = 30;

pub async fn start_broadcast_timer(room: Arc<RwLock<Room>>) {
    let mut timer = interval(Duration::from_millis(BROADCAST_INTERVAL_MS));
    
    loop {
        timer.tick().await;
        
        let mut room_guard = room.write().await;
        if room_guard.is_closed() {
            break;
        }
        
        room_guard.flush_buffered_messages();
    }
}
```

### 5.9 Error Types

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DrawboardError {
    #[error("Invalid draw type: {0}")]
    InvalidDrawType(i32),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Room is full (max 100 players)")]
    RoomFull,
    
    #[error("Room is closed")]
    RoomClosed,
    
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

### 5.10 Module Exports

```rust
// src/room/mod.rs
mod room;
mod player;
pub mod broadcaster;

pub use room::Room;
pub use player::Player;
```

```rust
// src/drawing/mod.rs
mod draw_message;
mod canvas;

pub use draw_message::{DrawMessage, DrawType};
pub use canvas::Canvas;
```

```rust
// src/websocket/mod.rs
pub mod handler;
pub mod message;
```

```rust
// src/lib.rs
pub mod websocket;
pub mod room;
pub mod drawing;
pub mod error;
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
    <title>Drawboard - Rust WebSocket</title>
    <link rel="stylesheet" href="style.css">
</head>
<body>
    <div class="container">
        <div id="drawContainer" class="hidden">
            <div id="labelContainer">
                <span id="playerCount">Players: 0</span>
            </div>
            <canvas id="canvas"></canvas>
            
            <div id="toolbar">
                <div class="tool-group">
                    <label>Tool:</label>
                    <select id="toolSelect">
                        <option value="1">Brush</option>
                        <option value="2">Line</option>
                        <option value="3">Rectangle</option>
                        <option value="4">Ellipse</option>
                    </select>
                </div>
                
                <div class="tool-group">
                    <label>Thickness:</label>
                    <select id="thicknessSelect">
                        <option value="2">2</option>
                        <option value="3">3</option>
                        <option value="6" selected>6</option>
                        <option value="10">10</option>
                        <option value="16">16</option>
                        <option value="28">28</option>
                        <option value="50">50</option>
                    </select>
                </div>
                
                <div class="tool-group">
                    <label>Color:</label>
                    <input type="color" id="colorPicker" value="#000000">
                    <input type="range" id="alphaSlider" min="0" max="255" value="255">
                </div>
            </div>
        </div>
        
        <div id="console-container">
            <h3>Console</h3>
            <div id="console"></div>
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
    font-family: Arial, sans-serif;
    font-size: 11pt;
    background-color: #eeeeea;
    padding: 20px;
}

.container {
    display: flex;
    gap: 20px;
    flex-wrap: wrap;
}

.hidden {
    display: none !important;
}

#drawContainer {
    display: flex;
    flex-direction: column;
    gap: 15px;
    background-color: #fff;
    padding: 15px;
    box-shadow: 0 0 8px 3px #bbb;
    border: 1px solid #ccc;
}

#labelContainer {
    font-weight: bold;
    color: #333;
}

#canvas {
    border: 1px solid #999;
    cursor: crosshair;
    touch-action: none;
}

#toolbar {
    display: flex;
    gap: 20px;
    flex-wrap: wrap;
    padding: 10px;
    background-color: #f5f5f5;
    border-radius: 4px;
}

.tool-group {
    display: flex;
    align-items: center;
    gap: 8px;
}

.tool-group label {
    font-weight: bold;
    color: #555;
}

.tool-group select,
.tool-group input[type="color"] {
    padding: 5px;
    border: 1px solid #ccc;
    border-radius: 4px;
}

.tool-group input[type="range"] {
    width: 80px;
}

#console-container {
    background-color: #fff;
    width: 300px;
    box-shadow: 0 0 8px 3px #bbb;
    border: 1px solid #ccc;
}

#console-container h3 {
    padding: 10px;
    background-color: #f0f0f0;
    border-bottom: 1px solid #ccc;
}

#console {
    height: 400px;
    overflow-y: auto;
    padding: 10px;
    font-size: 10pt;
}

#console p {
    margin: 0 0 5px 0;
    padding: 2px 0;
    border-bottom: 1px dotted #eee;
}

#console p.error {
    color: #c00;
}

#console p.info {
    color: #060;
}
```

### 6.3 JavaScript (static/app.js)

```javascript
"use strict";

(function() {
    // Configuration
    const CANVAS_WIDTH = 900;
    const CANVAS_HEIGHT = 600;
    const PING_INTERVAL = 30000; // 30 seconds
    
    // State
    let socket = null;
    let pingTimerId = null;
    let isStarted = false;
    let playerCount = 0;
    let nextMsgId = 1;
    
    // Pending paths (not yet confirmed by server)
    let pathsNotHandled = [];
    
    // Drawing state
    let isDrawing = false;
    let lastX = 0;
    let lastY = 0;
    let startX = 0;
    let startY = 0;
    
    // Current tool settings
    let currentTool = 1;      // 1=Brush, 2=Line, 3=Rectangle, 4=Ellipse
    let currentThickness = 6;
    let currentColor = [0, 0, 0, 255]; // RGBA
    
    // Canvas layers
    let canvasDisplay = null;
    let canvasBackground = null;
    let canvasServerImage = null;
    
    let ctxDisplay = null;
    let ctxBackground = null;
    let ctxServerImage = null;
    
    // DOM Elements
    let drawContainer = null;
    let playerCountLabel = null;
    
    // Console logging
    const Console = {
        log: function(message, type = '') {
            const console = document.getElementById('console');
            const p = document.createElement('p');
            p.textContent = message;
            if (type) p.className = type;
            console.appendChild(p);
            console.scrollTop = console.scrollHeight;
        }
    };
    
    // Initialize when DOM is ready
    document.addEventListener('DOMContentLoaded', init);
    
    function init() {
        Console.log('Initializing Drawboard...', 'info');
        
        drawContainer = document.getElementById('drawContainer');
        playerCountLabel = document.getElementById('playerCount');
        
        // Setup canvases
        setupCanvases();
        
        // Setup toolbar
        setupToolbar();
        
        // Connect to WebSocket
        connect();
    }
    
    function setupCanvases() {
        // Get the visible canvas
        canvasDisplay = document.getElementById('canvas');
        canvasDisplay.width = CANVAS_WIDTH;
        canvasDisplay.height = CANVAS_HEIGHT;
        ctxDisplay = canvasDisplay.getContext('2d');
        
        // Create background canvas (server state + pending drawings)
        canvasBackground = document.createElement('canvas');
        canvasBackground.width = CANVAS_WIDTH;
        canvasBackground.height = CANVAS_HEIGHT;
        ctxBackground = canvasBackground.getContext('2d');
        
        // Create server image canvas (confirmed server state)
        canvasServerImage = document.createElement('canvas');
        canvasServerImage.width = CANVAS_WIDTH;
        canvasServerImage.height = CANVAS_HEIGHT;
        ctxServerImage = canvasServerImage.getContext('2d');
        
        // Fill with white
        ctxServerImage.fillStyle = '#ffffff';
        ctxServerImage.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
        
        // Setup event listeners
        canvasDisplay.addEventListener('mousedown', handleMouseDown);
        canvasDisplay.addEventListener('mousemove', handleMouseMove);
        canvasDisplay.addEventListener('mouseup', handleMouseUp);
        canvasDisplay.addEventListener('mouseleave', handleMouseUp);
        
        // Touch events
        canvasDisplay.addEventListener('touchstart', handleTouchStart);
        canvasDisplay.addEventListener('touchmove', handleTouchMove);
        canvasDisplay.addEventListener('touchend', handleTouchEnd);
    }
    
    function setupToolbar() {
        document.getElementById('toolSelect').addEventListener('change', (e) => {
            currentTool = parseInt(e.target.value);
        });
        
        document.getElementById('thicknessSelect').addEventListener('change', (e) => {
            currentThickness = parseInt(e.target.value);
        });
        
        document.getElementById('colorPicker').addEventListener('change', (e) => {
            const hex = e.target.value;
            currentColor[0] = parseInt(hex.substr(1, 2), 16);
            currentColor[1] = parseInt(hex.substr(3, 2), 16);
            currentColor[2] = parseInt(hex.substr(5, 2), 16);
        });
        
        document.getElementById('alphaSlider').addEventListener('input', (e) => {
            currentColor[3] = parseInt(e.target.value);
        });
    }
    
    function connect() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const host = window.location.host;
        const url = `${protocol}//${host}/ws/drawboard`;
        
        Console.log(`Connecting to ${url}...`);
        
        socket = new WebSocket(url);
        
        socket.onopen = function() {
            Console.log('WebSocket connection opened.', 'info');
            
            // Start ping timer
            pingTimerId = setInterval(() => {
                if (socket.readyState === WebSocket.OPEN) {
                    socket.send('0'); // Pong message
                }
            }, PING_INTERVAL);
        };
        
        socket.onclose = function(event) {
            Console.log(`WebSocket closed: ${event.reason || 'Connection closed'}`, 'error');
            clearInterval(pingTimerId);
            isStarted = false;
            drawContainer.classList.add('hidden');
        };
        
        socket.onerror = function(error) {
            Console.log('WebSocket error occurred.', 'error');
        };
        
        socket.onmessage = handleMessage;
    }
    
    function handleMessage(event) {
        // Check if binary message (PNG image)
        if (event.data instanceof Blob) {
            handleBinaryMessage(event.data);
            return;
        }
        
        const messages = event.data.split(';');
        
        for (const msg of messages) {
            if (msg.length === 0) continue;
            
            const type = msg.charAt(0);
            const content = msg.substring(1);
            
            switch (type) {
                case '0': // Error
                    Console.log(`Error: ${content}`, 'error');
                    alert(content);
                    break;
                    
                case '1': // Draw message(s)
                    handleDrawMessages(content);
                    break;
                    
                case '2': // Image message (player count)
                    playerCount = parseInt(content);
                    updatePlayerCount();
                    Console.log(`Joined room with ${playerCount} player(s).`, 'info');
                    // Next message will be binary PNG
                    break;
                    
                case '3': // Player changed
                    if (content === '+') {
                        playerCount++;
                        Console.log('A player joined.');
                    } else {
                        playerCount--;
                        Console.log('A player left.');
                    }
                    updatePlayerCount();
                    break;
            }
        }
    }
    
    function handleBinaryMessage(blob) {
        const url = URL.createObjectURL(blob);
        const img = new Image();
        
        img.onload = function() {
            // Draw server image
            ctxServerImage.drawImage(img, 0, 0);
            
            // Initialize display
            updateDisplay();
            
            // Show drawing container
            drawContainer.classList.remove('hidden');
            isStarted = true;
            
            URL.revokeObjectURL(url);
            Console.log('Room image loaded.', 'info');
        };
        
        img.onerror = function() {
            Console.log('Failed to load room image.', 'error');
            URL.revokeObjectURL(url);
        };
        
        img.src = url;
    }
    
    function handleDrawMessages(content) {
        // Format: "lastMsgId,drawData" or "msg1|msg2|..."
        const messages = content.split('|');
        let maxLastHandledId = 0;
        
        for (const msgStr of messages) {
            const parts = msgStr.split(',');
            const lastHandledId = parseInt(parts[0]);
            maxLastHandledId = Math.max(maxLastHandledId, lastHandledId);
            
            // Parse draw message
            const drawMsg = {
                type: parseInt(parts[1]),
                colorR: parseInt(parts[2]),
                colorG: parseInt(parts[3]),
                colorB: parseInt(parts[4]),
                colorA: parseInt(parts[5]),
                thickness: parseFloat(parts[6]),
                x1: parseFloat(parts[7]),
                y1: parseFloat(parts[8]),
                x2: parseFloat(parts[9]),
                y2: parseFloat(parts[10])
            };
            
            // Draw on server canvas
            drawPath(ctxServerImage, drawMsg);
        }
        
        // Remove handled paths from pending queue
        while (pathsNotHandled.length > 0 && pathsNotHandled[0].id <= maxLastHandledId) {
            pathsNotHandled.shift();
        }
        
        // Update display
        updateDisplay();
    }
    
    function drawPath(ctx, msg) {
        ctx.save();
        ctx.lineCap = 'round';
        ctx.lineJoin = 'miter';
        ctx.lineWidth = msg.thickness;
        ctx.strokeStyle = `rgba(${msg.colorR}, ${msg.colorG}, ${msg.colorB}, ${msg.colorA / 255})`;
        
        ctx.beginPath();
        
        if (msg.x1 === msg.x2 && msg.y1 === msg.y2) {
            // Draw a point
            ctx.arc(msg.x1, msg.y1, 0.5, 0, Math.PI * 2);
        } else if (msg.type === 1 || msg.type === 2) {
            // Brush or Line
            ctx.moveTo(msg.x1, msg.y1);
            ctx.lineTo(msg.x2, msg.y2);
        } else if (msg.type === 3) {
            // Rectangle
            const x = Math.min(msg.x1, msg.x2);
            const y = Math.min(msg.y1, msg.y2);
            const w = Math.abs(msg.x2 - msg.x1);
            const h = Math.abs(msg.y2 - msg.y1);
            ctx.rect(x, y, w, h);
        } else if (msg.type === 4) {
            // Ellipse
            const cx = (msg.x1 + msg.x2) / 2;
            const cy = (msg.y1 + msg.y2) / 2;
            const rx = Math.abs(msg.x2 - msg.x1) / 2;
            const ry = Math.abs(msg.y2 - msg.y1) / 2;
            ctx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
        }
        
        ctx.stroke();
        ctx.restore();
    }
    
    function updateDisplay() {
        // Copy server image to background
        ctxBackground.drawImage(canvasServerImage, 0, 0);
        
        // Draw pending paths on background
        for (const container of pathsNotHandled) {
            drawPath(ctxBackground, container.path);
        }
        
        // Copy background to display
        ctxDisplay.drawImage(canvasBackground, 0, 0);
    }
    
    function updatePlayerCount() {
        playerCountLabel.textContent = `Players: ${playerCount}`;
    }
    
    function getCanvasCoords(event) {
        const rect = canvasDisplay.getBoundingClientRect();
        return {
            x: event.clientX - rect.left,
            y: event.clientY - rect.top
        };
    }
    
    function getTouchCoords(event) {
        const touch = event.touches[0] || event.changedTouches[0];
        return getCanvasCoords(touch);
    }
    
    // Mouse event handlers
    function handleMouseDown(event) {
        if (!isStarted) return;
        
        const coords = getCanvasCoords(event);
        startDrawing(coords.x, coords.y);
    }
    
    function handleMouseMove(event) {
        if (!isStarted || !isDrawing) return;
        
        const coords = getCanvasCoords(event);
        continueDrawing(coords.x, coords.y);
    }
    
    function handleMouseUp(event) {
        if (!isStarted || !isDrawing) return;
        
        const coords = getCanvasCoords(event);
        endDrawing(coords.x, coords.y);
    }
    
    // Touch event handlers
    function handleTouchStart(event) {
        event.preventDefault();
        if (!isStarted) return;
        
        const coords = getTouchCoords(event);
        startDrawing(coords.x, coords.y);
    }
    
    function handleTouchMove(event) {
        event.preventDefault();
        if (!isStarted || !isDrawing) return;
        
        const coords = getTouchCoords(event);
        continueDrawing(coords.x, coords.y);
    }
    
    function handleTouchEnd(event) {
        event.preventDefault();
        if (!isStarted || !isDrawing) return;
        
        const coords = getTouchCoords(event);
        endDrawing(coords.x, coords.y);
    }
    
    function startDrawing(x, y) {
        isDrawing = true;
        startX = x;
        startY = y;
        lastX = x;
        lastY = y;
        
        if (currentTool === 1) {
            // Brush: start with a point
            sendDrawMessage(x, y, x, y);
        }
    }
    
    function continueDrawing(x, y) {
        if (currentTool === 1) {
            // Brush: continuous drawing
            sendDrawMessage(lastX, lastY, x, y);
            lastX = x;
            lastY = y;
        } else {
            // Preview shape on display canvas
            updateDisplay();
            drawPreview(x, y);
        }
    }
    
    function endDrawing(x, y) {
        if (!isDrawing) return;
        isDrawing = false;
        
        if (currentTool !== 1) {
            // Line, Rectangle, Ellipse: send final shape
            sendDrawMessage(startX, startY, x, y);
        }
        
        updateDisplay();
    }
    
    function drawPreview(x, y) {
        const msg = {
            type: currentTool,
            colorR: currentColor[0],
            colorG: currentColor[1],
            colorB: currentColor[2],
            colorA: currentColor[3],
            thickness: currentThickness,
            x1: startX,
            y1: startY,
            x2: x,
            y2: y
        };
        
        drawPath(ctxDisplay, msg);
    }
    
    function sendDrawMessage(x1, y1, x2, y2) {
        const msg = {
            type: currentTool,
            colorR: currentColor[0],
            colorG: currentColor[1],
            colorB: currentColor[2],
            colorA: currentColor[3],
            thickness: currentThickness,
            x1: x1,
            y1: y1,
            x2: x2,
            y2: y2
        };
        
        const msgId = nextMsgId++;
        
        // Add to pending queue
        pathsNotHandled.push({ id: msgId, path: msg });
        
        // Build message string: "1<msgId>|type,R,G,B,A,thickness,x1,y1,x2,y2"
        const msgStr = `1${msgId}|${msg.type},${msg.colorR},${msg.colorG},${msg.colorB},` +
                       `${msg.colorA},${msg.thickness},${msg.x1},${msg.y1},${msg.x2},${msg.y2}`;
        
        // Send to server
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(msgStr);
        }
        
        // Draw on background immediately (optimistic update)
        drawPath(ctxBackground, msg);
        updateDisplay();
    }
    
})();
```

---

## 7. Testing Strategy

### 7.1 Unit Tests

#### 7.1.1 DrawMessage Parsing Tests

```rust
// tests/unit/draw_message_test.rs
use drawboard_rs::drawing::{DrawMessage, DrawType};

#[test]
fn test_parse_brush_message() {
    let msg = DrawMessage::parse("1,255,128,64,200,10.5,100.0,150.0,200.0,250.0").unwrap();
    
    assert_eq!(msg.draw_type, DrawType::Brush);
    assert_eq!(msg.color_r, 255);
    assert_eq!(msg.color_g, 128);
    assert_eq!(msg.color_b, 64);
    assert_eq!(msg.color_a, 200);
    assert_eq!(msg.thickness, 10.5);
    assert_eq!(msg.x1, 100.0);
    assert_eq!(msg.y1, 150.0);
    assert_eq!(msg.x2, 200.0);
    assert_eq!(msg.y2, 250.0);
}

#[test]
fn test_parse_all_draw_types() {
    for (type_id, expected) in [(1, DrawType::Brush), (2, DrawType::Line), 
                                 (3, DrawType::Rectangle), (4, DrawType::Ellipse)] {
        let msg = DrawMessage::parse(&format!("{},0,0,0,255,1.0,0,0,10,10", type_id)).unwrap();
        assert_eq!(msg.draw_type, expected);
    }
}

#[test]
fn test_parse_invalid_type() {
    let result = DrawMessage::parse("5,0,0,0,255,1.0,0,0,10,10");
    assert!(result.is_err());
    
    let result = DrawMessage::parse("0,0,0,0,255,1.0,0,0,10,10");
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_thickness() {
    // Negative
    let result = DrawMessage::parse("1,0,0,0,255,-1.0,0,0,10,10");
    assert!(result.is_err());
    
    // Too large
    let result = DrawMessage::parse("1,0,0,0,255,101.0,0,0,10,10");
    assert!(result.is_err());
    
    // NaN
    let result = DrawMessage::parse("1,0,0,0,255,NaN,0,0,10,10");
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_coordinates() {
    let result = DrawMessage::parse("1,0,0,0,255,1.0,NaN,0,10,10");
    assert!(result.is_err());
}

#[test]
fn test_parse_missing_elements() {
    let result = DrawMessage::parse("1,0,0,0,255,1.0,0,0,10");
    assert!(result.is_err());
}

#[test]
fn test_serialization_roundtrip() {
    let original = DrawMessage::new(
        DrawType::Rectangle,
        (100, 150, 200, 128),
        15.5,
        (50.5, 60.5),
        (200.5, 300.5),
    );
    
    let serialized = original.to_string();
    let parsed = DrawMessage::parse(&serialized).unwrap();
    
    assert_eq!(parsed.draw_type, original.draw_type);
    assert_eq!(parsed.color_r, original.color_r);
    assert_eq!(parsed.color_g, original.color_g);
    assert_eq!(parsed.color_b, original.color_b);
    assert_eq!(parsed.color_a, original.color_a);
    assert!((parsed.thickness - original.thickness).abs() < 0.001);
    assert!((parsed.x1 - original.x1).abs() < 0.001);
    assert!((parsed.y1 - original.y1).abs() < 0.001);
    assert!((parsed.x2 - original.x2).abs() < 0.001);
    assert!((parsed.y2 - original.y2).abs() < 0.001);
}

#[test]
fn test_color_boundary_values() {
    // Min values
    let msg = DrawMessage::parse("1,0,0,0,0,0,0,0,0,0").unwrap();
    assert_eq!(msg.color_r, 0);
    assert_eq!(msg.color_a, 0);
    
    // Max values
    let msg = DrawMessage::parse("1,255,255,255,255,100,0,0,0,0").unwrap();
    assert_eq!(msg.color_r, 255);
    assert_eq!(msg.color_a, 255);
}
```

#### 7.1.2 Room Tests

```rust
// tests/unit/room_test.rs
use drawboard_rs::room::{Room, Player};
use drawboard_rs::drawing::{DrawMessage, DrawType};
use tokio::sync::mpsc;
use uuid::Uuid;

#[test]
fn test_room_creation() {
    let room = Room::new(900, 600);
    assert_eq!(room.player_count(), 0);
    assert!(!room.is_closed());
}

#[test]
fn test_add_player() {
    let mut room = Room::new(900, 600);
    let (tx, _rx) = mpsc::unbounded_channel();
    let player = Player::new(tx);
    let id = Uuid::new_v4();
    
    room.add_player(id, player);
    assert_eq!(room.player_count(), 1);
}

#[test]
fn test_remove_player() {
    let mut room = Room::new(900, 600);
    let (tx, _rx) = mpsc::unbounded_channel();
    let player = Player::new(tx);
    let id = Uuid::new_v4();
    
    room.add_player(id, player);
    assert_eq!(room.player_count(), 1);
    
    let removed = room.remove_player(&id);
    assert!(removed.is_some());
    assert_eq!(room.player_count(), 0);
}

#[test]
fn test_remove_nonexistent_player() {
    let mut room = Room::new(900, 600);
    let id = Uuid::new_v4();
    
    let removed = room.remove_player(&id);
    assert!(removed.is_none());
}

#[test]
fn test_get_canvas_png() {
    let room = Room::new(100, 100);
    let png = room.get_canvas_png();
    
    // Check PNG magic bytes
    assert_eq!(&png[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
}

#[test]
fn test_handle_draw_message() {
    let mut room = Room::new(100, 100);
    let (tx, mut rx) = mpsc::unbounded_channel();
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
    
    // Check that message was buffered
    room.flush_buffered_messages();
    
    // Should receive the message
    let received = rx.try_recv();
    assert!(received.is_ok());
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
fn test_no_add_after_shutdown() {
    let mut room = Room::new(100, 100);
    room.shutdown();
    
    let (tx, _rx) = mpsc::unbounded_channel();
    let player = Player::new(tx);
    let id = Uuid::new_v4();
    
    room.add_player(id, player);
    assert_eq!(room.player_count(), 0); // Should not add
}
```

#### 7.1.3 Canvas Tests

```rust
// tests/unit/canvas_test.rs
use drawboard_rs::drawing::{Canvas, DrawMessage, DrawType};

#[test]
fn test_canvas_creation() {
    let canvas = Canvas::new(900, 600);
    assert_eq!(canvas.width(), 900);
    assert_eq!(canvas.height(), 600);
}

#[test]
fn test_canvas_draw_line() {
    let mut canvas = Canvas::new(100, 100);
    let msg = DrawMessage::new(
        DrawType::Line,
        (255, 0, 0, 255),
        2.0,
        (10.0, 10.0),
        (50.0, 50.0),
    );
    
    // Should not panic
    canvas.draw(&msg);
}

#[test]
fn test_canvas_draw_rectangle() {
    let mut canvas = Canvas::new(100, 100);
    let msg = DrawMessage::new(
        DrawType::Rectangle,
        (0, 255, 0, 128),
        5.0,
        (20.0, 20.0),
        (80.0, 80.0),
    );
    
    canvas.draw(&msg);
}

#[test]
fn test_canvas_draw_ellipse() {
    let mut canvas = Canvas::new(100, 100);
    let msg = DrawMessage::new(
        DrawType::Ellipse,
        (0, 0, 255, 255),
        3.0,
        (10.0, 10.0),
        (90.0, 90.0),
    );
    
    canvas.draw(&msg);
}

#[test]
fn test_canvas_draw_point() {
    let mut canvas = Canvas::new(100, 100);
    let msg = DrawMessage::new(
        DrawType::Brush,
        (0, 0, 0, 255),
        10.0,
        (50.0, 50.0),
        (50.0, 50.0), // Same point
    );
    
    canvas.draw(&msg);
}

#[test]
fn test_canvas_to_png() {
    let canvas = Canvas::new(100, 100);
    let png = canvas.to_png();
    
    assert!(!png.is_empty());
    // PNG magic bytes
    assert_eq!(&png[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
}

#[test]
fn test_canvas_png_changes_after_draw() {
    let mut canvas = Canvas::new(100, 100);
    let png_before = canvas.to_png();
    
    let msg = DrawMessage::new(
        DrawType::Line,
        (255, 0, 0, 255),
        10.0,
        (0.0, 0.0),
        (100.0, 100.0),
    );
    canvas.draw(&msg);
    
    let png_after = canvas.to_png();
    
    // PNGs should be different
    assert_ne!(png_before, png_after);
}
```

### 7.2 Integration Tests

```rust
// tests/integration/websocket_test.rs
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tokio::sync::RwLock;
use std::sync::Arc;
use tower::ServiceExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

use drawboard_rs::{AppState, room::Room};

async fn create_test_app() -> Router {
    let room = Arc::new(RwLock::new(Room::new(100, 100)));
    let state = AppState { room };
    
    Router::new()
        .route("/ws/drawboard", axum::routing::get(drawboard_rs::websocket::handler::ws_handler))
        .with_state(state)
}

#[tokio::test]
async fn test_websocket_connection() {
    let app = create_test_app().await;
    
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    let url = format!("ws://{}/ws/drawboard", addr);
    let (mut ws_stream, _) = connect_async(&url).await.expect("Failed to connect");
    
    // Should receive IMAGE_MESSAGE first
    let msg = ws_stream.next().await.unwrap().unwrap();
    if let Message::Text(text) = msg {
        assert!(text.starts_with('2')); // IMAGE_MESSAGE
    }
    
    // Should receive binary PNG
    let msg = ws_stream.next().await.unwrap().unwrap();
    assert!(matches!(msg, Message::Binary(_)));
    
    ws_stream.close(None).await.unwrap();
}

#[tokio::test]
async fn test_draw_message_broadcast() {
    let app = create_test_app().await;
    
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    let url = format!("ws://{}/ws/drawboard", addr);
    
    // Connect two clients
    let (mut ws1, _) = connect_async(&url).await.unwrap();
    let (mut ws2, _) = connect_async(&url).await.unwrap();
    
    // Skip initial messages
    for _ in 0..2 {
        ws1.next().await;
        ws2.next().await;
    }
    
    // Client 1 sends draw message
    let draw_msg = "1|1,255,0,0,255,5,10,10,50,50";
    ws1.send(Message::Text(format!("1{}", draw_msg))).await.unwrap();
    
    // Wait for broadcast (30ms timer)
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    ws1.close(None).await.ok();
    ws2.close(None).await.ok();
}

#[tokio::test]
async fn test_player_join_notification() {
    let app = create_test_app().await;
    
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    let url = format!("ws://{}/ws/drawboard", addr);
    
    // Connect first client
    let (mut ws1, _) = connect_async(&url).await.unwrap();
    
    // Skip initial messages for ws1
    ws1.next().await; // IMAGE_MESSAGE
    ws1.next().await; // Binary PNG
    
    // Connect second client - ws1 should receive "3+"
    let (mut ws2, _) = connect_async(&url).await.unwrap();
    
    // ws1 should receive player joined notification
    if let Some(Ok(Message::Text(text))) = ws1.next().await {
        assert_eq!(text, "3+");
    }
    
    ws1.close(None).await.ok();
    ws2.close(None).await.ok();
}

#[tokio::test]
async fn test_pong_message() {
    let app = create_test_app().await;
    
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    let url = format!("ws://{}/ws/drawboard", addr);
    let (mut ws, _) = connect_async(&url).await.unwrap();
    
    // Skip initial messages
    ws.next().await;
    ws.next().await;
    
    // Send pong
    ws.send(Message::Text("0".to_string())).await.unwrap();
    
    // Should not cause any error
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    ws.close(None).await.ok();
}
```

### 7.3 Client-side Tests (JavaScript)

```javascript
// tests/client/drawboard.test.js
// Using Jest or similar testing framework

describe('DrawMessage', () => {
    test('should create valid draw message string', () => {
        const msg = {
            type: 1,
            colorR: 255,
            colorG: 128,
            colorB: 64,
            colorA: 200,
            thickness: 10.5,
            x1: 100,
            y1: 150,
            x2: 200,
            y2: 250
        };
        
        const msgStr = `${msg.type},${msg.colorR},${msg.colorG},${msg.colorB},` +
                       `${msg.colorA},${msg.thickness},${msg.x1},${msg.y1},${msg.x2},${msg.y2}`;
        
        expect(msgStr).toBe('1,255,128,64,200,10.5,100,150,200,250');
    });
    
    test('should parse draw message from server', () => {
        const msgStr = '1,255,0,0,255,5,100.5,200.5,150.5,250.5';
        const parts = msgStr.split(',');
        
        const msg = {
            type: parseInt(parts[0]),
            colorR: parseInt(parts[1]),
            colorG: parseInt(parts[2]),
            colorB: parseInt(parts[3]),
            colorA: parseInt(parts[4]),
            thickness: parseFloat(parts[5]),
            x1: parseFloat(parts[6]),
            y1: parseFloat(parts[7]),
            x2: parseFloat(parts[8]),
            y2: parseFloat(parts[9])
        };
        
        expect(msg.type).toBe(1);
        expect(msg.colorR).toBe(255);
        expect(msg.thickness).toBe(5);
        expect(msg.x1).toBe(100.5);
    });
});

describe('Message Types', () => {
    test('should identify error message', () => {
        const msg = '0Connection error';
        expect(msg.charAt(0)).toBe('0');
        expect(msg.substring(1)).toBe('Connection error');
    });
    
    test('should identify draw message', () => {
        const msg = '11,1,255,0,0,255,5,10,10,50,50';
        expect(msg.charAt(0)).toBe('1');
    });
    
    test('should identify image message', () => {
        const msg = '25';
        expect(msg.charAt(0)).toBe('2');
        expect(parseInt(msg.substring(1))).toBe(5);
    });
    
    test('should identify player changed message', () => {
        expect('3+'.charAt(0)).toBe('3');
        expect('3+'.substring(1)).toBe('+');
        expect('3-'.substring(1)).toBe('-');
    });
});

describe('Batched Messages', () => {
    test('should parse batched draw messages', () => {
        const content = '1,1,255,0,0,255,5,10,10,20,20|2,1,0,255,0,255,3,30,30,40,40';
        const messages = content.split('|');
        
        expect(messages.length).toBe(2);
        
        const msg1Parts = messages[0].split(',');
        expect(parseInt(msg1Parts[0])).toBe(1); // lastMsgId
        
        const msg2Parts = messages[1].split(',');
        expect(parseInt(msg2Parts[0])).toBe(2); // lastMsgId
    });
    
    test('should handle single message', () => {
        const content = '1,1,255,0,0,255,5,10,10,20,20';
        const messages = content.split('|');
        
        expect(messages.length).toBe(1);
    });
});

describe('Path Queue Management', () => {
    test('should remove handled paths', () => {
        let pathsNotHandled = [
            { id: 1, path: {} },
            { id: 2, path: {} },
            { id: 3, path: {} },
            { id: 4, path: {} }
        ];
        
        const maxLastHandledId = 2;
        
        while (pathsNotHandled.length > 0 && pathsNotHandled[0].id <= maxLastHandledId) {
            pathsNotHandled.shift();
        }
        
        expect(pathsNotHandled.length).toBe(2);
        expect(pathsNotHandled[0].id).toBe(3);
    });
});
```

### 7.4 Test Commands

```bash
# Run all Rust tests
cargo test

# Run specific test module
cargo test draw_message_test
cargo test room_test
cargo test canvas_test

# Run integration tests
cargo test --test websocket_test

# Run with output
cargo test -- --nocapture

# Run benchmarks (if added)
cargo bench

# JavaScript tests (using npm)
cd static
npm test
```

---

## 8. Deployment

### 8.1 Build

```bash
# Development
cargo run

# Release build
cargo build --release

# Binary will be at: target/release/drawboard-rs
```

### 8.2 Docker

```dockerfile
# Dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/drawboard-rs /usr/local/bin/
COPY --from=builder /app/static /app/static
WORKDIR /app
EXPOSE 8080
CMD ["drawboard-rs"]
```

```bash
# Build và run
docker build -t drawboard-rs .
docker run -p 8080:8080 drawboard-rs
```

### 8.3 Environment Variables

```bash
# Server configuration
export DRAWBOARD_HOST=0.0.0.0
export DRAWBOARD_PORT=8080
export DRAWBOARD_CANVAS_WIDTH=900
export DRAWBOARD_CANVAS_HEIGHT=600
export DRAWBOARD_MAX_PLAYERS=100
export DRAWBOARD_BROADCAST_INTERVAL_MS=30
export RUST_LOG=info
```

### 8.4 Nginx Reverse Proxy

```nginx
# /etc/nginx/sites-available/drawboard
upstream drawboard {
    server 127.0.0.1:8080;
}

server {
    listen 80;
    server_name drawboard.example.com;

    location / {
        proxy_pass http://drawboard;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 86400;
    }
}
```

---

## 9. Tham khảo

### 9.1 Java Source Files (Original)

- `DrawboardEndpoint.java` - WebSocket endpoint
- `Room.java` - Room & Player management
- `Client.java` - Async message sender
- `DrawMessage.java` - Draw message model
- `drawboard.xhtml` - Client UI

### 9.2 Rust Libraries

- [axum](https://docs.rs/axum) - Web framework
- [tokio](https://tokio.rs) - Async runtime
- [tokio-tungstenite](https://docs.rs/tokio-tungstenite) - WebSocket
- [tiny-skia](https://docs.rs/tiny-skia) - 2D graphics
- [image](https://docs.rs/image) - Image processing

### 9.3 WebSocket Specification

- [RFC 6455](https://tools.ietf.org/html/rfc6455) - The WebSocket Protocol
- [MDN WebSocket API](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)

---

## 10. Changelog

| Version | Date | Changes |
|---------|------|---------|
| 0.1.0 | 2026-01-29 | Initial proposal document |

---

*Tài liệu này được tạo để hướng dẫn chuyển đổi ứng dụng Drawboard WebSocket từ Java Tomcat sang Rust.*
