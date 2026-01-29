//! HTTP/2 client with lock-free multiplexed connections
//!
//! Optimizations:
//! - ArcSwap for lock-free sender access
//! - DashMap for lock-free connection pool
//! - Minimal allocations in hot path

use arc_swap::ArcSwap;
use bytes::Bytes;
use dashmap::DashMap;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use apex_core::Backend;
use crate::client::ClientError;

type Sender = client_http2::SendRequest<Full<Bytes>>;

/// Lock-free HTTP/2 connection to a single backend
struct Http2Connection {
    /// ArcSwap for lock-free reads, only lock on reconnect
    sender: ArcSwap<Option<Sender>>,
    /// Mutex only for connection creation (rare)
    init_lock: Mutex<()>,
    addr: SocketAddr,
}

impl Http2Connection {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: ArcSwap::new(Arc::new(None)),
            init_lock: Mutex::new(()),
            addr,
        }
    }

    /// Get sender - lock-free fast path
    #[inline]
    async fn get_sender(&self) -> Result<Sender, ClientError> {
        // Fast path: lock-free read
        {
            let guard = self.sender.load();
            if let Some(ref s) = **guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Slow path: acquire init lock for connection creation
        let _lock = self.init_lock.lock().await;

        // Double-check after lock
        {
            let guard = self.sender.load();
            if let Some(ref s) = **guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Create new connection
        let stream = TcpStream::connect(self.addr)
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| ClientError::Connection(format!("HTTP/2 handshake: {}", e)))?;

        // Spawn connection driver
        tokio::spawn(async move { let _ = conn.await; });

        // Store atomically
        self.sender.store(Arc::new(Some(sender.clone())));
        Ok(sender)
    }
}

/// Lock-free HTTP/2 client pool
pub struct Http2ClientLockFree {
    /// DashMap for lock-free connection lookup
    connections: DashMap<SocketAddr, Arc<Http2Connection>>,
    timeout: Duration,
}

impl Http2ClientLockFree {
    /// Create a new lock-free HTTP/2 client
    pub fn new(timeout: Duration) -> Self {
        Self {
            connections: DashMap::new(),
            timeout,
        }
    }

    /// Get or create connection - lock-free
    #[inline]
    fn get_connection(&self, addr: SocketAddr) -> Arc<Http2Connection> {
        // Fast path: existing connection
        if let Some(conn) = self.connections.get(&addr) {
            return conn.clone();
        }

        // Slow path: create new (DashMap handles concurrent inserts)
        self.connections
            .entry(addr)
            .or_insert_with(|| Arc::new(Http2Connection::new(addr)))
            .clone()
    }

    /// Forward request to backend using HTTP/2 - optimized
    #[inline]
    pub async fn forward(
        &self,
        backend: &Backend,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ClientError> {
        backend.inc_connections();

        let conn = self.get_connection(backend.addr);

        // Build URI without format! allocation
        let path = req.uri().path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        // Use authority from backend (pre-computed)
        let uri = hyper::Uri::builder()
            .scheme("http")
            .authority(backend.authority.as_str())
            .path_and_query(path)
            .build()
            .map_err(|_| ClientError::Request("invalid URI".into()))?;

        // Collect body (required for HTTP/2 framing)
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        let forward_req = Request::builder()
            .method(parts.method)
            .uri(uri)
            .body(Full::new(body_bytes))
            .map_err(|_| ClientError::Request("build request failed".into()))?;

        // Get sender and send request with timeout
        let result = tokio::time::timeout(self.timeout, async {
            let mut sender = conn.get_sender().await?;
            sender.send_request(forward_req)
                .await
                .map_err(|e| ClientError::Request(e.to_string()))
        })
        .await
        .map_err(|_| ClientError::Timeout)?;

        backend.dec_connections();

        match result {
            Ok(resp) => {
                backend.inc_requests();
                let (parts, body) = resp.into_parts();
                let body_bytes = body.collect()
                    .await
                    .map(|b| b.to_bytes())
                    .unwrap_or_default();
                Ok(Response::from_parts(parts, Full::new(body_bytes)))
            }
            Err(e) => Err(e),
        }
    }
}

impl Default for Http2ClientLockFree {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}
