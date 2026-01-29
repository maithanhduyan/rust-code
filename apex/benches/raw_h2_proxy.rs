//! RAW HTTP/2 Proxy - Minimal abstraction for maximum performance
//!
//! This proxy uses minimal code between incoming and outgoing requests
//! to measure the theoretical maximum throughput.

use arc_swap::ArcSwap;
use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::server::conn::http2 as server_http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

type ReqSender = client_http2::SendRequest<Empty<Bytes>>;

/// Global shared sender to backend
struct SharedSender {
    sender: ArcSwap<Option<ReqSender>>,
    init_lock: Mutex<()>,
    backend_addr: SocketAddr,
}

impl SharedSender {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: ArcSwap::new(Arc::new(None)),
            init_lock: Mutex::new(()),
            backend_addr: addr,
        }
    }

    #[inline(always)]
    async fn get(&self) -> Result<ReqSender, Box<dyn std::error::Error + Send + Sync>> {
        // Fast path - already have valid sender
        let guard = self.sender.load();
        if let Some(ref s) = **guard {
            if s.is_ready() {
                return Ok(s.clone());
            }
        }
        drop(guard);

        // Slow path - create connection
        self.create().await
    }

    #[cold]
    async fn create(&self) -> Result<ReqSender, Box<dyn std::error::Error + Send + Sync>> {
        let _lock = self.init_lock.lock().await;

        // Double-check after acquiring lock
        let guard = self.sender.load();
        if let Some(ref s) = **guard {
            if s.is_ready() {
                return Ok(s.clone());
            }
        }
        drop(guard);

        // Create new connection
        let stream = TcpStream::connect(self.backend_addr).await?;
        stream.set_nodelay(true)?;
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io).await?;
        tokio::spawn(async move { let _ = conn.await; });

        self.sender.store(Arc::new(Some(sender.clone())));
        Ok(sender)
    }
}

/// Request counter
static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

/// Handle incoming request - minimal overhead
#[inline(always)]
async fn handle(
    shared: Arc<SharedSender>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);

    // Get path
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    // Build URI
    let uri = hyper::Uri::builder()
        .scheme("http")
        .authority(format!("{}", shared.backend_addr).as_str())
        .path_and_query(path)
        .build()?;

    // Build request with empty body
    let backend_req = Request::get(uri)
        .body(Empty::new())?;

    // Send request
    let mut sender = shared.get().await?;
    let resp = sender.send_request(backend_req).await?;

    // Collect response
    let (parts, body) = resp.into_parts();
    let body_bytes = body.collect().await?.to_bytes();

    Ok(Response::from_parts(parts, Full::new(body_bytes)))
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let backend_addr: SocketAddr = "127.0.0.1:9001".parse()?;
    let proxy_addr: SocketAddr = "0.0.0.0:8080".parse()?;

    println!("=== RAW HTTP/2 Proxy ===");
    println!("Listening on: {}", proxy_addr);
    println!("Backend: {}", backend_addr);

    // Create shared sender
    let shared = Arc::new(SharedSender::new(backend_addr));

    // Start metrics reporter
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut prev = 0u64;
        loop {
            interval.tick().await;
            let current = REQUEST_COUNT.load(Ordering::Relaxed);
            let rps = current - prev;
            prev = current;
            if rps > 0 {
                println!("RPS: {}", rps);
            }
        }
    });

    // Listen
    let listener = TcpListener::bind(proxy_addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        stream.set_nodelay(true)?;
        let io = TokioIo::new(stream);
        let shared = shared.clone();

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let shared = shared.clone();
                handle(shared, req)
            });

            let mut builder = server_http2::Builder::new(TokioExecutor::new());
            builder.max_concurrent_streams(10000);
            builder.initial_stream_window_size(1024 * 1024);
            builder.initial_connection_window_size(2 * 1024 * 1024);

            if let Err(e) = builder.serve_connection(io, service).await {
                if !e.to_string().contains("connection closed") {
                    eprintln!("Connection error: {}", e);
                }
            }
        });
    }
}
