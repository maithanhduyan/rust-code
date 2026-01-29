//! Backend representation and connection pooling
//!
//! Uses lock-free data structures for hot path performance.

use arc_swap::ArcSwap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// A single backend server
#[derive(Debug)]
pub struct Backend {
    /// Backend address
    pub addr: SocketAddr,

    /// Pre-computed URI base: "http://host:port"
    pub uri_base: String,

    /// Pre-computed authority string for URI building
    pub authority: String,

    /// Whether this backend is healthy
    healthy: AtomicBool,

    /// Active connection count
    active_connections: AtomicU64,

    /// Total requests served
    total_requests: AtomicU64,
}

impl Backend {
    /// Create a new backend
    pub fn new(addr: SocketAddr) -> Self {
        let uri_base = format!("http://{}", addr);
        let authority = addr.to_string();
        Self {
            addr,
            uri_base,
            authority,
            healthy: AtomicBool::new(true),
            active_connections: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
        }
    }

    /// Check if backend is healthy
    #[inline]
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    /// Set backend health status
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Relaxed);
    }

    /// Get active connection count
    #[inline]
    pub fn active_connections(&self) -> u64 {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Increment active connections (called when starting request)
    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connections (called when request completes)
    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Increment total requests counter
    pub fn inc_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total requests served
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }
}

/// A pool of backends with lock-free selection
#[derive(Debug)]
pub struct BackendPool {
    /// List of backends (lock-free swappable)
    backends: ArcSwap<Vec<Arc<Backend>>>,

    /// Round-robin counter
    next_idx: AtomicU64,
}

impl BackendPool {
    /// Create a new empty backend pool
    pub fn new() -> Self {
        Self {
            backends: ArcSwap::from_pointee(Vec::new()),
            next_idx: AtomicU64::new(0),
        }
    }

    /// Create pool from list of backends
    pub fn from_backends(backends: Vec<Arc<Backend>>) -> Self {
        Self {
            backends: ArcSwap::from_pointee(backends),
            next_idx: AtomicU64::new(0),
        }
    }

    /// Get next healthy backend using round-robin
    /// 
    /// # Performance
    /// O(n) worst case where n = number of backends
    /// Lock-free, uses atomic operations only
    #[inline]
    pub fn next_healthy(&self) -> Option<Arc<Backend>> {
        let backends = self.backends.load();
        let len = backends.len();

        if len == 0 {
            return None;
        }

        // Try each backend once
        for _ in 0..len {
            let idx = self.next_idx.fetch_add(1, Ordering::Relaxed) as usize % len;
            let backend = &backends[idx];

            if backend.is_healthy() {
                return Some(Arc::clone(backend));
            }
        }

        None
    }

    /// Get backend with least connections (for load balancing)
    /// 
    /// # Performance
    /// O(n) where n = number of backends
    #[inline]
    pub fn least_connections(&self) -> Option<Arc<Backend>> {
        let backends = self.backends.load();

        backends
            .iter()
            .filter(|b| b.is_healthy())
            .min_by_key(|b| b.active_connections())
            .cloned()
    }

    /// Update the backend list atomically (for hot reload)
    /// 
    /// # Performance
    /// Lock-free swap, O(1)
    pub fn update(&self, new_backends: Vec<Arc<Backend>>) {
        self.backends.store(Arc::new(new_backends));
    }

    /// Get current backend count
    pub fn len(&self) -> usize {
        self.backends.load().len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get all backends (for health checking)
    pub fn all(&self) -> Arc<Vec<Arc<Backend>>> {
        self.backends.load_full()
    }
}

impl Default for BackendPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_health() {
        let backend = Backend::new("127.0.0.1:8080".parse().unwrap());
        assert!(backend.is_healthy());

        backend.set_healthy(false);
        assert!(!backend.is_healthy());
    }

    #[test]
    fn test_round_robin() {
        let backends = vec![
            Arc::new(Backend::new("127.0.0.1:8001".parse().unwrap())),
            Arc::new(Backend::new("127.0.0.1:8002".parse().unwrap())),
            Arc::new(Backend::new("127.0.0.1:8003".parse().unwrap())),
        ];

        let pool = BackendPool::from_backends(backends);

        // Should cycle through backends
        let b1 = pool.next_healthy().unwrap();
        let b2 = pool.next_healthy().unwrap();
        let b3 = pool.next_healthy().unwrap();
        let b4 = pool.next_healthy().unwrap();

        assert_eq!(b1.addr.port(), 8001);
        assert_eq!(b2.addr.port(), 8002);
        assert_eq!(b3.addr.port(), 8003);
        assert_eq!(b4.addr.port(), 8001); // Wraps around
    }

    #[test]
    fn test_skip_unhealthy() {
        let backends = vec![
            Arc::new(Backend::new("127.0.0.1:8001".parse().unwrap())),
            Arc::new(Backend::new("127.0.0.1:8002".parse().unwrap())),
        ];

        backends[0].set_healthy(false);

        let pool = BackendPool::from_backends(backends);

        // Should always return the healthy one
        for _ in 0..10 {
            let b = pool.next_healthy().unwrap();
            assert_eq!(b.addr.port(), 8002);
        }
    }
}
