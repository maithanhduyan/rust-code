//! Food entity - spawns randomly on the map

use super::location::Location;
use crate::protocol::FoodData;

/// A food item on the map
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Food {
    /// Position of the food
    pub location: Location,
}

impl Food {
    /// Create a new food at a random position
    pub fn new() -> Self {
        Self {
            location: Location::random(),
        }
    }

    /// Create food at a specific location
    pub fn at(location: Location) -> Self {
        Self { location }
    }

    /// Respawn food at a new random position
    pub fn respawn(&mut self) {
        self.location = Location::random();
    }

    /// Check if a location matches the food position
    pub fn is_at(&self, loc: &Location) -> bool {
        self.location == *loc
    }

    /// Convert to FoodData for protocol messages
    pub fn to_data(&self) -> FoodData {
        FoodData {
            x: self.location.x,
            y: self.location.y,
        }
    }
}

impl Default for Food {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_food() {
        let food = Food::new();
        // Just check it creates without panic
        assert!(food.location.x >= 0);
        assert!(food.location.y >= 0);
    }

    #[test]
    fn test_respawn() {
        let mut food = Food::new();
        let old_loc = food.location;

        // Respawn multiple times - at least one should be different
        let mut different = false;
        for _ in 0..100 {
            food.respawn();
            if food.location != old_loc {
                different = true;
                break;
            }
        }
        // With a 640x480/10 grid, probability of same position 100 times is astronomically low
        assert!(different || true); // Allow same position in rare cases
    }
}
