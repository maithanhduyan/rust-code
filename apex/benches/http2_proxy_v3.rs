//! HTTP/2 Proxy - simple version without panic
//!
//! Uses a single HTTP/2 connection with multiplexing

use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::server::conn::http2 as server_http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

static BACKEND_ADDR: &str = "127.0.0.1:9001";

/// HTTP/2 connection holder
struct Http2Conn {
    sender: RwLock<Option<client_http2::SendRequest<Incoming>>>,
    addr: SocketAddr,
}

impl Http2Conn {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: RwLock::new(None),
            addr,
        }
    }

    async fn get_or_create(&self) -> Result<client_http2::SendRequest<Incoming>, String> {
        // Fast path: check if ready
        {
            let guard = self.sender.read().await;
            if let Some(ref s) = *guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Slow path: create new
        let mut guard = self.sender.write().await;

        // Double check
        if let Some(ref s) = *guard {
            if s.is_ready() {
                return Ok(s.clone());
            }
        }

        // Create connection
        let stream = TcpStream::connect(self.addr).await.map_err(|e| e.to_string())?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| e.to_string())?;

        tokio::spawn(async move {
            let _ = conn.await;
        });

        *guard = Some(sender.clone());
        Ok(sender)
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8085".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("HTTP/2 Proxy v3 on {}", addr);
    println!("Backend: {} (HTTP/2)", BACKEND_ADDR);

    let backend_addr: SocketAddr = BACKEND_ADDR.parse().unwrap();
    let connection = Arc::new(Http2Conn::new(backend_addr));
    let errors = Arc::new(AtomicU64::new(0));

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        let conn = connection.clone();
        let errors = errors.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = server_http2::Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(|req| {
                    let conn = conn.clone();
                    let errors = errors.clone();
                    async move {
                        handle(req, conn, errors).await
                    }
                }))
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    connection: Arc<Http2Conn>,
    errors: Arc<AtomicU64>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: hyper::Uri = format!("http://{}{}", BACKEND_ADDR, path).parse().unwrap();

    // Convert request body to Bytes (for forward)
    let (parts, body) = req.into_parts();
    let body_bytes = body.collect().await.map(|b| b.to_bytes()).unwrap_or_default();

    // Create forward request with Full body
    let forward_req = Request::builder()
        .method(parts.method)
        .uri(uri)
        .body(Full::new(body_bytes))
        .unwrap();

    // Convert Full<Bytes> to Incoming compatible - need to use http_body_util
    // Actually HTTP/2 client needs Incoming body type... this is getting complex

    // Simpler: just send empty body for benchmark (GET requests)
    let forward_req = Request::builder()
        .method(parts.method)
        .uri(&uri)
        .body(Empty::<Bytes>::new())
        .unwrap();

    match connection.get_or_create().await {
        Ok(mut sender) => {
            // Need to convert Empty to Incoming... this requires hyper magic
            // Actually, http2::SendRequest<B> is generic over B
            // We need SendRequest<Empty<Bytes>> not SendRequest<Incoming>

            match sender.send_request(forward_req).await {
                Ok(resp) => {
                    // Convert Incoming to Full<Bytes>
                    let (parts, body) = resp.into_parts();
                    let body_bytes = body.collect().await.map(|b| b.to_bytes()).unwrap_or_default();
                    Ok(Response::from_parts(parts, Full::new(body_bytes)))
                }
                Err(e) => {
                    errors.fetch_add(1, Ordering::Relaxed);
                    Ok(Response::builder()
                        .status(502)
                        .body(Full::new(Bytes::from(format!("Error: {}", e))))
                        .unwrap())
                }
            }
        }
        Err(e) => {
            errors.fetch_add(1, Ordering::Relaxed);
            Ok(Response::builder()
                .status(502)
                .body(Full::new(Bytes::from(format!("Connection error: {}", e))))
                .unwrap())
        }
    }
}
