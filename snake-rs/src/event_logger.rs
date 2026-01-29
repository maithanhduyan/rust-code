//! Game event logging for replay and anti-cheat analysis

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tracing::{error, info};

use crate::config::{ENABLE_EVENT_LOGGING, EVENT_LOG_FILE};
use crate::game::direction::Direction;
use crate::game::location::Location;

/// Types of game events that can be logged
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum GameEvent {
    /// Player connected
    PlayerJoin {
        player_id: u32,
        connection_id: String,
    },
    /// Player disconnected
    PlayerLeave {
        player_id: u32,
        connection_id: String,
    },
    /// Player changed direction
    DirectionChange {
        player_id: u32,
        direction: String,
    },
    /// Player ate food
    FoodEaten {
        player_id: u32,
        food_id: u32,
        location: LocationData,
    },
    /// Player died (collision)
    PlayerDeath {
        victim_id: u32,
        killer_id: Option<u32>,
        cause: String,
    },
    /// Rate limit violation
    RateLimitViolation {
        player_id: u32,
        connection_id: String,
        violation_count: u32,
    },
    /// Player kicked for cheating
    PlayerKicked {
        player_id: u32,
        connection_id: String,
        reason: String,
    },
}

/// Location data for serialization
#[derive(Debug, Clone, Serialize)]
pub struct LocationData {
    pub x: i32,
    pub y: i32,
}

impl From<Location> for LocationData {
    fn from(loc: Location) -> Self {
        Self { x: loc.x, y: loc.y }
    }
}

/// Logged event with timestamp
#[derive(Debug, Serialize)]
struct LogEntry {
    /// Unix timestamp in milliseconds
    timestamp_ms: u128,
    /// The event data
    #[serde(flatten)]
    event: GameEvent,
}

/// Game event logger
pub struct EventLogger {
    /// File writer (None if logging disabled)
    writer: Option<Mutex<BufWriter<File>>>,
    /// Whether logging is enabled
    enabled: bool,
}

impl EventLogger {
    /// Create a new event logger
    pub fn new() -> Self {
        if !ENABLE_EVENT_LOGGING {
            info!("Event logging is disabled");
            return Self {
                writer: None,
                enabled: false,
            };
        }

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(EVENT_LOG_FILE)
        {
            Ok(file) => {
                info!("Event logging enabled, writing to {}", EVENT_LOG_FILE);
                Self {
                    writer: Some(Mutex::new(BufWriter::new(file))),
                    enabled: true,
                }
            }
            Err(e) => {
                error!("Failed to open event log file: {}", e);
                Self {
                    writer: None,
                    enabled: false,
                }
            }
        }
    }

    /// Log a game event
    pub fn log(&self, event: GameEvent) {
        if !self.enabled {
            return;
        }

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        let entry = LogEntry { timestamp_ms, event };

        if let Some(ref writer) = self.writer {
            if let Ok(mut w) = writer.lock() {
                if let Ok(json) = serde_json::to_string(&entry) {
                    let _ = writeln!(w, "{}", json);
                    let _ = w.flush();
                }
            }
        }
    }

    /// Log player join
    pub fn log_join(&self, player_id: u32, connection_id: &str) {
        self.log(GameEvent::PlayerJoin {
            player_id,
            connection_id: connection_id.to_string(),
        });
    }

    /// Log player leave
    pub fn log_leave(&self, player_id: u32, connection_id: &str) {
        self.log(GameEvent::PlayerLeave {
            player_id,
            connection_id: connection_id.to_string(),
        });
    }

    /// Log direction change
    pub fn log_direction(&self, player_id: u32, direction: Direction) {
        self.log(GameEvent::DirectionChange {
            player_id,
            direction: format!("{:?}", direction),
        });
    }

    /// Log food eaten
    pub fn log_food_eaten(&self, player_id: u32, food_id: u32, location: Location) {
        self.log(GameEvent::FoodEaten {
            player_id,
            food_id,
            location: location.into(),
        });
    }

    /// Log player death
    pub fn log_death(&self, victim_id: u32, killer_id: Option<u32>, cause: &str) {
        self.log(GameEvent::PlayerDeath {
            victim_id,
            killer_id,
            cause: cause.to_string(),
        });
    }

    /// Log rate limit violation
    pub fn log_rate_violation(&self, player_id: u32, connection_id: &str, violation_count: u32) {
        self.log(GameEvent::RateLimitViolation {
            player_id,
            connection_id: connection_id.to_string(),
            violation_count,
        });
    }

    /// Log player kicked
    pub fn log_kick(&self, player_id: u32, connection_id: &str, reason: &str) {
        self.log(GameEvent::PlayerKicked {
            player_id,
            connection_id: connection_id.to_string(),
            reason: reason.to_string(),
        });
    }
}

impl Default for EventLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = GameEvent::FoodEaten {
            player_id: 1,
            food_id: 2,
            location: LocationData { x: 100, y: 200 },
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("food_eaten"));
        assert!(json.contains("player_id"));
    }
}
