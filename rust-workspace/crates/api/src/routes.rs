//! API Routes

use axum::{
    routing::{get, post, delete},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::state::AppState;

pub fn create_router() -> Router {
    let state = AppState::new();
    
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        
        // User routes
        .route("/api/users", get(handlers::list_users))
        .route("/api/users", post(handlers::create_user))
        .route("/api/users/:id", get(handlers::get_user))
        .route("/api/users/:id", delete(handlers::delete_user))
        
        // Item routes
        .route("/api/items", post(handlers::create_item))
        .route("/api/items/:id", get(handlers::get_item))
        .route("/api/users/:user_id/items", get(handlers::list_user_items))
        
        // Middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
