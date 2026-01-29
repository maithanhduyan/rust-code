//! Validation utilities

use std::collections::HashSet;

/// Kết quả validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    /// Tạo kết quả valid
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: vec![],
        }
    }

    /// Tạo kết quả invalid
    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
        }
    }
}

/// Validate email format (đơn giản)
pub fn validate_email(email: &str) -> bool {
    let email = email.trim();
    
    if email.is_empty() {
        return false;
    }
    
    // Kiểm tra có @ và ít nhất một ký tự trước và sau @
    if let Some(at_pos) = email.find('@') {
        let (local, domain) = email.split_at(at_pos);
        let domain = &domain[1..]; // Bỏ @
        
        !local.is_empty() 
            && !domain.is_empty() 
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    } else {
        false
    }
}

/// Validate password strength
pub fn validate_password(password: &str, min_length: usize) -> ValidationResult {
    let mut errors = vec![];
    
    if password.len() < min_length {
        errors.push(format!("Password must be at least {} characters", min_length));
    }
    
    if !password.chars().any(|c| c.is_uppercase()) {
        errors.push("Password must contain at least one uppercase letter".to_string());
    }
    
    if !password.chars().any(|c| c.is_lowercase()) {
        errors.push("Password must contain at least one lowercase letter".to_string());
    }
    
    if !password.chars().any(|c| c.is_numeric()) {
        errors.push("Password must contain at least one digit".to_string());
    }
    
    if errors.is_empty() {
        ValidationResult::valid()
    } else {
        ValidationResult::invalid(errors)
    }
}

/// Validate username
pub fn validate_username(username: &str) -> ValidationResult {
    let mut errors = vec![];
    let reserved: HashSet<&str> = ["admin", "root", "system", "null"].into_iter().collect();
    
    if username.len() < 3 {
        errors.push("Username must be at least 3 characters".to_string());
    }
    
    if username.len() > 30 {
        errors.push("Username must be at most 30 characters".to_string());
    }
    
    if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
        errors.push("Username can only contain letters, numbers, and underscores".to_string());
    }
    
    if reserved.contains(username.to_lowercase().as_str()) {
        errors.push("This username is reserved".to_string());
    }
    
    if errors.is_empty() {
        ValidationResult::valid()
    } else {
        ValidationResult::invalid(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email() {
        assert!(validate_email("test@example.com"));
        assert!(validate_email("user.name@domain.co.uk"));
        assert!(!validate_email("invalid"));
        assert!(!validate_email("@domain.com"));
        assert!(!validate_email("user@"));
    }

    #[test]
    fn test_validate_password() {
        let result = validate_password("Abc12345", 8);
        assert!(result.is_valid);
        
        let result = validate_password("weak", 8);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validate_username() {
        let result = validate_username("john_doe");
        assert!(result.is_valid);
        
        let result = validate_username("admin");
        assert!(!result.is_valid);
    }
}
