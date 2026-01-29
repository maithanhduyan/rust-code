//! Application state

use core::{ItemService, UserService};
use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub item_service: Arc<ItemService>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            user_service: Arc::new(UserService::new()),
            item_service: Arc::new(ItemService::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
