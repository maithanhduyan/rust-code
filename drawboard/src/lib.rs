pub mod drawing;
pub mod error;
pub mod room;
pub mod websocket;

use std::sync::Arc;
use tokio::sync::RwLock;

use room::Room;

/// Application state shared across all connections
#[derive(Clone)]
pub struct AppState {
    pub room: Arc<RwLock<Room>>,
}

impl AppState {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            room: Arc::new(RwLock::new(Room::new(width, height))),
        }
    }
}
