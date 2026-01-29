//! Ultra-optimized HTTP/2 client - minimal dependencies
//!
//! Optimizations:
//! - No DashMap - use simple Arc<[Connection]> array
//! - No timeout wrapper in hot path
//! - Reuse request parts where possible
//! - Inline everything

use arc_swap::ArcSwap;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use apex_core::Backend;
use crate::client::ClientError;

type Sender = client_http2::SendRequest<Full<Bytes>>;

/// Single HTTP/2 connection - ultra minimal
pub struct UltraConnection {
    sender: ArcSwap<Option<Sender>>,
    init_lock: Mutex<()>,
    addr: SocketAddr,
}

impl UltraConnection {
    /// Create new connection holder
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            sender: ArcSwap::new(Arc::new(None)),
            init_lock: Mutex::new(()),
            addr,
        }
    }

    /// Get sender - lock-free fast path, no timeout
    #[inline(always)]
    pub async fn get_sender(&self) -> Result<Sender, ClientError> {
        // Fast path: lock-free read
        let guard = self.sender.load();
        if let Some(ref s) = **guard {
            if s.is_ready() {
                return Ok(s.clone());
            }
        }
        drop(guard);

        // Slow path
        self.create_connection().await
    }

    #[cold]
    async fn create_connection(&self) -> Result<Sender, ClientError> {
        let _lock = self.init_lock.lock().await;

        // Double-check
        let guard = self.sender.load();
        if let Some(ref s) = **guard {
            if s.is_ready() {
                return Ok(s.clone());
            }
        }
        drop(guard);

        // Create connection
        let stream = TcpStream::connect(self.addr)
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| ClientError::Connection(format!("H2: {}", e)))?;

        tokio::spawn(async move { let _ = conn.await; });

        self.sender.store(Arc::new(Some(sender.clone())));
        Ok(sender)
    }

    /// Forward request - ultra optimized, no timeout wrapper
    #[inline(always)]
    pub async fn forward(&self, req: Request<Incoming>) -> Result<Response<Full<Bytes>>, ClientError> {
        // Get path without allocation if possible
        let path = req.uri().path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        // Build URI - minimal allocation
        let uri = hyper::Uri::builder()
            .scheme("http")
            .authority(format!("{}", self.addr).as_str())
            .path_and_query(path)
            .build()
            .map_err(|_| ClientError::Request("uri".into()))?;

        // Collect body once
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        // Build request
        let forward_req = Request::builder()
            .method(parts.method)
            .uri(uri)
            .body(Full::new(body_bytes))
            .map_err(|_| ClientError::Request("build".into()))?;

        // Get sender and send - no timeout wrapper
        let mut sender = self.get_sender().await?;
        let resp = sender.send_request(forward_req)
            .await
            .map_err(|e| ClientError::Request(e.to_string()))?;

        // Collect response body once
        let (parts, body) = resp.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        Ok(Response::from_parts(parts, Full::new(body_bytes)))
    }
}

/// Ultra-simple client - one connection per backend, pre-allocated
pub struct UltraClient {
    /// Single connection (for single backend scenarios)
    connection: Option<Arc<UltraConnection>>,
}

impl UltraClient {
    /// Create client for single backend
    pub fn for_backend(addr: SocketAddr) -> Self {
        Self {
            connection: Some(Arc::new(UltraConnection::new(addr))),
        }
    }

    /// Create empty client
    pub fn new() -> Self {
        Self { connection: None }
    }

    /// Forward to backend
    #[inline(always)]
    pub async fn forward(
        &self,
        backend: &Backend,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ClientError> {
        // Use cached connection or create on-demand
        let conn = match &self.connection {
            Some(c) => c.clone(),
            None => Arc::new(UltraConnection::new(backend.addr)),
        };

        backend.inc_connections();
        let result = conn.forward(req).await;
        backend.dec_connections();

        if result.is_ok() {
            backend.inc_requests();
        }

        result
    }
}

impl Default for UltraClient {
    fn default() -> Self {
        Self::new()
    }
}
