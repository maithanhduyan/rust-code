use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use drawboard_rs::{room, websocket, AppState};

// Canvas dimensions (same as Java version)
const CANVAS_WIDTH: u32 = 1920;
const CANVAS_HEIGHT: u32 = 1080;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "drawboard=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create shared room state
    let state = AppState::new(CANVAS_WIDTH, CANVAS_HEIGHT);

    // Start broadcast timer
    let broadcast_room = Arc::clone(&state.room);
    tokio::spawn(async move {
        room::broadcaster::start_broadcast_timer(broadcast_room).await;
    });

    // Build router
    let app = Router::new()
        // WebSocket endpoint
        .route("/ws/drawboard", get(websocket::handler::ws_handler))
        // Serve static files
        .nest_service("/", ServeDir::new("static"))
        .with_state(state);

    // Bind to address
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("🎨 Drawboard server running on http://localhost:8080");
    tracing::info!("   Canvas size: {}x{}", CANVAS_WIDTH, CANVAS_HEIGHT);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
