//! Game loop - runs every tick to update game state

use std::sync::Arc;
use std::time::Duration;

use tokio::time::interval;
use tracing::{debug, error};

use crate::config::TICK_DELAY_MS;
use crate::protocol::{FoodData, ServerMessage, SnakeData};
use crate::state::AppState;

use super::collision::check_collisions;

/// Spawn the game loop task
pub fn spawn_game_loop(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut tick_interval = interval(Duration::from_millis(TICK_DELAY_MS));

        loop {
            tick_interval.tick().await;

            if let Err(e) = game_tick(&state).await {
                error!("Game tick error: {}", e);
            }
        }
    });
}

/// Process one game tick
async fn game_tick(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Skip if no snakes
    if state.snakes.is_empty() {
        return Ok(());
    }

    // Update all snake positions
    for mut entry in state.snakes.iter_mut() {
        entry.value_mut().update();
    }

    // Check for collisions
    let collision_events = check_collisions(&state.snakes);

    // Process collision events
    for event in collision_events {
        debug!("Collision: victim={}, killer={:?}", event.victim_id, event.killer_id);

        // Log the death event
        let cause = if event.killer_id.is_some() {
            "killed_by_other"
        } else {
            "self_collision"
        };
        state.event_logger.log_death(event.victim_id, event.killer_id, cause);

        // Find and kill the victim
        for mut entry in state.snakes.iter_mut() {
            if entry.value().id == event.victim_id {
                entry.value_mut().kill();

                // Send dead message to victim (via their personal channel if we had one)
                // For now, we'll handle this differently in the ws handler
            }
        }

        // Reward the killer if there is one
        if let Some(killer_id) = event.killer_id {
            for mut entry in state.snakes.iter_mut() {
                if entry.value().id == killer_id {
                    entry.value_mut().reward();
                }
            }
        }
    }

    // Check for food eating - collect data first to avoid deadlock
    let mut food_eaten: Vec<(u32, u32, super::location::Location)> = Vec::new(); // (snake_id, food_id, location)

    for snake_entry in state.snakes.iter() {
        if !snake_entry.value().is_alive() {
            continue;
        }
        let snake_id = snake_entry.value().id;
        let head = snake_entry.value().head();

        // Check each food
        for food_entry in state.foods.iter() {
            if food_entry.value().is_at(&head) {
                food_eaten.push((snake_id, *food_entry.key(), food_entry.value().location));
                break;
            }
        }
    }

    // Now process the food eating (without holding any locks)
    for (snake_id, food_id, location) in food_eaten {
        debug!("Snake {} ate food!", snake_id);

        // Log the food eaten event
        state.event_logger.log_food_eaten(snake_id, food_id, location);

        // Respawn food at new location
        if let Some(mut food) = state.foods.get_mut(&food_id) {
            food.value_mut().respawn();
        }

        // Reward the snake (grow longer)
        for mut snake in state.snakes.iter_mut() {
            if snake.value().id == snake_id {
                snake.value_mut().reward();
                break;
            }
        }
    }

    // Collect all alive snake data for broadcast
    let snake_data: Vec<SnakeData> = state
        .snakes
        .iter()
        .filter(|entry| entry.value().is_alive())
        .map(|entry| entry.value().to_data())
        .collect();

    // Collect all food data for broadcast
    let food_data: Vec<FoodData> = state
        .foods
        .iter()
        .map(|entry| entry.value().to_data())
        .collect();

    // Broadcast update to all clients
    let update_msg = ServerMessage::Update {
        data: snake_data,
        food: food_data,
    };
    state.broadcaster.send(update_msg).await;

    Ok(())
}
