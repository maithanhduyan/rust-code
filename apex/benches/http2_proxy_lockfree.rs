//! HTTP/2 Proxy - lock-free version with pre-cached sender
//!
//! Key insight: HTTP/2 SendRequest is Clone-able and thread-safe
//! We can clone it ONCE and share across all tasks

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::server::conn::http2 as server_http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

static BACKEND_ADDR: &str = "127.0.0.1:9001";

type Sender = client_http2::SendRequest<Full<Bytes>>;

/// Create HTTP/2 connection to backend
async fn create_backend_connection(addr: SocketAddr) -> Result<Sender, String> {
    let stream = TcpStream::connect(addr).await.map_err(|e| e.to_string())?;
    stream.set_nodelay(true).ok();
    let io = TokioIo::new(stream);

    let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io)
        .await
        .map_err(|e| e.to_string())?;

    // Spawn connection driver
    tokio::spawn(async move {
        let _ = conn.await;
    });

    Ok(sender)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8085".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();

    let backend_addr: SocketAddr = BACKEND_ADDR.parse().unwrap();

    // Pre-create HTTP/2 connection to backend
    println!("Connecting to backend {}...", BACKEND_ADDR);
    let sender = create_backend_connection(backend_addr).await.unwrap();
    let sender = Arc::new(sender);

    println!("HTTP/2 Proxy (lock-free) on {}", addr);
    println!("Backend: {} (HTTP/2, pre-connected)", BACKEND_ADDR);

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        // Clone sender - this is lock-free! HTTP/2 SendRequest is Clone
        let sender = sender.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = server_http2::Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(|req| {
                    let sender = sender.clone();
                    async move { handle(req, sender).await }
                }))
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    sender: Arc<Sender>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: hyper::Uri = format!("http://{}{}", BACKEND_ADDR, path).parse().unwrap();

    // Collect body
    let (parts, body) = req.into_parts();
    let body_bytes = body.collect().await
        .map(|b| b.to_bytes())
        .unwrap_or_default();

    let forward_req = Request::builder()
        .method(parts.method)
        .uri(uri)
        .body(Full::new(body_bytes))
        .unwrap();

    // Clone sender (lock-free Arc::clone) and send
    let mut sender_clone = (*sender).clone();

    match sender_clone.send_request(forward_req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            let body_bytes = body.collect().await
                .map(|b| b.to_bytes())
                .unwrap_or_default();
            Ok(Response::from_parts(parts, Full::new(body_bytes)))
        }
        Err(_) => {
            Ok(Response::builder()
                .status(502)
                .body(Full::new(Bytes::from("Backend error")))
                .unwrap())
        }
    }
}
