//! WebSocket handler for client connections

use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::game::Snake;
use crate::protocol::{ClientMessage, PlayerInfo, ServerMessage};
use crate::state::AppState;

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let connection_id = Uuid::new_v4();

    // Create a new snake for this connection
    let snake = Snake::new();
    let snake_id = snake.id;

    info!("Player {} connected (snake {})", connection_id, snake_id);

    // Register player with rate limiter
    state.rate_limiter.add_player(connection_id);

    // Log player join
    state.event_logger.log_join(snake_id, &connection_id.to_string());

    // Add snake to the game
    state.snakes.insert(connection_id, snake);

    // Split the socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast messages BEFORE sending join (so we receive future updates)
    let mut broadcast_rx = state.broadcaster.subscribe();

    // Collect all players info
    let all_players: Vec<PlayerInfo> = state
        .snakes
        .iter()
        .map(|entry| PlayerInfo {
            id: entry.value().id,
            color: entry.value().color.clone(),
        })
        .collect();

    // Send join message directly to this client first
    let join_msg = ServerMessage::Join { data: all_players.clone() };
    if sender.send(Message::Text(join_msg.to_json().into())).await.is_err() {
        error!("Failed to send join message to player {}", snake_id);
        state.snakes.remove(&connection_id);
        return;
    }

    // Broadcast join to all OTHER players
    state.broadcaster.send(ServerMessage::Join { data: all_players }).await;

    // Clone for the send task
    let connection_id_clone = connection_id;

    // Task to send broadcast messages to this client
    let send_task = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(msg) => {
                    let json = msg.to_json();
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    warn!("Client {} lagged by {} messages", connection_id_clone, n);
                }
                Err(RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Task to receive messages from this client
    let recv_task = {
        let state = state.clone();
        let connection_id = connection_id;

        tokio::spawn(async move {
            let mut should_kick = false;

            while let Some(result) = receiver.next().await {
                match result {
                    Ok(Message::Text(text)) => {
                        if let Some(msg) = ClientMessage::parse(&text) {
                            match msg {
                                ClientMessage::Direction(dir) => {
                                    // Check rate limit
                                    let (allowed, kick) = state.rate_limiter.check_command(&connection_id);

                                    if kick {
                                        warn!("Player {} kicked for rate limit violations", snake_id);
                                        state.event_logger.log_kick(
                                            snake_id,
                                            &connection_id.to_string(),
                                            "rate_limit_exceeded",
                                        );
                                        should_kick = true;
                                        break;
                                    }

                                    if !allowed {
                                        let violations = state.rate_limiter.get_violations(&connection_id);
                                        warn!("Player {} rate limited (violations: {})", snake_id, violations);
                                        state.event_logger.log_rate_violation(
                                            snake_id,
                                            &connection_id.to_string(),
                                            violations,
                                        );
                                        continue; // Skip this command
                                    }

                                    debug!("Player {} direction: {:?}", snake_id, dir);

                                    // Log direction change
                                    state.event_logger.log_direction(snake_id, dir);

                                    if let Some(mut snake) = state.snakes.get_mut(&connection_id) {
                                        snake.set_direction(dir);
                                    }
                                }
                                ClientMessage::Ping(_) => {
                                    debug!("Ping from player {}", snake_id);
                                    // Ping is just to keep connection alive, no response needed
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("Player {} sent close frame", snake_id);
                        break;
                    }
                    Ok(_) => {
                        // Ignore binary, ping, pong frames
                    }
                    Err(e) => {
                        error!("WebSocket error for player {}: {}", snake_id, e);
                        break;
                    }
                }
            }

            should_kick
        })
    };

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {}
        kicked = recv_task => {
            if kicked.unwrap_or(false) {
                warn!("Player {} was kicked", snake_id);
            }
        }
    }

    // Clean up: remove snake and broadcast leave
    info!("Player {} disconnected (snake {})", connection_id, snake_id);

    // Log player leave
    state.event_logger.log_leave(snake_id, &connection_id.to_string());

    // Remove from rate limiter
    state.rate_limiter.remove_player(&connection_id);

    state.snakes.remove(&connection_id);

    let leave_msg = ServerMessage::Leave { id: snake_id };
    state.broadcaster.send(leave_msg).await;
}
