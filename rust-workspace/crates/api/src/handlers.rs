//! API Handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ============ Response Types ============

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data: Some(data),
            error: None,
        })
    }

    pub fn error(message: impl Into<String>) -> Json<Self> {
        Json(Self {
            success: false,
            data: None,
            error: Some(message.into()),
        })
    }
}

// ============ Request Types ============

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
}

#[derive(Deserialize)]
pub struct CreateItemRequest {
    pub title: String,
    pub owner_id: u64,
    pub description: Option<String>,
}

// ============ Handlers ============

/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": utils::current_timestamp_secs()
    }))
}

/// List all users
pub async fn list_users(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let users = state.user_service.list();
    ApiResponse::success(users)
}

/// Create a new user
pub async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> impl IntoResponse {
    // Validate email
    if !utils::validate_email(&payload.email) {
        return (StatusCode::BAD_REQUEST, ApiResponse::<core::User>::error("Invalid email format"));
    }

    match state.user_service.create(&payload.name, &payload.email) {
        Ok(user) => (StatusCode::CREATED, ApiResponse::success(user)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::error(e.to_string())),
    }
}

/// Get user by ID
pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.user_service.get(id) {
        Ok(user) => (StatusCode::OK, ApiResponse::success(user)),
        Err(e) => (StatusCode::NOT_FOUND, ApiResponse::error(e.to_string())),
    }
}

/// Delete user by ID
pub async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.user_service.delete(id) {
        Ok(_) => (StatusCode::OK, ApiResponse::success("User deleted")),
        Err(e) => (StatusCode::NOT_FOUND, ApiResponse::error(e.to_string())),
    }
}

/// Create a new item
pub async fn create_item(
    State(state): State<AppState>,
    Json(payload): Json<CreateItemRequest>,
) -> impl IntoResponse {
    match state.item_service.create(&payload.title, payload.owner_id) {
        Ok(mut item) => {
            item.description = payload.description;
            (StatusCode::CREATED, ApiResponse::success(item))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::error(e.to_string())),
    }
}

/// Get item by ID
pub async fn get_item(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.item_service.get(id) {
        Ok(item) => (StatusCode::OK, ApiResponse::success(item)),
        Err(e) => (StatusCode::NOT_FOUND, ApiResponse::error(e.to_string())),
    }
}

/// List items by user
pub async fn list_user_items(
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
) -> impl IntoResponse {
    let items = state.item_service.list_by_owner(user_id);
    ApiResponse::success(items)
}
