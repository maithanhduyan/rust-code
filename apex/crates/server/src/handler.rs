//! Request handler - hyper service implementation
//!
//! Optimized for high throughput with streaming responses.

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use apex_config::ApexConfig;

use crate::proxy::ProxyService;

type BoxedBody = BoxBody<Bytes, hyper::Error>;

/// Main proxy handler
pub struct ProxyHandler {
    /// Proxy service for request handling
    proxy: Arc<ProxyService>,

    /// Server listen address
    listen_addr: SocketAddr,
}

impl ProxyHandler {
    /// Create handler from configuration
    pub fn from_config(config: &ApexConfig) -> Self {
        Self {
            proxy: Arc::new(ProxyService::from_config(config)),
            listen_addr: config.server.listen,
        }
    }

    /// Run the HTTP server
    pub async fn run(self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        tracing::info!("Apex listening on {}", self.listen_addr);

        loop {
            let (stream, _remote_addr) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let proxy = Arc::clone(&self.proxy);

            tokio::spawn(async move {
                // Clone outside service_fn to avoid clone per request
                let service = service_fn(|req: Request<Incoming>| {
                    let proxy = Arc::clone(&proxy);
                    async move { handle_request(proxy, req).await }
                });

                if let Err(err) = http1::Builder::new()
                    .keep_alive(true)
                    .serve_connection(io, service)
                    .await
                {
                    if !is_connection_closed_error(&err) {
                        tracing::error!("Connection error: {}", err);
                    }
                }
            });
        }
    }
}

/// Handle a single request - returns streaming response
#[inline]
async fn handle_request(
    proxy: Arc<ProxyService>,
    req: Request<Incoming>,
) -> Result<Response<BoxedBody>, std::convert::Infallible> {
    let response = match proxy.handle(req).await {
        Ok(resp) => resp.map(|b| b.boxed()),
        Err(err) => {
            ProxyService::error_response(&err)
                .map(|b| b.map_err(|_| unreachable!()).boxed())
        }
    };

    Ok(response)
}

/// Check if error is just a closed connection
fn is_connection_closed_error<E: std::fmt::Display>(err: &E) -> bool {
    let msg = err.to_string();
    msg.contains("connection closed")
        || msg.contains("broken pipe")
        || msg.contains("reset by peer")
}
