//! Rate limiter for anti-cheat

use std::collections::VecDeque;
use std::time::Instant;

use dashmap::DashMap;
use uuid::Uuid;

use crate::config::{MAX_COMMANDS_PER_SECOND, MAX_RATE_VIOLATIONS, RATE_LIMIT_WINDOW_MS};

/// Rate limiter state for a single player
#[derive(Debug)]
pub struct PlayerRateState {
    /// Timestamps of recent commands
    command_times: VecDeque<Instant>,
    /// Number of rate limit violations
    violations: u32,
}

impl PlayerRateState {
    pub fn new() -> Self {
        Self {
            command_times: VecDeque::with_capacity(MAX_COMMANDS_PER_SECOND as usize + 10),
            violations: 0,
        }
    }

    /// Check if a new command is allowed
    /// Returns (allowed, should_kick)
    pub fn check_command(&mut self) -> (bool, bool) {
        let now = Instant::now();
        let window_start = now - std::time::Duration::from_millis(RATE_LIMIT_WINDOW_MS);

        // Remove old timestamps outside the window
        while let Some(front) = self.command_times.front() {
            if *front < window_start {
                self.command_times.pop_front();
            } else {
                break;
            }
        }

        // Check if under the limit
        if self.command_times.len() < MAX_COMMANDS_PER_SECOND as usize {
            self.command_times.push_back(now);
            (true, false)
        } else {
            // Rate limit exceeded
            self.violations += 1;
            let should_kick = self.violations >= MAX_RATE_VIOLATIONS;
            (false, should_kick)
        }
    }

    /// Get current violation count
    pub fn violation_count(&self) -> u32 {
        self.violations
    }

    /// Get commands in current window
    pub fn commands_in_window(&self) -> usize {
        self.command_times.len()
    }
}

impl Default for PlayerRateState {
    fn default() -> Self {
        Self::new()
    }
}

/// Global rate limiter managing all players
pub struct RateLimiter {
    /// Rate state per player connection
    players: DashMap<Uuid, PlayerRateState>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            players: DashMap::new(),
        }
    }

    /// Register a new player
    pub fn add_player(&self, connection_id: Uuid) {
        self.players.insert(connection_id, PlayerRateState::new());
    }

    /// Remove a player
    pub fn remove_player(&self, connection_id: &Uuid) {
        self.players.remove(connection_id);
    }

    /// Check if a command from a player is allowed
    /// Returns (allowed, should_kick)
    pub fn check_command(&self, connection_id: &Uuid) -> (bool, bool) {
        if let Some(mut state) = self.players.get_mut(connection_id) {
            state.check_command()
        } else {
            // Unknown player, allow but don't track
            (true, false)
        }
    }

    /// Get violation count for a player
    pub fn get_violations(&self, connection_id: &Uuid) -> u32 {
        self.players
            .get(connection_id)
            .map(|s| s.violation_count())
            .unwrap_or(0)
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_rate_limit_allows_normal_usage() {
        let mut state = PlayerRateState::new();

        // Should allow up to MAX_COMMANDS_PER_SECOND
        for _ in 0..MAX_COMMANDS_PER_SECOND {
            let (allowed, _) = state.check_command();
            assert!(allowed);
        }
    }

    #[test]
    fn test_rate_limit_blocks_excess() {
        let mut state = PlayerRateState::new();

        // Fill up the limit
        for _ in 0..MAX_COMMANDS_PER_SECOND {
            state.check_command();
        }

        // Next one should be blocked
        let (allowed, _) = state.check_command();
        assert!(!allowed);
        assert_eq!(state.violation_count(), 1);
    }

    #[test]
    fn test_violations_accumulate() {
        let mut state = PlayerRateState::new();

        // Fill up and violate multiple times
        for _ in 0..MAX_COMMANDS_PER_SECOND {
            state.check_command();
        }

        for i in 1..=MAX_RATE_VIOLATIONS {
            let (allowed, should_kick) = state.check_command();
            assert!(!allowed);
            assert_eq!(state.violation_count(), i);

            if i >= MAX_RATE_VIOLATIONS {
                assert!(should_kick);
            } else {
                assert!(!should_kick);
            }
        }
    }
}
