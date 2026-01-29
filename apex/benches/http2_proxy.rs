//! HTTP/2 Proxy - multiplexing for high throughput
//!
//! Key difference from HTTP/1.1:
//! - Single connection can handle multiple concurrent requests
//! - No head-of-line blocking
//! - Should achieve much higher RPS

use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::server::conn::http2 as server_http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

static BACKEND_ADDR: &str = "127.0.0.1:9001";

/// Shared HTTP/2 connection - multiplexed, single connection handles many requests
struct Http2Connection {
    sender: Mutex<Option<client_http2::SendRequest<Incoming>>>,
    addr: SocketAddr,
}

impl Http2Connection {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: Mutex::new(None),
            addr,
        }
    }

    async fn send(&self, req: Request<Incoming>) -> Result<Response<Incoming>, String> {
        // Clone sender if ready (HTTP/2 SendRequest is Clone-able for multiplexing)
        let mut sender = {
            let mut guard = self.sender.lock().await;

            let needs_new = match &*guard {
                Some(s) => !s.is_ready(),
                None => true,
            };

            if needs_new {
                // Create new HTTP/2 connection
                let stream = TcpStream::connect(self.addr).await
                    .map_err(|e| e.to_string())?;
                stream.set_nodelay(true).ok();
                let io = TokioIo::new(stream);

                let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io).await
                    .map_err(|e| e.to_string())?;

                // Spawn connection driver
                tokio::spawn(async move {
                    if let Err(e) = conn.await {
                        eprintln!("HTTP/2 connection error: {}", e);
                    }
                });

                *guard = Some(sender.clone());
                sender
            } else {
                guard.as_ref().unwrap().clone()
            }
        };

        // Send request (can be concurrent due to HTTP/2 multiplexing)
        sender.send_request(req).await.map_err(|e| e.to_string())
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8085".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("HTTP/2 Proxy on {}", addr);
    println!("Backend: {} (HTTP/2)", BACKEND_ADDR);
    println!("Note: Backend must support HTTP/2!");

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

    match connection.send(forward_req).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            eprintln!("Error: {}", e);
            panic!("Backend error: {}", e);
        }
    }
}
