//! Proxy service - core request handling logic
//!
//! Optimized for high performance with:
//! - HTTP/2 multiplexing for backend connections (high throughput)
//! - Lock-free connection pool with ArcSwap + DashMap
//! - HTTP/1.1 fallback for compatibility

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use std::sync::Arc;

use apex_config::ApexConfig;
use apex_core::{Backend, BackendPool, ProxyError, Route, Router};

use crate::client::HttpClient;
use crate::http2_client_lockfree::Http2ClientLockFree;
use crate::ultra_http2_client::UltraHttp2Client;

/// Backend protocol mode
#[derive(Clone, Copy, Debug, Default)]
pub enum BackendProtocol {
    /// HTTP/1.1 with connection pooling
    #[default]
    Http1,
    /// HTTP/2 with multiplexing (higher throughput)
    Http2,
    /// HTTP/2 Ultra mode (single backend, maximum throughput)
    Http2Ultra,
}

/// Proxy service that routes requests to backends
pub struct ProxyService {
    /// Router for matching requests
    router: Arc<Router>,

    /// HTTP/1.1 client for backend connections
    http1_client: HttpClient,

    /// HTTP/2 client for backend connections (lock-free)
    http2_client: Http2ClientLockFree,

    /// Ultra HTTP/2 client for single backend
    ultra_client: Option<Arc<UltraHttp2Client>>,

    /// Default backend for ultra mode
    ultra_backend: Option<Arc<Backend>>,

    /// Protocol mode
    protocol: BackendProtocol,
}

impl ProxyService {
    /// Create a new proxy service from configuration
    pub fn from_config(config: &ApexConfig) -> Self {
        Self::from_config_with_protocol(config, BackendProtocol::Http1)
    }

    /// Create a new proxy service with HTTP/2 backend
    pub fn from_config_http2(config: &ApexConfig) -> Self {
        Self::from_config_with_protocol(config, BackendProtocol::Http2)
    }

    /// Create a new proxy service with Ultra HTTP/2 (single backend, max perf)
    pub fn from_config_http2_ultra(config: &ApexConfig) -> Self {
        Self::from_config_with_protocol(config, BackendProtocol::Http2Ultra)
    }

    /// Create a new proxy service with specified protocol
    pub fn from_config_with_protocol(config: &ApexConfig, protocol: BackendProtocol) -> Self {
        let router = Arc::new(Router::new());
        let timeout = std::time::Duration::from_secs(config.server.timeout_secs);
        let http1_client = HttpClient::new(timeout);
        let http2_client = Http2ClientLockFree::new(timeout);

        let mut ultra_client = None;
        let mut ultra_backend = None;

        // Build router from config
        for route_config in &config.routes {
            let backends: Vec<Arc<Backend>> = route_config
                .backends
                .iter()
                .filter_map(|b| {
                    parse_backend_url(&b.url).map(|addr| Arc::new(Backend::new(addr)))
                })
                .collect();

            if backends.is_empty() {
                tracing::warn!("Route '{}' has no valid backends", route_config.name);
                continue;
            }

            // For Ultra mode, use first backend's address
            if matches!(protocol, BackendProtocol::Http2Ultra) && ultra_client.is_none() {
                if let Some(first_backend) = backends.first() {
                    ultra_client = Some(Arc::new(UltraHttp2Client::new(first_backend.addr)));
                    ultra_backend = Some(first_backend.clone());
                    tracing::info!("Ultra mode: using backend {}", first_backend.addr);
                }
            }

            let backend_pool = Arc::new(BackendPool::from_backends(backends));
            let route = Route::new(
                route_config.host.clone(),
                route_config.path_prefix.clone(),
                backend_pool,
            )
            .with_strip_prefix(route_config.strip_prefix);

            router.add_route(route);
            tracing::info!(
                "Added route '{}': {} {} -> {} backends ({:?})",
                route_config.name,
                route_config.host,
                route_config.path_prefix,
                route_config.backends.len(),
                protocol
            );
        }

        Self { router, http1_client, http2_client, ultra_client, ultra_backend, protocol }
    }

    /// Get router reference
    pub fn router(&self) -> &Arc<Router> {
        &self.router
    }

    /// Get HTTP/2 client reference
    pub fn http2_client(&self) -> &Http2ClientLockFree {
        &self.http2_client
    }

    /// Handle an incoming request (HTTP/1.1 mode)
    #[inline]
    pub async fn handle(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<Incoming>, ProxyError> {
        // Extract routing info
        let host = req
            .headers()
            .get(hyper::header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        let path = req.uri().path();

        // Find matching route
        let route_match = self.router.find(host, path)?;

        // Get healthy backend
        let backend = route_match
            .route
            .backends
            .next_healthy()
            .ok_or(ProxyError::NoHealthyBackend)?;

        // Build backend URI
        let effective_path = if route_match.should_strip {
            path.strip_prefix(&route_match.route.path_prefix).unwrap_or(path)
        } else {
            path
        };

        let backend_uri = hyper::Uri::builder()
            .scheme("http")
            .authority(backend.authority.as_str())
            .path_and_query(effective_path)
            .build()
            .map_err(|_| ProxyError::Internal("invalid backend URI".into()))?;

        // Decompose and rebuild request
        let (mut parts, body) = req.into_parts();
        parts.uri = backend_uri;
        parts.headers.remove("connection");

        let forward_req = Request::from_parts(parts, body);

        // Forward using HTTP/1.1
        self.http1_client
            .forward_streaming(&backend, forward_req)
            .await
            .map_err(|e| ProxyError::ConnectionError(e.to_string()))
    }

    /// Handle request and return Full<Bytes> body (for HTTP/2 mode)
    #[inline]
    pub async fn handle_buffered(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ProxyError> {
        // Extract routing info
        let host = req
            .headers()
            .get(hyper::header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        let path = req.uri().path();

        // Find matching route
        let route_match = self.router.find(host, path)?;

        // Get healthy backend
        let backend = route_match
            .route
            .backends
            .next_healthy()
            .ok_or(ProxyError::NoHealthyBackend)?;

        // Forward using HTTP/2
        self.http2_client
            .forward(&backend, req)
            .await
            .map_err(|e| ProxyError::ConnectionError(e.to_string()))
    }

    /// Handle request with Ultra HTTP/2 client (single backend, max performance)
    #[inline(always)]
    pub async fn handle_ultra(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ProxyError> {
        let ultra = self.ultra_client.as_ref()
            .ok_or_else(|| ProxyError::Internal("ultra client not configured".into()))?;
        let backend = self.ultra_backend.as_ref()
            .ok_or_else(|| ProxyError::Internal("ultra backend not configured".into()))?;

        ultra.forward(backend, req)
            .await
            .map_err(|e| ProxyError::ConnectionError(e.to_string()))
    }

    /// Get protocol mode
    pub fn protocol(&self) -> BackendProtocol {
        self.protocol
    }

    /// Build error response
    #[cold]
    pub fn error_response(error: &ProxyError) -> Response<Full<Bytes>> {
        let status = StatusCode::from_u16(error.status_code())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        let body = format!(
            r#"{{"error":"{}","status":{}}}"#,
            error,
            status.as_u16()
        );

        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(body)))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("{}"))))
    }
}

/// Parse backend URL to SocketAddr
#[inline]
fn parse_backend_url(url: &str) -> Option<std::net::SocketAddr> {
    let url = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let host_port = url.split('/').next()?;

    if let Ok(addr) = host_port.parse() {
        return Some(addr);
    }

    if !host_port.contains(':') {
        format!("{}:80", host_port).parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_backend_url() {
        assert_eq!(
            parse_backend_url("http://127.0.0.1:8080"),
            Some("127.0.0.1:8080".parse().unwrap())
        );

        assert_eq!(
            parse_backend_url("127.0.0.1:8080"),
            Some("127.0.0.1:8080".parse().unwrap())
        );

        assert_eq!(
            parse_backend_url("http://127.0.0.1:8080/path"),
            Some("127.0.0.1:8080".parse().unwrap())
        );
    }
}
