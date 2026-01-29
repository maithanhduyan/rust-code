//! HTTP/2 Proxy - multiplexing for high throughput (optimized)
//!
//! Key difference from HTTP/1.1:
//! - Single connection can handle multiple concurrent requests
//! - No head-of-line blocking
//! - HTTP/2 SendRequest is Clone-able - no lock needed after init

use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::server::conn::http2 as server_http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use arc_swap::ArcSwap;

static BACKEND_ADDR: &str = "127.0.0.1:9001";

/// Lock-free HTTP/2 connection using ArcSwap
struct Http2Connection {
    sender: ArcSwap<Option<client_http2::SendRequest<Incoming>>>,
    init_lock: RwLock<()>,  // Only for initialization
    addr: SocketAddr,
}

impl Http2Connection {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: ArcSwap::new(Arc::new(None)),
            init_lock: RwLock::new(()),
            addr,
        }
    }

    async fn get_sender(&self) -> Result<client_http2::SendRequest<Incoming>, String> {
        // Fast path: try to get existing sender (lock-free read)
        {
            let guard = self.sender.load();
            if let Some(ref s) = **guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Slow path: need to create new connection
        let _lock = self.init_lock.write().await;

        // Double-check after acquiring lock
        {
            let guard = self.sender.load();
            if let Some(ref s) = **guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Create new HTTP/2 connection
        let stream = TcpStream::connect(self.addr).await
            .map_err(|e| e.to_string())?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io).await
            .map_err(|e| e.to_string())?;

        // Spawn connection driver
        tokio::spawn(async move {
            if let Err(_e) = conn.await {
                // Connection closed
            }
        });

        self.sender.store(Arc::new(Some(sender.clone())));
        Ok(sender)
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8085".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("HTTP/2 Proxy v2 on {}", addr);
    println!("Backend: {} (HTTP/2)", BACKEND_ADDR);
    println!("Using ArcSwap for lock-free sender cloning");

    let backend_addr: SocketAddr = BACKEND_ADDR.parse().unwrap();
    let connection = Arc::new(Http2Connection::new(backend_addr));

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        let connection = connection.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = server_http2::Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(|req| {
                    let connection = connection.clone();
                    async move {
                        handle(req, connection).await
                    }
                }))
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    connection: Arc<Http2Connection>,
) -> Result<Response<Incoming>, std::convert::Infallible> {
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: hyper::Uri = format!("http://{}{}", BACKEND_ADDR, path).parse().unwrap();

    let (mut parts, body) = req.into_parts();
    parts.uri = uri;

    let forward_req = Request::from_parts(parts, body);

    match connection.get_sender().await {
        Ok(mut sender) => {
            match sender.send_request(forward_req).await {
                Ok(resp) => Ok(resp),
                Err(_e) => {
                    // Return 502 on error
                    Ok(Response::builder()
                        .status(502)
                        .body(http_body_util::Empty::new().map_err(|e| match e {}).boxed())
                        .unwrap()
                        .map(|_| unreachable!()))
                }
            }
        }
        Err(_e) => {
            Ok(Response::builder()
                .status(502)
                .body(http_body_util::Empty::new().map_err(|e| match e {}).boxed())
                .unwrap()
                .map(|_| unreachable!()))
        }
    }
}
