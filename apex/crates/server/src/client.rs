//! HTTP client for backend connections - optimized for streaming

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::{Request, Response};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::time::Duration;

use apex_core::Backend;

/// Error type for client operations
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Connection failed
    #[error("connection failed: {0}")]
    Connection(String),

    /// Request failed
    #[error("request failed: {0}")]
    Request(String),

    /// Timeout
    #[error("timeout")]
    Timeout,
}

/// HTTP client for proxying requests to backends
#[derive(Clone)]
pub struct HttpClient {
    /// Client for Full<Bytes> body (used for buffered requests)
    client_full: Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>,
    /// Client for Incoming body (streaming)
    client_incoming: Client<hyper_util::client::legacy::connect::HttpConnector, Incoming>,
    timeout: Duration,
}

impl HttpClient {
    /// Create a new HTTP client
    pub fn new(timeout: Duration) -> Self {
        let mut connector = hyper_util::client::legacy::connect::HttpConnector::new();
        connector.set_nodelay(true);
        connector.set_keepalive(Some(Duration::from_secs(60)));

        let client_full = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(1024)
            .build(connector.clone());

        let client_incoming = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(1024)
            .build(connector);

        Self { client_full, client_incoming, timeout }
    }

    /// Forward a request with streaming body (zero-copy)
    #[inline]
    pub async fn forward_streaming(
        &self,
        backend: &Backend,
        req: Request<Incoming>,
    ) -> Result<Response<Incoming>, ClientError> {
        // Track connection
        backend.inc_connections();

        // Send request with timeout
        let result = tokio::time::timeout(self.timeout, self.client_incoming.request(req))
            .await
            .map_err(|_| ClientError::Timeout)?
            .map_err(|e| ClientError::Connection(e.to_string()));

        // Always decrement connection count
        backend.dec_connections();

        match result {
            Ok(response) => {
                // Track successful request
                backend.inc_requests();
                Ok(response)
            }
            Err(e) => Err(e),
        }
    }

    /// Forward a request with buffered body
    pub async fn forward(
        &self,
        backend: &Backend,
        mut req: Request<Full<Bytes>>,
    ) -> Result<Response<Incoming>, ClientError> {
        backend.inc_connections();

        let uri = format!(
            "http://{}{}",
            backend.addr,
            req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
        );

        *req.uri_mut() = uri.parse().map_err(|e| {
            ClientError::Request(format!("invalid URI: {}", e))
        })?;

        let result = tokio::time::timeout(self.timeout, self.client_full.request(req))
            .await
            .map_err(|_| ClientError::Timeout)?
            .map_err(|e| ClientError::Connection(e.to_string()));

        backend.dec_connections();

        match result {
            Ok(response) => {
                backend.inc_requests();
                Ok(response)
            }
            Err(e) => Err(e),
        }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}
