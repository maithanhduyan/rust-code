//! Configuration loader with hot reload support

use arc_swap::ArcSwap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

use crate::types::ApexConfig;

/// Configuration loading errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// File not found
    #[error("config file not found: {0}")]
    NotFound(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parse error
    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),

    /// Validation error
    #[error("validation error: {0}")]
    Validation(String),
}

/// Configuration loader with hot reload support
pub struct ConfigLoader {
    /// Current configuration (lock-free swappable)
    config: ArcSwap<ApexConfig>,

    /// Path to config file (for reload)
    config_path: Option<std::path::PathBuf>,
}

impl ConfigLoader {
    /// Create loader with default configuration
    pub fn new() -> Self {
        Self {
            config: ArcSwap::from_pointee(ApexConfig::default()),
            config_path: None,
        }
    }

    /// Load configuration from file
    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }

        let content = std::fs::read_to_string(path)?;
        let config: ApexConfig = toml::from_str(&content)?;

        // Validate configuration
        Self::validate(&config)?;

        Ok(Self {
            config: ArcSwap::from_pointee(config),
            config_path: Some(path.to_path_buf()),
        })
    }

    /// Load configuration from string
    pub fn load_str(content: &str) -> Result<Self, ConfigError> {
        let config: ApexConfig = toml::from_str(content)?;
        Self::validate(&config)?;

        Ok(Self {
            config: ArcSwap::from_pointee(config),
            config_path: None,
        })
    }

    /// Get current configuration (lock-free)
    #[inline]
    pub fn get(&self) -> Arc<ApexConfig> {
        self.config.load_full()
    }

    /// Reload configuration from file
    /// 
    /// # Performance
    /// Lock-free swap, O(1) for readers
    pub fn reload(&self) -> Result<(), ConfigError> {
        let path = self.config_path.as_ref().ok_or_else(|| {
            ConfigError::Validation("no config file path set".to_string())
        })?;

        let content = std::fs::read_to_string(path)?;
        let new_config: ApexConfig = toml::from_str(&content)?;

        Self::validate(&new_config)?;

        // Atomic swap - existing readers continue with old config
        self.config.store(Arc::new(new_config));

        tracing::info!("Configuration reloaded successfully");
        Ok(())
    }

    /// Update configuration programmatically
    pub fn update(&self, new_config: ApexConfig) -> Result<(), ConfigError> {
        Self::validate(&new_config)?;
        self.config.store(Arc::new(new_config));
        Ok(())
    }

    /// Validate configuration
    fn validate(config: &ApexConfig) -> Result<(), ConfigError> {
        // Validate routes
        for route in &config.routes {
            if route.backends.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "route '{}' has no backends",
                    route.name
                )));
            }

            for backend in &route.backends {
                // Basic URL validation
                if backend.url.is_empty() {
                    return Err(ConfigError::Validation(format!(
                        "route '{}' has backend with empty URL",
                        route.name
                    )));
                }
            }
        }

        Ok(())
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_string() {
        let config_str = r#"
[server]
listen = "127.0.0.1:8080"

[[routes]]
name = "test"
backends = [{ url = "http://localhost:9000" }]
"#;

        let loader = ConfigLoader::load_str(config_str).unwrap();
        let config = loader.get();

        assert_eq!(config.server.listen.port(), 8080);
        assert_eq!(config.routes.len(), 1);
    }

    #[test]
    fn test_validation_empty_backends() {
        let config_str = r#"
[[routes]]
name = "test"
backends = []
"#;

        let result = ConfigLoader::load_str(config_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_hot_reload() {
        let config_str = r#"
[[routes]]
name = "v1"
backends = [{ url = "http://localhost:9000" }]
"#;

        let loader = ConfigLoader::load_str(config_str).unwrap();
        
        // Get initial config
        let config1 = loader.get();
        assert_eq!(config1.routes[0].name, "v1");

        // Update config
        let mut new_config = (*config1).clone();
        new_config.routes[0].name = "v2".to_string();
        loader.update(new_config).unwrap();

        // Get updated config
        let config2 = loader.get();
        assert_eq!(config2.routes[0].name, "v2");
    }
}
