//! Configuration types

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApexConfig {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Route definitions
    #[serde(default)]
    pub routes: Vec<RouteConfig>,
}

impl Default for ApexConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            routes: Vec::new(),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Listen address
    #[serde(default = "default_listen_addr")]
    pub listen: SocketAddr,

    /// Number of worker threads (0 = auto)
    #[serde(default)]
    pub workers: usize,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Maximum connections per backend
    #[serde(default = "default_max_connections")]
    pub max_connections_per_backend: usize,

    /// Enable access logging
    #[serde(default = "default_true")]
    pub access_log: bool,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_listen_addr() -> SocketAddr {
    "0.0.0.0:8080".parse().unwrap()
}

fn default_timeout() -> u64 {
    30
}

fn default_max_connections() -> usize {
    100
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: default_listen_addr(),
            workers: 0,
            timeout_secs: default_timeout(),
            max_connections_per_backend: default_max_connections(),
            access_log: true,
            log_level: default_log_level(),
        }
    }
}

/// Route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Route name for logging
    pub name: String,

    /// Host pattern (e.g., "api.example.com", "*" for any)
    #[serde(default = "default_host")]
    pub host: String,

    /// Path prefix to match
    #[serde(default = "default_path")]
    pub path_prefix: String,

    /// Backend servers
    pub backends: Vec<BackendConfig>,

    /// Strip path prefix before forwarding
    #[serde(default)]
    pub strip_prefix: bool,

    /// Load balancing strategy
    #[serde(default)]
    pub load_balancing: LoadBalancingStrategy,
}

fn default_host() -> String {
    "*".to_string()
}

fn default_path() -> String {
    "/".to_string()
}

/// Load balancing strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalancingStrategy {
    /// Round-robin (default)
    #[default]
    RoundRobin,
    /// Least connections
    LeastConnections,
    /// Random
    Random,
}

/// Backend server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Backend URL or address
    pub url: String,

    /// Weight for weighted load balancing
    #[serde(default = "default_weight")]
    pub weight: u32,

    /// Health check path
    #[serde(default)]
    pub health_check: Option<String>,
}

fn default_weight() -> u32 {
    1
}

/// TLS configuration (for future use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert: PathBuf,

    /// Path to private key file  
    pub key: PathBuf,

    /// Enable ACME/Let's Encrypt
    #[serde(default)]
    pub acme: Option<AcmeConfig>,
}

/// ACME configuration (for future use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcmeConfig {
    /// Email for ACME account
    pub email: String,

    /// Use staging environment
    #[serde(default)]
    pub staging: bool,

    /// Domains to request certificates for
    pub domains: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ApexConfig::default();
        assert_eq!(config.server.listen.port(), 8080);
        assert!(config.routes.is_empty());
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[server]
listen = "127.0.0.1:3000"

[[routes]]
name = "api"
backends = [{ url = "http://localhost:8001" }]
"#;

        let config: ApexConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.server.listen.port(), 3000);
        assert_eq!(config.routes.len(), 1);
        assert_eq!(config.routes[0].name, "api");
    }
}
