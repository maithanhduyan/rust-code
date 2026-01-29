//! Collision detection logic

use dashmap::DashMap;
use uuid::Uuid;

use super::snake::Snake;

/// Result of a collision check
#[derive(Debug, Clone)]
pub struct CollisionEvent {
    /// ID of the snake that was killed
    pub victim_id: u32,
    /// ID of the snake that killed (if any)
    pub killer_id: Option<u32>,
}

/// Check for collisions between snakes
/// Returns a list of collision events (who killed whom)
pub fn check_collisions(snakes: &DashMap<Uuid, Snake>) -> Vec<CollisionEvent> {
    let mut events = Vec::new();

    // Collect all snake data for collision checking
    let snake_data: Vec<(Uuid, u32, _, Vec<_>)> = snakes
        .iter()
        .filter(|entry| entry.value().is_alive())
        .map(|entry| {
            let snake = entry.value();
            let body: Vec<_> = snake.body().iter().copied().collect();
            (*entry.key(), snake.id, snake.head(), body)
        })
        .collect();

    // Check each snake's head against all other snakes' bodies
    for (uuid_a, id_a, head_a, _) in &snake_data {
        for (uuid_b, id_b, head_b, body_b) in &snake_data {
            if uuid_a == uuid_b {
                // Check self-collision (head hits own body, excluding head)
                let own_body: Vec<_> = snakes
                    .get(uuid_a)
                    .map(|s| s.body().iter().skip(1).copied().collect())
                    .unwrap_or_default();

                if own_body.contains(head_a) {
                    events.push(CollisionEvent {
                        victim_id: *id_a,
                        killer_id: None, // Suicide
                    });
                }
            } else {
                // Check if snake A's head hits snake B
                // Head-to-head collision
                if head_a == head_b {
                    // Both die, no killer
                    events.push(CollisionEvent {
                        victim_id: *id_a,
                        killer_id: None,
                    });
                } else if body_b.contains(head_a) {
                    // Snake A hit snake B's body
                    events.push(CollisionEvent {
                        victim_id: *id_a,
                        killer_id: Some(*id_b),
                    });
                }
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_collisions() {
        let snakes: DashMap<Uuid, Snake> = DashMap::new();
        let snake = Snake::new();
        snakes.insert(Uuid::new_v4(), snake);

        let events = check_collisions(&snakes);
        // New snake with no movement shouldn't collide with itself
        assert!(events.is_empty());
    }
}
