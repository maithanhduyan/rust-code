//! Application state shared across all handlers

use std::sync::Arc;

use dashmap::DashMap;
use uuid::Uuid;

use crate::broadcast::{Broadcaster, InMemoryBroadcaster};
use crate::event_logger::EventLogger;
use crate::game::{Food, Snake};
use crate::rate_limiter::RateLimiter;

/// Shared application state
pub struct AppState {
    /// All active snakes, keyed by connection UUID
    pub snakes: DashMap<Uuid, Snake>,
    /// Food items on the map
    pub foods: DashMap<u32, Food>,
    /// Broadcaster for sending messages to all clients
    pub broadcaster: Arc<dyn Broadcaster>,
    /// Rate limiter for anti-cheat
    pub rate_limiter: RateLimiter,
    /// Event logger for replay/analysis
    pub event_logger: Arc<EventLogger>,
}

impl AppState {
    /// Create a new application state with in-memory broadcasting
    pub fn new() -> Self {
        let state = Self {
            snakes: DashMap::new(),
            foods: DashMap::new(),
            broadcaster: Arc::new(InMemoryBroadcaster::new()),
            rate_limiter: RateLimiter::new(),
            event_logger: Arc::new(EventLogger::new()),
        };

        // Spawn initial food items
        for i in 0..5 {
            state.foods.insert(i, Food::new());
        }

        state
    }

    /// Create with a custom broadcaster (for testing or Redis)
    pub fn with_broadcaster(broadcaster: Arc<dyn Broadcaster>) -> Self {
        let state = Self {
            snakes: DashMap::new(),
            foods: DashMap::new(),
            broadcaster,
            rate_limiter: RateLimiter::new(),
            event_logger: Arc::new(EventLogger::new()),
        };

        // Spawn initial food items
        for i in 0..5 {
            state.foods.insert(i, Food::new());
        }

        state
    }

    /// Get the number of active players
    pub fn player_count(&self) -> usize {
        self.snakes.len()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
