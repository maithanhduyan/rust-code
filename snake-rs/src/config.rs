//! Game configuration constants

/// Playfield width in pixels
pub const PLAYFIELD_WIDTH: i32 = 640;

/// Playfield height in pixels
pub const PLAYFIELD_HEIGHT: i32 = 480;

/// Grid size (snake segment size) in pixels
pub const GRID_SIZE: i32 = 10;

/// Game tick delay in milliseconds
pub const TICK_DELAY_MS: u64 = 100;

/// Default snake length when spawning
pub const DEFAULT_SNAKE_LENGTH: usize = 5;

/// Maximum snake length for reward
pub const MAX_SNAKE_LENGTH: usize = 50;

/// WebSocket server port
pub const SERVER_PORT: u16 = 8080;

/// Broadcast channel capacity
pub const BROADCAST_CAPACITY: usize = 100;

/// Available snake colors (hex format)
pub const SNAKE_COLORS: &[&str] = &[
    "#FF0000", // Red
    "#00FF00", // Green
    "#0000FF", // Blue
    "#FFFF00", // Yellow
    "#FF00FF", // Magenta
    "#00FFFF", // Cyan
    "#FFA500", // Orange
    "#800080", // Purple
    "#008080", // Teal
    "#FFD700", // Gold
];

// =============================================================================
// Anti-cheat / Rate Limiting
// =============================================================================

/// Maximum direction commands per second per player
pub const MAX_COMMANDS_PER_SECOND: u32 = 15;

/// Time window for rate limiting (in milliseconds)
pub const RATE_LIMIT_WINDOW_MS: u64 = 1000;

/// Number of violations before player is kicked
pub const MAX_RATE_VIOLATIONS: u32 = 3;

// =============================================================================
// Event Logging
// =============================================================================

/// Enable game event logging
pub const ENABLE_EVENT_LOGGING: bool = true;

/// Log file path
pub const EVENT_LOG_FILE: &str = "game_events.log";
