use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

use crate::room::Room;

/// Broadcast interval in milliseconds (same as Java version)
const BROADCAST_INTERVAL_MS: u64 = 30;

/// Start the broadcast timer that flushes buffered messages
pub async fn start_broadcast_timer(room: Arc<RwLock<Room>>) {
    let mut timer = interval(Duration::from_millis(BROADCAST_INTERVAL_MS));

    loop {
        timer.tick().await;

        let mut room_guard = room.write().await;
        if room_guard.is_closed() {
            tracing::info!("Broadcast timer stopped - room is closed");
            break;
        }

        room_guard.flush_buffered_messages();
    }
}
