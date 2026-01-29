//! Models/Entities của ứng dụng

use serde::{Deserialize, Serialize};

/// User model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub active: bool,
}

impl User {
    /// Tạo user mới
    pub fn new(id: u64, name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            email: email.into(),
            active: true,
        }
    }
}

/// Item model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub owner_id: u64,
}

impl Item {
    /// Tạo item mới
    pub fn new(id: u64, title: impl Into<String>, owner_id: u64) -> Self {
        Self {
            id,
            title: title.into(),
            description: None,
            owner_id,
        }
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user() {
        let user = User::new(1, "John Doe", "john@example.com");
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "John Doe");
        assert!(user.active);
    }

    #[test]
    fn test_create_item() {
        let item = Item::new(1, "Test Item", 1)
            .with_description("This is a test item");
        
        assert_eq!(item.title, "Test Item");
        assert_eq!(item.description, Some("This is a test item".to_string()));
    }
}
