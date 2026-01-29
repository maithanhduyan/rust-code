//! Snake entity with game logic

use std::collections::VecDeque;

use super::direction::Direction;
use super::location::Location;
use crate::config::{DEFAULT_SNAKE_LENGTH, MAX_SNAKE_LENGTH, SNAKE_COLORS};
use crate::protocol::{SnakeData, SnakePosition};

/// Atomic counter for snake IDs
static SNAKE_ID_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// A snake in the game
#[derive(Debug, Clone)]
pub struct Snake {
    /// Unique identifier
    pub id: u32,
    /// Snake color (hex format)
    pub color: String,
    /// Current movement direction
    direction: Direction,
    /// Snake body segments (head is front, tail is back)
    body: VecDeque<Location>,
    /// Whether the snake is alive
    alive: bool,
}

impl Snake {
    /// Create a new snake at a random position
    pub fn new() -> Self {
        let id = SNAKE_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let color_index = (id as usize - 1) % SNAKE_COLORS.len();
        let color = SNAKE_COLORS[color_index].to_string();

        // Start at random position
        let head = Location::random();
        let mut body = VecDeque::with_capacity(MAX_SNAKE_LENGTH);
        body.push_front(head);

        // Add initial tail segments - spread out behind the head
        // Start going west (left) from head
        let mut current = head;
        for _ in 1..DEFAULT_SNAKE_LENGTH {
            current = current.adjacent(Direction::West);
            body.push_back(current);
        }

        Self {
            id,
            color,
            direction: Direction::None,
            body,
            alive: true,
        }
    }

    /// Get the snake's head location
    pub fn head(&self) -> Location {
        *self.body.front().expect("Snake must have a head")
    }

    /// Get all body locations (for collision detection)
    pub fn body(&self) -> &VecDeque<Location> {
        &self.body
    }

    /// Get the current direction
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Check if the snake is alive
    pub fn is_alive(&self) -> bool {
        self.alive
    }

    /// Set the snake's direction (prevents 180-degree turns)
    pub fn set_direction(&mut self, new_direction: Direction) {
        // Don't allow reversing direction directly
        if !self.direction.is_opposite(&new_direction) {
            self.direction = new_direction;
        }
    }

    /// Update the snake's position (called each game tick)
    pub fn update(&mut self) {
        if !self.alive || !self.direction.is_moving() {
            return;
        }

        // Calculate new head position
        let new_head = self.head().adjacent(self.direction);

        // Add new head
        self.body.push_front(new_head);

        // Remove tail (snake doesn't grow during normal movement)
        self.body.pop_back();
    }

    /// Kill the snake
    pub fn kill(&mut self) {
        self.alive = false;
    }

    /// Reward the snake for killing another (grow longer)
    pub fn reward(&mut self) {
        if self.body.len() < MAX_SNAKE_LENGTH {
            // Add segment at tail
            if let Some(tail) = self.body.back().copied() {
                self.body.push_back(tail);
            }
        }
    }

    /// Reset the snake to initial state
    pub fn reset(&mut self) {
        let head = Location::random();
        self.body.clear();
        self.body.push_front(head);

        for _ in 1..DEFAULT_SNAKE_LENGTH {
            self.body.push_back(head);
        }

        self.direction = Direction::None;
        self.alive = true;
    }

    /// Convert to SnakeData for protocol messages
    pub fn to_data(&self) -> SnakeData {
        SnakeData {
            id: self.id,
            color: self.color.clone(),
            body: self
                .body
                .iter()
                .map(|loc| SnakePosition { x: loc.x, y: loc.y })
                .collect(),
        }
    }
}

impl Default for Snake {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_snake() {
        let snake = Snake::new();
        assert!(snake.is_alive());
        assert_eq!(snake.body.len(), DEFAULT_SNAKE_LENGTH);
        assert_eq!(snake.direction(), Direction::None);
    }

    #[test]
    fn test_set_direction() {
        let mut snake = Snake::new();
        snake.set_direction(Direction::North);
        assert_eq!(snake.direction(), Direction::North);

        // Should not reverse
        snake.set_direction(Direction::South);
        assert_eq!(snake.direction(), Direction::North);

        // Can turn 90 degrees
        snake.set_direction(Direction::East);
        assert_eq!(snake.direction(), Direction::East);
    }

    #[test]
    fn test_kill_and_reward() {
        let mut snake = Snake::new();
        let initial_len = snake.body.len();

        snake.reward();
        assert_eq!(snake.body.len(), initial_len + 1);

        snake.kill();
        assert!(!snake.is_alive());
    }
}
