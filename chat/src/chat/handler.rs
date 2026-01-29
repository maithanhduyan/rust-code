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

use super::state::{ChatState, Client};

/// Sanitize HTML to prevent XSS attacks
/// Uses the ammonia crate to clean potentially dangerous HTML
fn sanitize_html(input: &str) -> String {
    ammonia::clean(input)
}

/// WebSocket upgrade handler
/// This is the endpoint that clients connect to
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<ChatState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection
/// This function manages the entire lifecycle of a client connection
async fn handle_socket(socket: WebSocket, state: ChatState) {
    let (mut sender, mut receiver) = socket.split();

    // Create channel for outgoing messages
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Generate unique ID and nickname for this client
    let client_id = Uuid::new_v4();
    let nickname = state.generate_nickname();

    tracing::info!("{} connected (id: {})", nickname, client_id);

    // Create and add client to state
    let client = Client::new(nickname.clone(), tx);
    state.add_client(client_id, client).await;

    // Broadcast join message to all clients
    let join_msg = format!("* {} has joined.", nickname);
    state.broadcast(&join_msg).await;

    // Spawn a task to forward messages from the channel to the WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from this client
    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                // Sanitize the message to prevent XSS
                let sanitized = sanitize_html(&text);
                if !sanitized.is_empty() {
                    let chat_msg = format!("{}: {}", nickname, sanitized);
                    tracing::debug!("{}", chat_msg);
                    state.broadcast(&chat_msg).await;
                }
            }
            Ok(Message::Close(_)) => {
                tracing::info!("{} sent close frame", nickname);
                break;
            }
            Ok(Message::Ping(data)) => {
                // Ping/Pong is handled automatically by axum
                tracing::trace!("{} ping: {:?}", nickname, data);
            }
            Ok(Message::Pong(_)) => {
                // Ignore pong messages
            }
            Ok(Message::Binary(_)) => {
                // Ignore binary messages in chat
                tracing::warn!("{} sent binary message (ignored)", nickname);
            }
            Err(e) => {
                tracing::warn!("WebSocket error for {}: {}", nickname, e);
                break;
            }
        }
    }

    // Cleanup: remove client from state and notify others
    if state.remove_client(&client_id).await.is_some() {
        let disconnect_msg = format!("* {} has disconnected.", nickname);
        state.broadcast(&disconnect_msg).await;
    }

    // Abort the send task
    send_task.abort();
    tracing::info!("{} disconnected", nickname);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_html_script() {
        let input = "<script>alert('xss')</script>";
        let output = sanitize_html(input);
        assert!(!output.contains("<script>"));
        assert!(!output.contains("</script>"));
    }

    #[test]
    fn test_sanitize_html_preserves_text() {
        let input = "Hello, world!";
        let output = sanitize_html(input);
        assert_eq!(output, "Hello, world!");
    }

    #[test]
    fn test_sanitize_html_removes_onclick() {
        let input = "<div onclick='alert(1)'>Click me</div>";
        let output = sanitize_html(input);
        assert!(!output.contains("onclick"));
        assert!(output.contains("Click me"));
    }

    #[test]
    fn test_sanitize_html_allows_safe_tags() {
        let input = "<strong>Bold</strong> and <em>italic</em>";
        let output = sanitize_html(input);
        assert!(output.contains("<strong>"));
        assert!(output.contains("<em>"));
    }

    #[test]
    fn test_sanitize_html_removes_iframe() {
        let input = "<iframe src='http://evil.com'></iframe>";
        let output = sanitize_html(input);
        assert!(!output.contains("<iframe"));
    }
}
