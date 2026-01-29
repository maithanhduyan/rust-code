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

    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
}
