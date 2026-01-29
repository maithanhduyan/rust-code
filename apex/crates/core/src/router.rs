//! Request routing with immutable table + ArcSwap
//!
//! Lock-free routing using simple linear scan.
//! With <100 routes, linear scan is cache-friendly and faster than DashMap.

use arc_swap::ArcSwap;
use std::sync::Arc;

use crate::backend::BackendPool;
use crate::error::{ProxyError, Result};

/// A route entry mapping host/path to backend pool
#[derive(Debug)]
pub struct Route {
    /// Host pattern (e.g., "api.example.com", "*" for any)
    pub host: String,

    /// Path prefix (e.g., "/api/v1")
    pub path_prefix: String,

    /// Backend pool for this route
    pub backends: Arc<BackendPool>,

    /// Strip path prefix before forwarding
    pub strip_prefix: bool,
}

impl Route {
    /// Create a new route
    pub fn new(host: String, path_prefix: String, backends: Arc<BackendPool>) -> Self {
        Self {
            host,
            path_prefix,
            backends,
            strip_prefix: false,
        }
    }

    /// Set strip prefix option
    pub fn with_strip_prefix(mut self, strip: bool) -> Self {
        self.strip_prefix = strip;
        self
    }

    /// Check if this route matches the given host
    #[inline]
    fn matches_host(&self, host: &str) -> bool {
        self.host == "*" || self.host.is_empty() || self.host == host
    }

    /// Check if this route matches the given path
    #[inline]
    fn matches_path(&self, path: &str) -> bool {
        self.path_prefix == "/" || path.starts_with(&self.path_prefix)
    }
}

/// Match result from router lookup
#[derive(Debug, Clone)]
pub struct RouteMatch<'a> {
    /// Matched route
    pub route: Arc<Route>,

    /// Original path (for rewriting by caller if needed)
    pub path: &'a str,
    
    /// Whether to strip prefix
    pub should_strip: bool,
}

/// Immutable routing table
struct RouterTable {
    /// All routes, sorted by specificity (longer path prefix first)
    routes: Vec<Arc<Route>>,
}

impl RouterTable {
    fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Find matching route using linear scan
    /// 
    /// # Performance
    /// O(n) where n = number of routes
    /// Cache-friendly sequential access
    #[inline]
    fn find<'a>(&self, host: &str, path: &'a str) -> Option<RouteMatch<'a>> {
        // Linear scan - cache friendly, fast for <100 routes
        for route in &self.routes {
            if route.matches_host(host) && route.matches_path(path) {
                return Some(RouteMatch {
                    route: Arc::clone(route),
                    path,
                    should_strip: route.strip_prefix && route.path_prefix != "/",
                });
            }
        }
        None
    }
}

/// Router for matching requests to backend pools
/// 
/// Uses ArcSwap for lock-free reads with atomic updates.
#[derive(Debug)]
pub struct Router {
    /// Immutable routing table (swapped atomically)
    table: ArcSwap<Vec<Arc<Route>>>,
}

impl Router {
    /// Create a new empty router
    pub fn new() -> Self {
        Self {
            table: ArcSwap::from_pointee(Vec::new()),
        }
    }

    /// Add a route to the router
    /// 
    /// Note: This clones the entire table. Fine for config reload,
    /// not meant for high-frequency updates.
    pub fn add_route(&self, route: Route) {
        let route = Arc::new(route);
        
        // Clone current table, add route, sort, swap
        let mut routes = (*self.table.load_full()).clone();
        routes.push(route);
        
        // Sort by specificity: longer path prefix = higher priority
        routes.sort_by(|a, b| {
            // First by host specificity ("*" is less specific)
            let host_cmp = match (&a.host == "*", &b.host == "*") {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Equal,
            };
            
            if host_cmp != std::cmp::Ordering::Equal {
                return host_cmp;
            }
            
            // Then by path length (longer = more specific)
            b.path_prefix.len().cmp(&a.path_prefix.len())
        });
        
        self.table.store(Arc::new(routes));
    }

    /// Find matching route for request
    /// 
    /// # Performance
    /// Lock-free read via ArcSwap::load()
    /// O(n) linear scan where n = number of routes
    #[inline]
    pub fn find<'a>(&self, host: &str, path: &'a str) -> Result<RouteMatch<'a>> {
        let routes = self.table.load();
        
        // Linear scan - cache friendly
        for route in routes.iter() {
            if route.matches_host(host) && route.matches_path(path) {
                return Ok(RouteMatch {
                    route: Arc::clone(route),
                    path,
                    should_strip: route.strip_prefix && route.path_prefix != "/",
                });
            }
        }

        Err(ProxyError::RouteNotFound {
            host: host.to_string(),
            path: path.to_string(),
        })
    }

    /// Clear all routes (for hot reload)
    pub fn clear(&self) {
        self.table.store(Arc::new(Vec::new()));
    }

    /// Get total route count
    pub fn route_count(&self) -> usize {
        self.table.load().len()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Backend;
    use std::net::SocketAddr;

    fn make_pool(port: u16) -> Arc<BackendPool> {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let backend = Arc::new(Backend::new(addr));
        Arc::new(BackendPool::from_backends(vec![backend]))
    }

    #[test]
    fn test_basic_routing() {
        let router = Router::new();

        router.add_route(Route::new(
            "api.example.com".to_string(),
            "/".to_string(),
            make_pool(8001),
        ));

        let result = router.find("api.example.com", "/users").unwrap();
        assert_eq!(result.route.host, "api.example.com");
    }

    #[test]
    fn test_path_prefix_matching() {
        let router = Router::new();

        router.add_route(Route::new(
            "example.com".to_string(),
            "/api/v1".to_string(),
            make_pool(8001),
        ));

        router.add_route(Route::new(
            "example.com".to_string(),
            "/api/v2".to_string(),
            make_pool(8002),
        ));

        let v1 = router.find("example.com", "/api/v1/users").unwrap();
        let v2 = router.find("example.com", "/api/v2/users").unwrap();

        assert_eq!(v1.route.path_prefix, "/api/v1");
        assert_eq!(v2.route.path_prefix, "/api/v2");
    }

    #[test]
    fn test_default_routes() {
        let router = Router::new();

        router.add_route(Route::new(
            "*".to_string(),
            "/".to_string(),
            make_pool(8000),
        ));

        // Should match any host
        let result = router.find("unknown.example.com", "/anything").unwrap();
        assert_eq!(result.route.host, "*");
    }

    #[test]
    fn test_route_not_found() {
        let router = Router::new();

        router.add_route(Route::new(
            "api.example.com".to_string(),
            "/".to_string(),
            make_pool(8001),
        ));

        let result = router.find("other.example.com", "/");
        assert!(result.is_err());
    }
}
