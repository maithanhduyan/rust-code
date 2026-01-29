//! Ultra-optimized HTTP/2 client for single backend
//!
//! Minimal overhead version - no DashMap, no timeout wrapper in hot path

use arc_swap::ArcSwap;
use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full};
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

// Use Empty for GET requests (no allocation)
type EmptySender = client_http2::SendRequest<Empty<Bytes>>;

/// Ultra-fast HTTP/2 connection for single backend
pub struct UltraHttp2Client {
    sender: ArcSwap<Option<EmptySender>>,
    init_lock: Mutex<()>,
    addr: SocketAddr,
    authority: String,
}

impl UltraHttp2Client {
    /// Create new client for single backend
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            sender: ArcSwap::new(Arc::new(None)),
            init_lock: Mutex::new(()),
            authority: format!("{}", addr),
            addr,
        }
    }

    /// Get sender - lock-free fast path
    #[inline(always)]
    async fn get_sender(&self) -> Result<EmptySender, ClientError> {
        // Fast path: lock-free
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
    async fn create_connection(&self) -> Result<EmptySender, ClientError> {
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
            .map_err(|e| ClientError::Connection(e.to_string()))?;

        tokio::spawn(async move { let _ = conn.await; });

        self.sender.store(Arc::new(Some(sender.clone())));
        Ok(sender)
    }

    /// Forward GET request - ultra optimized, no timeout wrapper
    #[inline(always)]
    pub async fn forward_get(
        &self,
        backend: &Backend,
        path: &str,
    ) -> Result<Response<Full<Bytes>>, ClientError> {
        // Build URI
        let uri = hyper::Uri::builder()
            .scheme("http")
            .authority(self.authority.as_str())
            .path_and_query(path)
            .build()
            .map_err(|_| ClientError::Request("uri".into()))?;

        let req = Request::get(uri)
            .body(Empty::new())
            .map_err(|_| ClientError::Request("build".into()))?;

        let mut sender = self.get_sender().await?;
        let resp = sender.send_request(req)
            .await
            .map_err(|e| ClientError::Request(e.to_string()))?;

        backend.inc_requests();

        // Collect response only
        let (parts, body) = resp.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        Ok(Response::from_parts(parts, Full::new(body_bytes)))
    }

    /// Forward incoming request - detect GET for optimization
    #[inline(always)]
    pub async fn forward(
        &self,
        backend: &Backend,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ClientError> {
        // Optimize GET requests (most common)
        if req.method() == hyper::Method::GET || req.method() == hyper::Method::HEAD {
            let path = req.uri().path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/")
                .to_string();
            return self.forward_get(backend, &path).await;
        }

        // Fallback for POST/PUT - use Full body
        self.forward_with_body(backend, req).await
    }

    #[cold]
    async fn forward_with_body(
        &self,
        backend: &Backend,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ClientError> {
        let path = req.uri().path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/")
            .to_string();

        // Need different sender type for Full body - recreate connection
        let stream = TcpStream::connect(self.addr)
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        type FullSender = client_http2::SendRequest<Full<Bytes>>;
        let (mut sender, conn): (FullSender, _) = client_http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;

        tokio::spawn(async move { let _ = conn.await; });

        let uri = hyper::Uri::builder()
            .scheme("http")
            .authority(self.authority.as_str())
            .path_and_query(path.as_str())
            .build()
            .map_err(|_| ClientError::Request("uri".into()))?;

        let (parts, body) = req.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        let forward_req = Request::builder()
            .method(parts.method)
            .uri(uri)
            .body(Full::new(body_bytes))
            .map_err(|_| ClientError::Request("build".into()))?;

        let resp = sender.send_request(forward_req)
            .await
            .map_err(|e| ClientError::Request(e.to_string()))?;

        backend.inc_requests();

        let (parts, body) = resp.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        Ok(Response::from_parts(parts, Full::new(body_bytes)))
    }
}
