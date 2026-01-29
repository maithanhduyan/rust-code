//! HTTP/2 request handler - for high-throughput scenarios
//!
//! Uses HTTP/2 multiplexing for both client and backend connections.

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use apex_config::ApexConfig;

use crate::proxy::{BackendProtocol, ProxyService};

/// HTTP/2 proxy handler - uses HTTP/2 for both client and backend
pub struct Http2Handler {
    /// Proxy service for request handling
    proxy: Arc<ProxyService>,

    /// Server listen address
    listen_addr: SocketAddr,
}

impl Http2Handler {
    /// Create handler from configuration (with HTTP/2 backend)
    pub fn from_config(config: &ApexConfig) -> Self {
        Self {
            proxy: Arc::new(ProxyService::from_config_http2(config)),
            listen_addr: config.server.listen,
        }
    }

    /// Create handler with Ultra mode (maximum performance)
    pub fn from_config_ultra(config: &ApexConfig) -> Self {
        Self {
            proxy: Arc::new(ProxyService::from_config_http2_ultra(config)),
            listen_addr: config.server.listen,
        }
    }

    /// Run the HTTP/2 server
    pub async fn run(self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        tracing::info!("Apex HTTP/2 listening on {}", self.listen_addr);

        let is_ultra = matches!(self.proxy.protocol(), BackendProtocol::Http2Ultra);

        loop {
            let (stream, _remote_addr) = listener.accept().await?;
            stream.set_nodelay(true)?;
            let io = TokioIo::new(stream);
            let proxy = Arc::clone(&self.proxy);

            if is_ultra {
                tokio::spawn(async move {
                    let service = service_fn(|req: Request<Incoming>| {
                        let proxy = Arc::clone(&proxy);
                        async move { handle_request_ultra(proxy, req).await }
                    });

                    let mut builder = http2::Builder::new(TokioExecutor::new());
                    builder.max_concurrent_streams(10000);
                    builder.initial_stream_window_size(1024 * 1024);
                    builder.initial_connection_window_size(2 * 1024 * 1024);

                    if let Err(err) = builder.serve_connection(io, service).await {
                        if !is_connection_closed_error(&err) {
                            tracing::error!("HTTP/2 connection error: {}", err);
                        }
                    }
                });
            } else {
                tokio::spawn(async move {
                    let service = service_fn(|req: Request<Incoming>| {
                        let proxy = Arc::clone(&proxy);
                        async move { handle_request_h2(proxy, req).await }
                    });

                    let mut builder = http2::Builder::new(TokioExecutor::new());
                    builder.max_concurrent_streams(10000);
                    builder.initial_stream_window_size(1024 * 1024);
                    builder.initial_connection_window_size(2 * 1024 * 1024);

                    if let Err(err) = builder.serve_connection(io, service).await {
                        if !is_connection_closed_error(&err) {
                            tracing::error!("HTTP/2 connection error: {}", err);
                        }
                    }
                });
            }
        }
    }
}

/// Handle a single request - standard HTTP/2 mode
#[inline]
async fn handle_request_h2(
    proxy: Arc<ProxyService>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    match proxy.handle_buffered(req).await {
        Ok(resp) => Ok(resp),
        Err(err) => Ok(ProxyService::error_response(&err)),
    }
}

/// Handle a single request - Ultra mode (maximum performance)
#[inline(always)]
async fn handle_request_ultra(
    proxy: Arc<ProxyService>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    match proxy.handle_ultra(req).await {
        Ok(resp) => Ok(resp),
        Err(err) => Ok(ProxyService::error_response(&err)),
    }
}

/// Check if error is just a closed connection
fn is_connection_closed_error<E: std::fmt::Display>(err: &E) -> bool {
    let msg = err.to_string();
    msg.contains("connection closed")
        || msg.contains("broken pipe")
        || msg.contains("reset by peer")
}
