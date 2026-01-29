//! Direction enum for snake movement

use serde::{Deserialize, Serialize};

/// Direction of movement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// No movement
    #[default]
    None,
    /// Moving up
    North,
    /// Moving down
    South,
    /// Moving left
    West,
    /// Moving right
    East,
}

impl Direction {
    /// Parse direction from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Direction::None),
            "north" | "up" => Some(Direction::North),
            "south" | "down" => Some(Direction::South),
            "west" | "left" => Some(Direction::West),
            "east" | "right" => Some(Direction::East),
            _ => None,
        }
    }

    /// Check if this direction is opposite to another
    pub fn is_opposite(&self, other: &Direction) -> bool {
        matches!(
            (self, other),
            (Direction::North, Direction::South)
                | (Direction::South, Direction::North)
                | (Direction::East, Direction::West)
                | (Direction::West, Direction::East)
        )
    }

    /// Check if the snake is moving (not None)
    pub fn is_moving(&self) -> bool {
        !matches!(self, Direction::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        assert_eq!(Direction::from_str("north"), Some(Direction::North));
        assert_eq!(Direction::from_str("SOUTH"), Some(Direction::South));
        assert_eq!(Direction::from_str("invalid"), None);
    }

    #[test]
    fn test_is_opposite() {
        assert!(Direction::North.is_opposite(&Direction::South));
        assert!(Direction::East.is_opposite(&Direction::West));
        assert!(!Direction::North.is_opposite(&Direction::East));
    }
}
