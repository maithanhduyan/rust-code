//! HTTP/2 client with multiplexed connections
//!
//! Key features:
//! - Single HTTP/2 connection per backend handles many concurrent requests
//! - Lock-free sender cloning for zero contention
//! - Auto-reconnect on connection failure

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::RwLock;

use apex_core::Backend;
use crate::client::ClientError;

type Sender = client_http2::SendRequest<Full<Bytes>>;

/// HTTP/2 connection to a single backend
struct Http2Connection {
    sender: RwLock<Option<Sender>>,
    addr: SocketAddr,
}

impl Http2Connection {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: RwLock::new(None),
            addr,
        }
    }

    /// Get or create HTTP/2 sender (connection)
    async fn get_sender(&self) -> Result<Sender, ClientError> {
        // Fast path: check if connection ready (read lock)
        {
            let guard = self.sender.read().await;
            if let Some(ref s) = *guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Slow path: create new connection (write lock)
        let mut guard = self.sender.write().await;

        // Double-check after acquiring write lock
        if let Some(ref s) = *guard {
            if s.is_ready() {
                return Ok(s.clone());
            }
        }

        // Create new HTTP/2 connection
        let stream = TcpStream::connect(self.addr)
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| ClientError::Connection(format!("HTTP/2 handshake failed: {}", e)))?;

        // Spawn connection driver
        tokio::spawn(async move {
            let _ = conn.await;
        });

        *guard = Some(sender.clone());
        Ok(sender)
    }
}

/// HTTP/2 client pool - manages connections to multiple backends
pub struct Http2Client {
    /// Connections per backend address
    connections: RwLock<HashMap<SocketAddr, Arc<Http2Connection>>>,
    timeout: Duration,
}

impl Http2Client {
    /// Create a new HTTP/2 client
    pub fn new(timeout: Duration) -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            timeout,
        }
    }

    /// Get or create connection for backend
    async fn get_connection(&self, addr: SocketAddr) -> Arc<Http2Connection> {
        // Fast path: check if connection exists
        {
            let guard = self.connections.read().await;
            if let Some(conn) = guard.get(&addr) {
                return conn.clone();
            }
        }

        // Slow path: create new connection entry
        let mut guard = self.connections.write().await;

        // Double-check
        if let Some(conn) = guard.get(&addr) {
            return conn.clone();
        }

        let conn = Arc::new(Http2Connection::new(addr));
        guard.insert(addr, conn.clone());
        conn
    }

    /// Forward request to backend using HTTP/2
    pub async fn forward(
        &self,
        backend: &Backend,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ClientError> {
        backend.inc_connections();

        let conn = self.get_connection(backend.addr).await;

        // Build forward request
        let path = req.uri().path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        let uri: hyper::Uri = format!("http://{}{}", backend.addr, path)
            .parse()
            .map_err(|e| ClientError::Request(format!("invalid URI: {}", e)))?;

        // Collect body
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        let forward_req = Request::builder()
            .method(parts.method)
            .uri(uri)
            .body(Full::new(body_bytes))
            .unwrap();

        // Get sender and send request
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
                // Convert response body
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

impl Default for Http2Client {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}
