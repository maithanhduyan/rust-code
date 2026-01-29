use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::drawing::DrawMessage;
use crate::room::Player;
use crate::websocket::message::{ClientMessage, ServerMessage};
use crate::AppState;

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Create channel for outgoing messages
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Generate player ID
    let player_id = Uuid::new_v4();

    // Try to add player to room
    let join_result = {
        let mut room = state.room.write().await;

        if room.is_full() {
            Err("Maximum player count reached")
        } else {
            // Notify existing players about new player
            room.broadcast_message("3+");

            // Add new player
            let player = Player::new(tx.clone());
            room.add_player(player_id, player);

            // Get current state
            let count = room.player_count();
            let image = room.get_canvas_png();

            Ok((count, image))
        }
    };

    // Handle join result
    let (player_count, image_data) = match join_result {
        Ok(data) => data,
        Err(msg) => {
            let error_msg = ServerMessage::Error(msg.to_string()).to_ws_message();
            let _ = sender.send(error_msg).await;
            let _ = sender.close().await;
            return;
        }
    };

    tracing::info!("Player {} joined. Total players: {}", player_id, player_count);

    // Send initial state: player count
    let image_msg = ServerMessage::ImageMessage(player_count).to_ws_message();
    if sender.send(image_msg).await.is_err() {
        cleanup_player(&state, player_id).await;
        return;
    }

    // Send initial state: PNG image
    if sender.send(Message::Binary(image_data)).await.is_err() {
        cleanup_player(&state, player_id).await;
        return;
    }

    // Spawn task for sending outgoing messages
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                handle_text_message(&state, player_id, &text).await;
            }
            Ok(Message::Close(_)) => {
                tracing::info!("Player {} disconnected", player_id);
                break;
            }
            Ok(_) => {
                // Ignore other message types (binary, ping, pong)
            }
            Err(e) => {
                tracing::warn!("WebSocket error for player {}: {}", player_id, e);
                break;
            }
        }
    }

    // Cleanup: remove player
    cleanup_player(&state, player_id).await;

    // Abort the send task
    send_task.abort();
}

/// Handle a text message from a client
async fn handle_text_message(state: &AppState, player_id: Uuid, text: &str) {
    match ClientMessage::parse(text) {
        Some(ClientMessage::Pong) => {
            // Keepalive - no action needed
        }
        Some(ClientMessage::Draw { msg_id, data }) => {
            match DrawMessage::parse(&data) {
                Ok(draw_msg) => {
                    let mut room = state.room.write().await;
                    room.handle_draw_message(player_id, draw_msg, msg_id);
                }
                Err(e) => {
                    tracing::warn!("Invalid draw message from {}: {}", player_id, e);
                }
            }
        }
        None => {
            tracing::warn!("Unknown message from {}: {}", player_id, text);
        }
    }
}

/// Remove a player from the room and notify others
async fn cleanup_player(state: &AppState, player_id: Uuid) {
    let mut room = state.room.write().await;
    if room.remove_player(&player_id).is_some() {
        // Notify remaining players
        room.broadcast_message("3-");
        tracing::info!(
            "Player {} removed. Remaining players: {}",
            player_id,
            room.player_count()
        );
    }
}
