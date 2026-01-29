//! Configuration utilities

use serde::{Deserialize, Serialize};
use std::env;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app_name: String,
    pub host: String,
    pub port: u16,
    pub debug: bool,
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: "rust-app".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            debug: false,
            log_level: "info".to_string(),
        }
    }
}

impl AppConfig {
    /// Load config từ environment variables
    pub fn from_env() -> Self {
        Self {
            app_name: env::var("APP_NAME").unwrap_or_else(|_| "rust-app".to_string()),
            host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            debug: env::var("DEBUG")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
        }
    }

    /// Lấy địa chỉ bind đầy đủ
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.port, 8080);
        assert!(!config.debug);
    }

    #[test]
    fn test_bind_address() {
        let config = AppConfig::default();
        assert_eq!(config.bind_address(), "127.0.0.1:8080");
    }
}
