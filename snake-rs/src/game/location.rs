//! Location struct for grid positions

use serde::{Deserialize, Serialize};
use std::hash::Hash;

use super::direction::Direction;
use crate::config::{GRID_SIZE, PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};

/// A position on the game grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    pub x: i32,
    pub y: i32,
}

impl Location {
    /// Create a new location
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Get the location adjacent to this one in the given direction
    /// Wraps around the playfield edges
    pub fn adjacent(&self, direction: Direction) -> Location {
        let (dx, dy) = match direction {
            Direction::None => (0, 0),
            Direction::North => (0, -GRID_SIZE),
            Direction::South => (0, GRID_SIZE),
            Direction::West => (-GRID_SIZE, 0),
            Direction::East => (GRID_SIZE, 0),
        };

        let mut new_x = self.x + dx;
        let mut new_y = self.y + dy;

        // Wrap around horizontally
        if new_x < 0 {
            new_x = PLAYFIELD_WIDTH - GRID_SIZE;
        } else if new_x >= PLAYFIELD_WIDTH {
            new_x = 0;
        }

        // Wrap around vertically
        if new_y < 0 {
            new_y = PLAYFIELD_HEIGHT - GRID_SIZE;
        } else if new_y >= PLAYFIELD_HEIGHT {
            new_y = 0;
        }

        Location::new(new_x, new_y)
    }

    /// Generate a random location on the grid
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Align to grid
        let max_x = PLAYFIELD_WIDTH / GRID_SIZE;
        let max_y = PLAYFIELD_HEIGHT / GRID_SIZE;

        let x = rng.gen_range(0..max_x) * GRID_SIZE;
        let y = rng.gen_range(0..max_y) * GRID_SIZE;

        Location::new(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjacent() {
        let loc = Location::new(100, 100);

        assert_eq!(loc.adjacent(Direction::North), Location::new(100, 90));
        assert_eq!(loc.adjacent(Direction::South), Location::new(100, 110));
        assert_eq!(loc.adjacent(Direction::West), Location::new(90, 100));
        assert_eq!(loc.adjacent(Direction::East), Location::new(110, 100));
    }

    #[test]
    fn test_wrap_around() {
        // Test left edge wrap
        let left = Location::new(0, 100);
        assert_eq!(left.adjacent(Direction::West).x, PLAYFIELD_WIDTH - GRID_SIZE);

        // Test right edge wrap
        let right = Location::new(PLAYFIELD_WIDTH - GRID_SIZE, 100);
        assert_eq!(right.adjacent(Direction::East).x, 0);

        // Test top edge wrap
        let top = Location::new(100, 0);
        assert_eq!(top.adjacent(Direction::North).y, PLAYFIELD_HEIGHT - GRID_SIZE);

        // Test bottom edge wrap
        let bottom = Location::new(100, PLAYFIELD_HEIGHT - GRID_SIZE);
        assert_eq!(bottom.adjacent(Direction::South).y, 0);
    }
}
