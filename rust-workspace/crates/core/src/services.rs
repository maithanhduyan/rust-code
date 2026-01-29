//! Business services

use crate::error::{CoreError, Result};
use crate::models::{Item, User};
use std::collections::HashMap;
use std::sync::RwLock;

/// User Service - quản lý users
pub struct UserService {
    users: RwLock<HashMap<u64, User>>,
    next_id: RwLock<u64>,
}

impl Default for UserService {
    fn default() -> Self {
        Self::new()
    }
}

impl UserService {
    /// Tạo service mới
    pub fn new() -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
            next_id: RwLock::new(1),
        }
    }

    /// Tạo user mới
    pub fn create(&self, name: impl Into<String>, email: impl Into<String>) -> Result<User> {
        let mut next_id = self.next_id.write().unwrap();
        let id = *next_id;
        *next_id += 1;

        let user = User::new(id, name, email);
        self.users.write().unwrap().insert(id, user.clone());
        
        Ok(user)
    }

    /// Lấy user theo id
    pub fn get(&self, id: u64) -> Result<User> {
        self.users
            .read()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| CoreError::NotFound(format!("User with id {} not found", id)))
    }

    /// Lấy tất cả users
    pub fn list(&self) -> Vec<User> {
        self.users.read().unwrap().values().cloned().collect()
    }

    /// Xóa user
    pub fn delete(&self, id: u64) -> Result<()> {
        self.users
            .write()
            .unwrap()
            .remove(&id)
            .map(|_| ())
            .ok_or_else(|| CoreError::NotFound(format!("User with id {} not found", id)))
    }
}

/// Item Service - quản lý items
pub struct ItemService {
    items: RwLock<HashMap<u64, Item>>,
    next_id: RwLock<u64>,
}

impl Default for ItemService {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemService {
    /// Tạo service mới
    pub fn new() -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
            next_id: RwLock::new(1),
        }
    }

    /// Tạo item mới
    pub fn create(&self, title: impl Into<String>, owner_id: u64) -> Result<Item> {
        let mut next_id = self.next_id.write().unwrap();
        let id = *next_id;
        *next_id += 1;

        let item = Item::new(id, title, owner_id);
        self.items.write().unwrap().insert(id, item.clone());
        
        Ok(item)
    }

    /// Lấy item theo id
    pub fn get(&self, id: u64) -> Result<Item> {
        self.items
            .read()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| CoreError::NotFound(format!("Item with id {} not found", id)))
    }

    /// Lấy items của một user
    pub fn list_by_owner(&self, owner_id: u64) -> Vec<Item> {
        self.items
            .read()
            .unwrap()
            .values()
            .filter(|item| item.owner_id == owner_id)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_service_crud() {
        let service = UserService::new();
        
        // Create
        let user = service.create("Alice", "alice@example.com").unwrap();
        assert_eq!(user.id, 1);
        
        // Read
        let fetched = service.get(1).unwrap();
        assert_eq!(fetched.name, "Alice");
        
        // List
        let users = service.list();
        assert_eq!(users.len(), 1);
        
        // Delete
        service.delete(1).unwrap();
        assert!(service.get(1).is_err());
    }

    #[test]
    fn test_item_service() {
        let service = ItemService::new();
        
        let item = service.create("My Item", 1).unwrap();
        assert_eq!(item.owner_id, 1);
        
        let items = service.list_by_owner(1);
        assert_eq!(items.len(), 1);
    }
}
