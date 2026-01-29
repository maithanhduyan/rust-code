//! Apex Server - HTTP server and reverse proxy implementation
//!
//! Built on hyper 1.x with tokio for high-performance async I/O.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod backend_task;
pub mod client;
pub mod handler;
pub mod http2_client;
pub mod http2_client_lockfree;
pub mod http2_handler;
pub mod pool;
pub mod proxy;
pub mod ultra_http2_client;

pub use handler::ProxyHandler;
pub use http2_client::Http2Client;
pub use http2_client_lockfree::Http2ClientLockFree;
pub use http2_handler::Http2Handler;
pub use proxy::ProxyService;
pub use ultra_http2_client::UltraHttp2Client;
