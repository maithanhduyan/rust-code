//! Snake Game WebSocket Server
//!
//! A multiplayer snake game using WebSocket for real-time communication.

use std::sync::Arc;

use axum::{
    Router,
    routing::get,
    response::IntoResponse,
    http::StatusCode,
};
use rust_embed::Embed;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod broadcast;
mod config;
mod event_logger;
mod game;
mod protocol;
mod rate_limiter;
mod state;
mod ws;

use config::SERVER_PORT;
use game::game_loop::spawn_game_loop;
use state::AppState;
use ws::ws_handler;

/// Embedded static files
#[derive(Embed)]
#[folder = "static/"]
struct Assets;

/// Serve embedded static files
async fn serve_static(path: &str) -> impl IntoResponse {
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [("content-type", mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
    }
}

/// Index page handler
async fn index_handler() -> impl IntoResponse {
    serve_static("index.html").await
}

/// Static file handler
async fn static_handler(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    serve_static(&path).await
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "snake_rs=debug,tower_http=debug".into()),
        )
        .init();

    // Create shared state
    let state = Arc::new(AppState::new());

    // Spawn the game loop
    spawn_game_loop(state.clone());
    info!("Game loop started (tick every {}ms)", config::TICK_DELAY_MS);

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws/snake", get(ws_handler))
        .route("/health", get(health_handler))
        .route("/{*path}", get(static_handler))
        .layer(cors)
        .with_state(state);

    // Start the server
    let addr = format!("0.0.0.0:{}", SERVER_PORT);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    info!("🐍 Snake server running on http://{}", addr);
    info!("   WebSocket endpoint: ws://localhost:{}/ws/snake", SERVER_PORT);

    axum::serve(listener, app).await.unwrap();
}
