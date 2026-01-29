---
date: 2026-01-28 14:41:56 
---

# Cấu trúc Dự án như sau:

```
./
├── Cargo.toml
├── benches
│   ├── Cargo.toml
│   ├── bench.rs
│   ├── bench_h2.rs
│   ├── direct_proxy.rs
│   ├── echo_server.rs
│   ├── echo_server_h2.rs
│   ├── http2_proxy.rs
│   ├── http2_proxy_lockfree.rs
│   ├── http2_proxy_v2.rs
│   ├── http2_proxy_v3.rs
│   ├── http2_proxy_v4.rs
│   ├── lockfree_pool_proxy.rs
│   ├── minimal_proxy.rs
│   └── semaphore_pool_proxy.rs
├── config.toml
├── crates
│   ├── apex
│   │   ├── Cargo.toml
│   │   └── src
│   │       └── main.rs
│   ├── config
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── lib.rs
│   │       ├── loader.rs
│   │       └── types.rs
│   ├── core
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── backend.rs
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       └── router.rs
│   └── server
│       ├── Cargo.toml
│       └── src
│           ├── backend_task.rs
│           ├── client.rs
│           ├── handler.rs
│           ├── http2_client.rs
│           ├── http2_client_lockfree.rs
│           ├── http2_handler.rs
│           ├── lib.rs
│           ├── pool.rs
│           └── proxy.rs
├── rust-toolchain.toml
└── scripts
```

# Danh sách chi tiết các file:

## File ./benches\bench.rs:
```rust
//! Simple HTTP benchmark tool

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::Request;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().collect();

    let url = args.get(1).map(|s| s.as_str()).unwrap_or("http://127.0.0.1:8080/");
    let concurrency: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    println!("Benchmark: {}", url);
    println!("Concurrency: {}", concurrency);
    println!("Duration: {}s", duration_secs);
    println!();

    let client = Client::builder(TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(concurrency)
        .build_http::<Empty<Bytes>>();

    let success_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    let total_latency_us = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);

    let mut handles = vec![];

    // Spawn worker tasks
    for _ in 0..concurrency {
        let client = client.clone();
        let url = url.to_string();
        let success = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);
        let latency = Arc::clone(&total_latency_us);
        let running = Arc::clone(&running);

        let handle = tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                let req_start = Instant::now();
                let req = Request::builder()
                    .uri(&url)
                    .body(Empty::<Bytes>::new())
                    .unwrap();

                match client.request(req).await {
                    Ok(resp) => {
                        // Consume body
                        let _ = resp.into_body().collect().await;
                        success.fetch_add(1, Ordering::Relaxed);
                        latency.fetch_add(req_start.elapsed().as_micros() as u64, Ordering::Relaxed);
                    }
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait for duration
    tokio::time::sleep(duration).await;
    running.store(false, Ordering::Relaxed);

    // Wait for all workers to finish current request
    for handle in handles {
        let _ = handle.await;
    }

    let elapsed = start.elapsed();
    let success = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let total_latency = total_latency_us.load(Ordering::Relaxed);

    let rps = success as f64 / elapsed.as_secs_f64();
    let avg_latency_us = if success > 0 {
        total_latency as f64 / success as f64
    } else {
        0.0
    };

    println!("=== Results ===");
    println!("Duration:     {:.2}s", elapsed.as_secs_f64());
    println!("Requests:     {}", success);
    println!("Errors:       {}", errors);
    println!("RPS:          {:.2}", rps);
    println!("Avg Latency:  {:.2}μs ({:.2}ms)", avg_latency_us, avg_latency_us / 1000.0);

    Ok(())
}

```

## File ./benches\bench_h2.rs:
```rust
//! HTTP/2 benchmark tool

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::client::conn::http2;
use hyper::Request;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().collect();

    let url = args.get(1).map(|s| s.as_str()).unwrap_or("http://127.0.0.1:8085/");
    let concurrency: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    // Parse host:port from URL
    let url_parsed: hyper::Uri = url.parse()?;
    let host = url_parsed.host().unwrap_or("127.0.0.1");
    let port = url_parsed.port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);

    println!("HTTP/2 Benchmark: {}", url);
    println!("Concurrency: {}", concurrency);
    println!("Duration: {}s", duration_secs);
    println!();

    // Create single HTTP/2 connection (multiplexed)
    let stream = TcpStream::connect(&addr).await?;
    stream.set_nodelay(true)?;
    let io = TokioIo::new(stream);

    let (sender, conn) = http2::handshake(TokioExecutor::new(), io).await?;

    // Spawn connection driver
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("Connection error: {}", e);
        }
    });

    let sender = Arc::new(Mutex::new(sender));

    let success_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    let total_latency_us = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);

    let mut handles = vec![];

    // Spawn worker tasks - all share single HTTP/2 connection
    for _ in 0..concurrency {
        let sender = Arc::clone(&sender);
        let url = url.to_string();
        let success = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);
        let latency = Arc::clone(&total_latency_us);
        let running = Arc::clone(&running);

        let handle = tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                let req_start = Instant::now();
                let req = Request::builder()
                    .uri(&url)
                    .body(Empty::<Bytes>::new())
                    .unwrap();

                // Clone sender for this request (HTTP/2 allows concurrent sends)
                let mut sender_clone = {
                    let guard = sender.lock().await;
                    guard.clone()
                };

                match sender_clone.send_request(req).await {
                    Ok(resp) => {
                        // Consume body
                        let _ = resp.into_body().collect().await;
                        success.fetch_add(1, Ordering::Relaxed);
                        latency.fetch_add(req_start.elapsed().as_micros() as u64, Ordering::Relaxed);
                    }
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait for duration
    tokio::time::sleep(duration).await;
    running.store(false, Ordering::Relaxed);

    // Wait for all workers to finish current request
    for handle in handles {
        let _ = handle.await;
    }

    let elapsed = start.elapsed();
    let success = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let total_latency = total_latency_us.load(Ordering::Relaxed);

    let rps = success as f64 / elapsed.as_secs_f64();
    let avg_latency_us = if success > 0 {
        total_latency as f64 / success as f64
    } else {
        0.0
    };

    println!("=== Results ===");
    println!("Duration:     {:.2}s", elapsed.as_secs_f64());
    println!("Requests:     {}", success);
    println!("Errors:       {}", errors);
    println!("RPS:          {:.2}", rps);
    println!("Avg Latency:  {:.2}μs ({:.2}ms)", avg_latency_us, avg_latency_us / 1000.0);

    Ok(())
}

```

## File ./benches\direct_proxy.rs:
```rust
//! Direct proxy - using hyper directly without legacy Client
//! To test if we can eliminate hyper-util overhead

use hyper::body::Incoming;
use hyper::client::conn::http1;
use hyper::server::conn::http1 as server_http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

static BACKEND_ADDR: &str = "127.0.0.1:9001";

/// Shared connection pool - very simple
struct ConnectionPool {
    sender: Mutex<Option<http1::SendRequest<Incoming>>>,
    addr: SocketAddr,
}

impl ConnectionPool {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: Mutex::new(None),
            addr,
        }
    }

    async fn send(&self, req: Request<Incoming>) -> Result<Response<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
        let mut guard = self.sender.lock().await;

        // Check if we have a ready connection
        let needs_new = match &*guard {
            Some(s) => !s.is_ready(),
            None => true,
        };

        if needs_new {
            // Create new connection
            let stream = TcpStream::connect(self.addr).await?;
            stream.set_nodelay(true)?;
            let io = TokioIo::new(stream);

            let (sender, conn) = http1::handshake(io).await?;

            // Spawn connection driver
            tokio::spawn(async move {
                let _ = conn.await;
            });

            *guard = Some(sender);
        }

        let sender = guard.as_mut().unwrap();
        let resp = sender.send_request(req).await?;
        Ok(resp)
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8082".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Direct proxy on {}", addr);
    println!("Backend: {}", BACKEND_ADDR);

    let backend_addr: SocketAddr = BACKEND_ADDR.parse().unwrap();
    let pool = Arc::new(ConnectionPool::new(backend_addr));

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        let pool = pool.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = server_http1::Builder::new()
                .serve_connection(io, service_fn(|req| {
                    let pool = pool.clone();
                    async move {
                        handle(req, pool).await
                    }
                }))
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    pool: Arc<ConnectionPool>,
) -> Result<Response<Incoming>, std::convert::Infallible> {
    // Build backend URI
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: hyper::Uri = format!("http://{}{}", BACKEND_ADDR, path).parse().unwrap();

    let (mut parts, body) = req.into_parts();
    parts.uri = uri;
    parts.headers.remove("connection");

    let forward_req = Request::from_parts(parts, body);

    match pool.send(forward_req).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            eprintln!("Error: {}", e);
            panic!("Backend error: {}", e);
        }
    }
}

```

## File ./benches\echo_server.rs:
```rust
//! Simple echo server for benchmarking
//!
//! Returns a simple JSON response for any request.

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port: u16 = env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(9001);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    println!("Echo server listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .keep_alive(true)
                .serve_connection(io, service_fn(handle))
                .await
            {
                if !err.to_string().contains("connection closed") {
                    eprintln!("Error: {}", err);
                }
            }
        });
    }
}

async fn handle(_req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let body = r#"{"status":"ok","message":"Hello from echo server"}"#;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .unwrap())
}

```

## File ./benches\echo_server_h2.rs:
```rust
//! HTTP/2 Echo Server - for testing HTTP/2 proxy
//!
//! Supports HTTP/2 multiplexing

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn echo(_req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("OK"))))
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let port = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9001);

    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("HTTP/2 Echo server listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = http2::Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(echo))
                .await;
        });
    }
}

```

## File ./benches\http2_proxy.rs:
```rust
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

```

## File ./benches\http2_proxy_lockfree.rs:
```rust
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

```

## File ./benches\http2_proxy_v2.rs:
```rust
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

```

## File ./benches\http2_proxy_v3.rs:
```rust
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

```

## File ./benches\http2_proxy_v4.rs:
```rust
//! HTTP/2 Proxy - clean version with proper body handling
//!
//! Key insight: Use SendRequest<Full<Bytes>> for both directions

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
use tokio::sync::RwLock;

static BACKEND_ADDR: &str = "127.0.0.1:9001";

type Sender = client_http2::SendRequest<Full<Bytes>>;

/// HTTP/2 connection holder
struct Http2Conn {
    sender: RwLock<Option<Sender>>,
    addr: SocketAddr,
}

impl Http2Conn {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: RwLock::new(None),
            addr,
        }
    }

    async fn get_or_create(&self) -> Result<Sender, String> {
        // Fast path
        {
            let guard = self.sender.read().await;
            if let Some(ref s) = *guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Slow path
        let mut guard = self.sender.write().await;

        if let Some(ref s) = *guard {
            if s.is_ready() {
                return Ok(s.clone());
            }
        }

        let stream = TcpStream::connect(self.addr).await.map_err(|e| e.to_string())?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| e.to_string())?;

        tokio::spawn(async move { let _ = conn.await; });

        *guard = Some(sender.clone());
        Ok(sender)
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8085".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("HTTP/2 Proxy v4 on {}", addr);
    println!("Backend: {}", BACKEND_ADDR);

    let backend_addr: SocketAddr = BACKEND_ADDR.parse().unwrap();
    let connection = Arc::new(Http2Conn::new(backend_addr));

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        let conn = connection.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = server_http2::Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(|req| {
                    let conn = conn.clone();
                    async move { handle(req, conn).await }
                }))
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    connection: Arc<Http2Conn>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: hyper::Uri = format!("http://{}{}", BACKEND_ADDR, path).parse().unwrap();

    // Collect incoming body
    let (parts, body) = req.into_parts();
    let body_bytes = match body.collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => Bytes::new(),
    };

    // Build forward request
    let forward_req = Request::builder()
        .method(parts.method)
        .uri(uri)
        .body(Full::new(body_bytes))
        .unwrap();

    match connection.get_or_create().await {
        Ok(mut sender) => {
            match sender.send_request(forward_req).await {
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
        Err(_) => {
            Ok(Response::builder()
                .status(502)
                .body(Full::new(Bytes::from("Connection error")))
                .unwrap())
        }
    }
}

```

## File ./benches\lockfree_pool_proxy.rs:
```rust
//! Lock-free pool proxy - multiple connections, atomic selection
//! Using crossbeam ArrayQueue for lock-free connection pool

use hyper::body::Incoming;
use hyper::client::conn::http1;
use hyper::server::conn::http1 as server_http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

static BACKEND_ADDR: &str = "127.0.0.1:9001";
const NUM_CONNECTIONS: usize = 512;

/// Request to send to backend
struct BackendRequest {
    request: Request<Incoming>,
    response_tx: oneshot::Sender<Result<Response<Incoming>, String>>,
}

/// Connection pool using mpsc channels
struct ConnectionPool {
    senders: Vec<mpsc::Sender<BackendRequest>>,
    counter: AtomicUsize,
}

impl ConnectionPool {
    async fn new(addr: SocketAddr, num_conns: usize) -> Self {
        let mut senders = Vec::with_capacity(num_conns);

        for _ in 0..num_conns {
            let (tx, rx) = mpsc::channel::<BackendRequest>(64);

            // Spawn worker task for this connection
            tokio::spawn(connection_worker(addr, rx));

            senders.push(tx);
        }

        Self {
            senders,
            counter: AtomicUsize::new(0),
        }
    }

    async fn send(&self, req: Request<Incoming>) -> Result<Response<Incoming>, String> {
        let (response_tx, response_rx) = oneshot::channel();

        let backend_req = BackendRequest {
            request: req,
            response_tx,
        };

        // Round-robin selection
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % self.senders.len();

        self.senders[idx].send(backend_req).await
            .map_err(|_| "channel closed".to_string())?;

        response_rx.await
            .map_err(|_| "response channel closed".to_string())?
    }
}

async fn connection_worker(
    addr: SocketAddr,
    mut rx: mpsc::Receiver<BackendRequest>,
) {
    let mut sender: Option<http1::SendRequest<Incoming>> = None;

    while let Some(req) = rx.recv().await {
        // Ensure connection
        let s = match ensure_connection(&mut sender, addr).await {
            Ok(s) => s,
            Err(e) => {
                let _ = req.response_tx.send(Err(e.to_string()));
                continue;
            }
        };

        // Send request
        match s.send_request(req.request).await {
            Ok(response) => {
                let _ = req.response_tx.send(Ok(response));
            }
            Err(e) => {
                sender = None; // Reconnect next time
                let _ = req.response_tx.send(Err(e.to_string()));
            }
        }
    }
}

async fn ensure_connection(
    sender: &mut Option<http1::SendRequest<Incoming>>,
    addr: SocketAddr,
) -> Result<&mut http1::SendRequest<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(ref s) = sender {
        if s.is_ready() {
            return Ok(sender.as_mut().unwrap());
        }
    }

    let stream = TcpStream::connect(addr).await?;
    stream.set_nodelay(true)?;
    let io = TokioIo::new(stream);

    let (new_sender, conn) = http1::handshake(io).await?;

    tokio::spawn(async move {
        let _ = conn.await;
    });

    *sender = Some(new_sender);
    Ok(sender.as_mut().unwrap())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8083".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Lock-free pool proxy on {}", addr);
    println!("Backend: {} x {} connections", BACKEND_ADDR, NUM_CONNECTIONS);

    let backend_addr: SocketAddr = BACKEND_ADDR.parse().unwrap();
    let pool = Arc::new(ConnectionPool::new(backend_addr, NUM_CONNECTIONS).await);

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        let pool = pool.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = server_http1::Builder::new()
                .serve_connection(io, service_fn(|req| {
                    let pool = pool.clone();
                    async move {
                        handle(req, pool).await
                    }
                }))
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    pool: Arc<ConnectionPool>,
) -> Result<Response<Incoming>, std::convert::Infallible> {
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: hyper::Uri = format!("http://{}{}", BACKEND_ADDR, path).parse().unwrap();

    let (mut parts, body) = req.into_parts();
    parts.uri = uri;
    parts.headers.remove("connection");

    let forward_req = Request::from_parts(parts, body);

    match pool.send(forward_req).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            eprintln!("Error: {}", e);
            panic!("Backend error: {}", e);
        }
    }
}

```

## File ./benches\minimal_proxy.rs:
```rust
//! Minimal proxy - just forward, no routing, no frills
//! To test baseline proxy overhead

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use tokio::net::TcpListener;

static BACKEND: &str = "http://127.0.0.1:9001";

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Minimal proxy on {}", addr);
    println!("Backend: {}", BACKEND);

    // Create shared client
    let mut connector = hyper_util::client::legacy::connect::HttpConnector::new();
    connector.set_nodelay(true);

    let client: Client<_, Incoming> = Client::builder(TokioExecutor::new())
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .pool_max_idle_per_host(1024)
        .build(connector);

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        let client = client.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = http1::Builder::new()
                .serve_connection(io, service_fn(|req| handle(req, client.clone())))
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Incoming>,
) -> Result<Response<Incoming>, std::convert::Infallible> {
    // Build backend URI
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: hyper::Uri = format!("{}{}", BACKEND, path).parse().unwrap();

    let (mut parts, body) = req.into_parts();
    parts.uri = uri;
    parts.headers.remove("connection");

    let forward_req = Request::from_parts(parts, body);

    match client.request(forward_req).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            eprintln!("Error: {}", e);
            // Return a simple error - won't happen in benchmark
            panic!("Backend error: {}", e);
        }
    }
}

```

## File ./benches\semaphore_pool_proxy.rs:
```rust
//! Semaphore pool proxy - acquire connection permit, then send
//! Eliminates channel overhead by directly using connections

use hyper::body::Incoming;
use hyper::client::conn::http1;
use hyper::server::conn::http1 as server_http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

static BACKEND_ADDR: &str = "127.0.0.1:9001";

/// Simple connection - just holds SendRequest behind mutex
struct Connection {
    sender: Mutex<Option<http1::SendRequest<Incoming>>>,
    addr: SocketAddr,
}

impl Connection {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: Mutex::new(None),
            addr,
        }
    }

    async fn send(&self, req: Request<Incoming>) -> Result<Response<Incoming>, String> {
        let mut guard = self.sender.lock().await;

        // Ensure connection
        let needs_new = match &*guard {
            Some(s) => !s.is_ready(),
            None => true,
        };

        if needs_new {
            let stream = TcpStream::connect(self.addr).await
                .map_err(|e| e.to_string())?;
            stream.set_nodelay(true).ok();
            let io = TokioIo::new(stream);

            let (new_sender, conn) = http1::handshake(io).await
                .map_err(|e| e.to_string())?;

            tokio::spawn(async move {
                let _ = conn.await;
            });

            *guard = Some(new_sender);
        }

        let sender = guard.as_mut().unwrap();
        sender.send_request(req).await
            .map_err(|e| e.to_string())
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8084".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Semaphore pool proxy on {}", addr);
    println!("Backend: {}", BACKEND_ADDR);

    let backend_addr: SocketAddr = BACKEND_ADDR.parse().unwrap();

    // Create a pool of connections
    let num_connections = 128;

    let connections: Arc<Vec<Connection>> = Arc::new(
        (0..num_connections)
            .map(|_| Connection::new(backend_addr))
            .collect()
    );

    println!("Created {} connections", connections.len());

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).ok();

        let connections = connections.clone();
        let counter = counter.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = server_http1::Builder::new()
                .serve_connection(io, service_fn(|req| {
                    let connections = connections.clone();
                    let counter = counter.clone();
                    async move {
                        handle(req, connections, counter).await
                    }
                }))
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    connections: Arc<Vec<Connection>>,
    counter: Arc<std::sync::atomic::AtomicUsize>,
) -> Result<Response<Incoming>, std::convert::Infallible> {
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: hyper::Uri = format!("http://{}{}", BACKEND_ADDR, path).parse().unwrap();

    let (mut parts, body) = req.into_parts();
    parts.uri = uri;
    parts.headers.remove("connection");

    let forward_req = Request::from_parts(parts, body);

    // Round-robin connection selection
    let idx = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % connections.len();

    match connections[idx].send(forward_req).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            eprintln!("Error: {}", e);
            panic!("Backend error: {}", e);
        }
    }
}

```

## File ./crates\apex\src\main.rs:
```rust
//! Apex - High-performance reverse proxy
//!
//! # Usage
//! ```bash
//! apex --config apex.toml
//! apex --config apex.toml --http2    # Use HTTP/2 for higher throughput
//! apex --config apex.toml --check    # Validate config only
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use apex_config::ConfigLoader;
use apex_server::{ProxyHandler, Http2Handler};

/// Apex - High-performance reverse proxy written in Rust
#[derive(Parser, Debug)]
#[command(name = "apex")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "apex.toml")]
    config: PathBuf,

    /// Validate configuration and exit
    #[arg(long)]
    check: bool,

    /// Use HTTP/2 for client and backend (higher throughput)
    #[arg(long)]
    http2: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    init_logging(&args.log_level);

    tracing::info!("Apex v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let loader = ConfigLoader::load_file(&args.config)
        .with_context(|| format!("Failed to load config from {:?}", args.config))?;

    let config = loader.get();

    tracing::info!("Loaded configuration with {} routes", config.routes.len());

    // Config check mode
    if args.check {
        tracing::info!("Configuration is valid");
        return Ok(());
    }

    // Create and run server
    if args.http2 {
        tracing::info!("Starting Apex HTTP/2 proxy server (high-throughput mode)...");
        let handler = Http2Handler::from_config(&config);
        handler.run().await?;
    } else {
        tracing::info!("Starting Apex proxy server...");
        let handler = ProxyHandler::from_config(&config);
        handler.run().await?;
    }

    Ok(())
}

fn init_logging(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .init();
}

```

## File ./crates\config\src\lib.rs:
```rust
//! Apex Config - Configuration management
//!
//! Supports hot reload via ArcSwap.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod loader;
pub mod types;

pub use loader::ConfigLoader;
pub use types::{ApexConfig, BackendConfig, RouteConfig, ServerConfig};

```

## File ./crates\config\src\loader.rs:
```rust
//! Configuration loader with hot reload support

use arc_swap::ArcSwap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

use crate::types::ApexConfig;

/// Configuration loading errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// File not found
    #[error("config file not found: {0}")]
    NotFound(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parse error
    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),

    /// Validation error
    #[error("validation error: {0}")]
    Validation(String),
}

/// Configuration loader with hot reload support
pub struct ConfigLoader {
    /// Current configuration (lock-free swappable)
    config: ArcSwap<ApexConfig>,

    /// Path to config file (for reload)
    config_path: Option<std::path::PathBuf>,
}

impl ConfigLoader {
    /// Create loader with default configuration
    pub fn new() -> Self {
        Self {
            config: ArcSwap::from_pointee(ApexConfig::default()),
            config_path: None,
        }
    }

    /// Load configuration from file
    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }

        let content = std::fs::read_to_string(path)?;
        let config: ApexConfig = toml::from_str(&content)?;

        // Validate configuration
        Self::validate(&config)?;

        Ok(Self {
            config: ArcSwap::from_pointee(config),
            config_path: Some(path.to_path_buf()),
        })
    }

    /// Load configuration from string
    pub fn load_str(content: &str) -> Result<Self, ConfigError> {
        let config: ApexConfig = toml::from_str(content)?;
        Self::validate(&config)?;

        Ok(Self {
            config: ArcSwap::from_pointee(config),
            config_path: None,
        })
    }

    /// Get current configuration (lock-free)
    #[inline]
    pub fn get(&self) -> Arc<ApexConfig> {
        self.config.load_full()
    }

    /// Reload configuration from file
    /// 
    /// # Performance
    /// Lock-free swap, O(1) for readers
    pub fn reload(&self) -> Result<(), ConfigError> {
        let path = self.config_path.as_ref().ok_or_else(|| {
            ConfigError::Validation("no config file path set".to_string())
        })?;

        let content = std::fs::read_to_string(path)?;
        let new_config: ApexConfig = toml::from_str(&content)?;

        Self::validate(&new_config)?;

        // Atomic swap - existing readers continue with old config
        self.config.store(Arc::new(new_config));

        tracing::info!("Configuration reloaded successfully");
        Ok(())
    }

    /// Update configuration programmatically
    pub fn update(&self, new_config: ApexConfig) -> Result<(), ConfigError> {
        Self::validate(&new_config)?;
        self.config.store(Arc::new(new_config));
        Ok(())
    }

    /// Validate configuration
    fn validate(config: &ApexConfig) -> Result<(), ConfigError> {
        // Validate routes
        for route in &config.routes {
            if route.backends.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "route '{}' has no backends",
                    route.name
                )));
            }

            for backend in &route.backends {
                // Basic URL validation
                if backend.url.is_empty() {
                    return Err(ConfigError::Validation(format!(
                        "route '{}' has backend with empty URL",
                        route.name
                    )));
                }
            }
        }

        Ok(())
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_string() {
        let config_str = r#"
[server]
listen = "127.0.0.1:8080"

[[routes]]
name = "test"
backends = [{ url = "http://localhost:9000" }]
"#;

        let loader = ConfigLoader::load_str(config_str).unwrap();
        let config = loader.get();

        assert_eq!(config.server.listen.port(), 8080);
        assert_eq!(config.routes.len(), 1);
    }

    #[test]
    fn test_validation_empty_backends() {
        let config_str = r#"
[[routes]]
name = "test"
backends = []
"#;

        let result = ConfigLoader::load_str(config_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_hot_reload() {
        let config_str = r#"
[[routes]]
name = "v1"
backends = [{ url = "http://localhost:9000" }]
"#;

        let loader = ConfigLoader::load_str(config_str).unwrap();
        
        // Get initial config
        let config1 = loader.get();
        assert_eq!(config1.routes[0].name, "v1");

        // Update config
        let mut new_config = (*config1).clone();
        new_config.routes[0].name = "v2".to_string();
        loader.update(new_config).unwrap();

        // Get updated config
        let config2 = loader.get();
        assert_eq!(config2.routes[0].name, "v2");
    }
}

```

## File ./crates\config\src\types.rs:
```rust
//! Configuration types

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApexConfig {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Route definitions
    #[serde(default)]
    pub routes: Vec<RouteConfig>,
}

impl Default for ApexConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            routes: Vec::new(),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Listen address
    #[serde(default = "default_listen_addr")]
    pub listen: SocketAddr,

    /// Number of worker threads (0 = auto)
    #[serde(default)]
    pub workers: usize,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Maximum connections per backend
    #[serde(default = "default_max_connections")]
    pub max_connections_per_backend: usize,

    /// Enable access logging
    #[serde(default = "default_true")]
    pub access_log: bool,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_listen_addr() -> SocketAddr {
    "0.0.0.0:8080".parse().unwrap()
}

fn default_timeout() -> u64 {
    30
}

fn default_max_connections() -> usize {
    100
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: default_listen_addr(),
            workers: 0,
            timeout_secs: default_timeout(),
            max_connections_per_backend: default_max_connections(),
            access_log: true,
            log_level: default_log_level(),
        }
    }
}

/// Route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Route name for logging
    pub name: String,

    /// Host pattern (e.g., "api.example.com", "*" for any)
    #[serde(default = "default_host")]
    pub host: String,

    /// Path prefix to match
    #[serde(default = "default_path")]
    pub path_prefix: String,

    /// Backend servers
    pub backends: Vec<BackendConfig>,

    /// Strip path prefix before forwarding
    #[serde(default)]
    pub strip_prefix: bool,

    /// Load balancing strategy
    #[serde(default)]
    pub load_balancing: LoadBalancingStrategy,
}

fn default_host() -> String {
    "*".to_string()
}

fn default_path() -> String {
    "/".to_string()
}

/// Load balancing strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalancingStrategy {
    /// Round-robin (default)
    #[default]
    RoundRobin,
    /// Least connections
    LeastConnections,
    /// Random
    Random,
}

/// Backend server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Backend URL or address
    pub url: String,

    /// Weight for weighted load balancing
    #[serde(default = "default_weight")]
    pub weight: u32,

    /// Health check path
    #[serde(default)]
    pub health_check: Option<String>,
}

fn default_weight() -> u32 {
    1
}

/// TLS configuration (for future use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert: PathBuf,

    /// Path to private key file  
    pub key: PathBuf,

    /// Enable ACME/Let's Encrypt
    #[serde(default)]
    pub acme: Option<AcmeConfig>,
}

/// ACME configuration (for future use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcmeConfig {
    /// Email for ACME account
    pub email: String,

    /// Use staging environment
    #[serde(default)]
    pub staging: bool,

    /// Domains to request certificates for
    pub domains: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ApexConfig::default();
        assert_eq!(config.server.listen.port(), 8080);
        assert!(config.routes.is_empty());
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[server]
listen = "127.0.0.1:3000"

[[routes]]
name = "api"
backends = [{ url = "http://localhost:8001" }]
"#;

        let config: ApexConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.server.listen.port(), 3000);
        assert_eq!(config.routes.len(), 1);
        assert_eq!(config.routes[0].name, "api");
    }
}

```

## File ./crates\core\src\backend.rs:
```rust
//! Backend representation and connection pooling
//!
//! Uses lock-free data structures for hot path performance.

use arc_swap::ArcSwap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// A single backend server
#[derive(Debug)]
pub struct Backend {
    /// Backend address
    pub addr: SocketAddr,

    /// Pre-computed URI base: "http://host:port"
    pub uri_base: String,

    /// Pre-computed authority string for URI building
    pub authority: String,

    /// Whether this backend is healthy
    healthy: AtomicBool,

    /// Active connection count
    active_connections: AtomicU64,

    /// Total requests served
    total_requests: AtomicU64,
}

impl Backend {
    /// Create a new backend
    pub fn new(addr: SocketAddr) -> Self {
        let uri_base = format!("http://{}", addr);
        let authority = addr.to_string();
        Self {
            addr,
            uri_base,
            authority,
            healthy: AtomicBool::new(true),
            active_connections: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
        }
    }

    /// Check if backend is healthy
    #[inline]
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    /// Set backend health status
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Relaxed);
    }

    /// Get active connection count
    #[inline]
    pub fn active_connections(&self) -> u64 {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Increment active connections (called when starting request)
    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connections (called when request completes)
    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Increment total requests counter
    pub fn inc_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total requests served
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }
}

/// A pool of backends with lock-free selection
#[derive(Debug)]
pub struct BackendPool {
    /// List of backends (lock-free swappable)
    backends: ArcSwap<Vec<Arc<Backend>>>,

    /// Round-robin counter
    next_idx: AtomicU64,
}

impl BackendPool {
    /// Create a new empty backend pool
    pub fn new() -> Self {
        Self {
            backends: ArcSwap::from_pointee(Vec::new()),
            next_idx: AtomicU64::new(0),
        }
    }

    /// Create pool from list of backends
    pub fn from_backends(backends: Vec<Arc<Backend>>) -> Self {
        Self {
            backends: ArcSwap::from_pointee(backends),
            next_idx: AtomicU64::new(0),
        }
    }

    /// Get next healthy backend using round-robin
    /// 
    /// # Performance
    /// O(n) worst case where n = number of backends
    /// Lock-free, uses atomic operations only
    #[inline]
    pub fn next_healthy(&self) -> Option<Arc<Backend>> {
        let backends = self.backends.load();
        let len = backends.len();

        if len == 0 {
            return None;
        }

        // Try each backend once
        for _ in 0..len {
            let idx = self.next_idx.fetch_add(1, Ordering::Relaxed) as usize % len;
            let backend = &backends[idx];

            if backend.is_healthy() {
                return Some(Arc::clone(backend));
            }
        }

        None
    }

    /// Get backend with least connections (for load balancing)
    /// 
    /// # Performance
    /// O(n) where n = number of backends
    #[inline]
    pub fn least_connections(&self) -> Option<Arc<Backend>> {
        let backends = self.backends.load();

        backends
            .iter()
            .filter(|b| b.is_healthy())
            .min_by_key(|b| b.active_connections())
            .cloned()
    }

    /// Update the backend list atomically (for hot reload)
    /// 
    /// # Performance
    /// Lock-free swap, O(1)
    pub fn update(&self, new_backends: Vec<Arc<Backend>>) {
        self.backends.store(Arc::new(new_backends));
    }

    /// Get current backend count
    pub fn len(&self) -> usize {
        self.backends.load().len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get all backends (for health checking)
    pub fn all(&self) -> Arc<Vec<Arc<Backend>>> {
        self.backends.load_full()
    }
}

impl Default for BackendPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_health() {
        let backend = Backend::new("127.0.0.1:8080".parse().unwrap());
        assert!(backend.is_healthy());

        backend.set_healthy(false);
        assert!(!backend.is_healthy());
    }

    #[test]
    fn test_round_robin() {
        let backends = vec![
            Arc::new(Backend::new("127.0.0.1:8001".parse().unwrap())),
            Arc::new(Backend::new("127.0.0.1:8002".parse().unwrap())),
            Arc::new(Backend::new("127.0.0.1:8003".parse().unwrap())),
        ];

        let pool = BackendPool::from_backends(backends);

        // Should cycle through backends
        let b1 = pool.next_healthy().unwrap();
        let b2 = pool.next_healthy().unwrap();
        let b3 = pool.next_healthy().unwrap();
        let b4 = pool.next_healthy().unwrap();

        assert_eq!(b1.addr.port(), 8001);
        assert_eq!(b2.addr.port(), 8002);
        assert_eq!(b3.addr.port(), 8003);
        assert_eq!(b4.addr.port(), 8001); // Wraps around
    }

    #[test]
    fn test_skip_unhealthy() {
        let backends = vec![
            Arc::new(Backend::new("127.0.0.1:8001".parse().unwrap())),
            Arc::new(Backend::new("127.0.0.1:8002".parse().unwrap())),
        ];

        backends[0].set_healthy(false);

        let pool = BackendPool::from_backends(backends);

        // Should always return the healthy one
        for _ in 0..10 {
            let b = pool.next_healthy().unwrap();
            assert_eq!(b.addr.port(), 8002);
        }
    }
}

```

## File ./crates\core\src\error.rs:
```rust
//! Error types for Apex
//!
//! All errors are non-panicking and propagate via Result.

use thiserror::Error;

/// Core proxy errors
#[derive(Error, Debug)]
pub enum ProxyError {
    /// Backend is not available
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    /// No healthy backend found
    #[error("no healthy backend available")]
    NoHealthyBackend,

    /// Route not found
    #[error("route not found for host: {host}, path: {path}")]
    RouteNotFound {
        /// The requested host
        host: String,
        /// The requested path
        path: String,
    },

    /// Invalid request
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// Connection error
    #[error("connection error: {0}")]
    ConnectionError(String),

    /// Timeout
    #[error("request timeout")]
    Timeout,

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}

impl ProxyError {
    /// Returns the appropriate HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            ProxyError::BackendUnavailable(_) => 503,
            ProxyError::NoHealthyBackend => 503,
            ProxyError::RouteNotFound { .. } => 404,
            ProxyError::InvalidRequest(_) => 400,
            ProxyError::ConnectionError(_) => 502,
            ProxyError::Timeout => 504,
            ProxyError::Internal(_) => 500,
        }
    }
}

/// Result type alias using ProxyError
pub type Result<T> = std::result::Result<T, ProxyError>;

```

## File ./crates\core\src\lib.rs:
```rust
//! Apex Core - Hot path logic for reverse proxy
//!
//! This crate contains the performance-critical code paths.
//! 
//! # Invariants
//! 
//! 1. NO Mutex/RwLock in hot path
//! 2. NO allocation per-request (except arena)
//! 3. NO panic on user input

#![warn(missing_docs)]
#![warn(clippy::all)]
#![deny(unsafe_code)]

pub mod backend;
pub mod error;
pub mod router;

pub use backend::{Backend, BackendPool};
pub use error::ProxyError;
pub use router::{Route, RouteMatch, Router};

```

## File ./crates\core\src\router.rs:
```rust
//! Request routing with immutable table + ArcSwap
//!
//! Lock-free routing using simple linear scan.
//! With <100 routes, linear scan is cache-friendly and faster than DashMap.

use arc_swap::ArcSwap;
use std::sync::Arc;

use crate::backend::BackendPool;
use crate::error::{ProxyError, Result};

/// A route entry mapping host/path to backend pool
#[derive(Debug)]
pub struct Route {
    /// Host pattern (e.g., "api.example.com", "*" for any)
    pub host: String,

    /// Path prefix (e.g., "/api/v1")
    pub path_prefix: String,

    /// Backend pool for this route
    pub backends: Arc<BackendPool>,

    /// Strip path prefix before forwarding
    pub strip_prefix: bool,
}

impl Route {
    /// Create a new route
    pub fn new(host: String, path_prefix: String, backends: Arc<BackendPool>) -> Self {
        Self {
            host,
            path_prefix,
            backends,
            strip_prefix: false,
        }
    }

    /// Set strip prefix option
    pub fn with_strip_prefix(mut self, strip: bool) -> Self {
        self.strip_prefix = strip;
        self
    }

    /// Check if this route matches the given host
    #[inline]
    fn matches_host(&self, host: &str) -> bool {
        self.host == "*" || self.host.is_empty() || self.host == host
    }

    /// Check if this route matches the given path
    #[inline]
    fn matches_path(&self, path: &str) -> bool {
        self.path_prefix == "/" || path.starts_with(&self.path_prefix)
    }
}

/// Match result from router lookup
#[derive(Debug, Clone)]
pub struct RouteMatch<'a> {
    /// Matched route
    pub route: Arc<Route>,

    /// Original path (for rewriting by caller if needed)
    pub path: &'a str,
    
    /// Whether to strip prefix
    pub should_strip: bool,
}

/// Immutable routing table
struct RouterTable {
    /// All routes, sorted by specificity (longer path prefix first)
    routes: Vec<Arc<Route>>,
}

impl RouterTable {
    fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Find matching route using linear scan
    /// 
    /// # Performance
    /// O(n) where n = number of routes
    /// Cache-friendly sequential access
    #[inline]
    fn find<'a>(&self, host: &str, path: &'a str) -> Option<RouteMatch<'a>> {
        // Linear scan - cache friendly, fast for <100 routes
        for route in &self.routes {
            if route.matches_host(host) && route.matches_path(path) {
                return Some(RouteMatch {
                    route: Arc::clone(route),
                    path,
                    should_strip: route.strip_prefix && route.path_prefix != "/",
                });
            }
        }
        None
    }
}

/// Router for matching requests to backend pools
/// 
/// Uses ArcSwap for lock-free reads with atomic updates.
#[derive(Debug)]
pub struct Router {
    /// Immutable routing table (swapped atomically)
    table: ArcSwap<Vec<Arc<Route>>>,
}

impl Router {
    /// Create a new empty router
    pub fn new() -> Self {
        Self {
            table: ArcSwap::from_pointee(Vec::new()),
        }
    }

    /// Add a route to the router
    /// 
    /// Note: This clones the entire table. Fine for config reload,
    /// not meant for high-frequency updates.
    pub fn add_route(&self, route: Route) {
        let route = Arc::new(route);
        
        // Clone current table, add route, sort, swap
        let mut routes = (*self.table.load_full()).clone();
        routes.push(route);
        
        // Sort by specificity: longer path prefix = higher priority
        routes.sort_by(|a, b| {
            // First by host specificity ("*" is less specific)
            let host_cmp = match (&a.host == "*", &b.host == "*") {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Equal,
            };
            
            if host_cmp != std::cmp::Ordering::Equal {
                return host_cmp;
            }
            
            // Then by path length (longer = more specific)
            b.path_prefix.len().cmp(&a.path_prefix.len())
        });
        
        self.table.store(Arc::new(routes));
    }

    /// Find matching route for request
    /// 
    /// # Performance
    /// Lock-free read via ArcSwap::load()
    /// O(n) linear scan where n = number of routes
    #[inline]
    pub fn find<'a>(&self, host: &str, path: &'a str) -> Result<RouteMatch<'a>> {
        let routes = self.table.load();
        
        // Linear scan - cache friendly
        for route in routes.iter() {
            if route.matches_host(host) && route.matches_path(path) {
                return Ok(RouteMatch {
                    route: Arc::clone(route),
                    path,
                    should_strip: route.strip_prefix && route.path_prefix != "/",
                });
            }
        }

        Err(ProxyError::RouteNotFound {
            host: host.to_string(),
            path: path.to_string(),
        })
    }

    /// Clear all routes (for hot reload)
    pub fn clear(&self) {
        self.table.store(Arc::new(Vec::new()));
    }

    /// Get total route count
    pub fn route_count(&self) -> usize {
        self.table.load().len()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Backend;
    use std::net::SocketAddr;

    fn make_pool(port: u16) -> Arc<BackendPool> {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let backend = Arc::new(Backend::new(addr));
        Arc::new(BackendPool::from_backends(vec![backend]))
    }

    #[test]
    fn test_basic_routing() {
        let router = Router::new();

        router.add_route(Route::new(
            "api.example.com".to_string(),
            "/".to_string(),
            make_pool(8001),
        ));

        let result = router.find("api.example.com", "/users").unwrap();
        assert_eq!(result.route.host, "api.example.com");
    }

    #[test]
    fn test_path_prefix_matching() {
        let router = Router::new();

        router.add_route(Route::new(
            "example.com".to_string(),
            "/api/v1".to_string(),
            make_pool(8001),
        ));

        router.add_route(Route::new(
            "example.com".to_string(),
            "/api/v2".to_string(),
            make_pool(8002),
        ));

        let v1 = router.find("example.com", "/api/v1/users").unwrap();
        let v2 = router.find("example.com", "/api/v2/users").unwrap();

        assert_eq!(v1.route.path_prefix, "/api/v1");
        assert_eq!(v2.route.path_prefix, "/api/v2");
    }

    #[test]
    fn test_default_routes() {
        let router = Router::new();

        router.add_route(Route::new(
            "*".to_string(),
            "/".to_string(),
            make_pool(8000),
        ));

        // Should match any host
        let result = router.find("unknown.example.com", "/anything").unwrap();
        assert_eq!(result.route.host, "*");
    }

    #[test]
    fn test_route_not_found() {
        let router = Router::new();

        router.add_route(Route::new(
            "api.example.com".to_string(),
            "/".to_string(),
            make_pool(8001),
        ));

        let result = router.find("other.example.com", "/");
        assert!(result.is_err());
    }
}

```

## File ./crates\server\src\backend_task.rs:
```rust
//! BackendTask - persistent connection per backend
//!
//! Each backend has a dedicated task that:
//! - Owns a single persistent HTTP/1.1 connection
//! - Receives requests via mpsc channel
//! - Sends responses back via oneshot channel
//!
//! BackendHandlePool - multiple connections for parallelism
//! - Round-robin across multiple BackendHandle
//! - Lock-free via atomic counter
//!
//! NO Mutex. NO pool. NO hyper-util Client.
//! This is the "protocol engine" model, same as echo server.

use hyper::body::Incoming;
use hyper::client::conn::http1::{self, SendRequest};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

/// Request to send to backend
pub struct BackendRequest {
    /// The HTTP request to forward
    pub request: Request<Incoming>,
    /// Channel to send response back
    pub response_tx: oneshot::Sender<Result<Response<Incoming>, BackendError>>,
}

/// Error from backend task
#[derive(Debug, Clone)]
pub enum BackendError {
    /// Connection failed
    ConnectionFailed(String),
    /// Request failed
    RequestFailed(String),
    /// Channel closed
    ChannelClosed,
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::ConnectionFailed(e) => write!(f, "connection failed: {}", e),
            BackendError::RequestFailed(e) => write!(f, "request failed: {}", e),
            BackendError::ChannelClosed => write!(f, "channel closed"),
        }
    }
}

impl std::error::Error for BackendError {}

/// Handle to send requests to a backend task
#[derive(Clone)]
pub struct BackendHandle {
    /// Channel to send requests
    tx: mpsc::Sender<BackendRequest>,
    /// Backend address (for error messages)
    pub addr: SocketAddr,
}

impl BackendHandle {
    /// Send a request to the backend and wait for response
    #[inline]
    pub async fn send(&self, request: Request<Incoming>) -> Result<Response<Incoming>, BackendError> {
        let (response_tx, response_rx) = oneshot::channel();

        let backend_req = BackendRequest {
            request,
            response_tx,
        };

        // Send to backend task
        self.tx.send(backend_req).await
            .map_err(|_| BackendError::ChannelClosed)?;

        // Wait for response
        response_rx.await
            .map_err(|_| BackendError::ChannelClosed)?
    }
}

/// Pool of multiple BackendHandle for parallelism
/// Round-robin across connections to avoid serial bottleneck
pub struct BackendHandlePool {
    /// Multiple handles to the same backend
    handles: Vec<BackendHandle>,
    /// Round-robin counter (lock-free)
    counter: AtomicUsize,
    /// Backend address
    pub addr: SocketAddr,
}

impl BackendHandlePool {
    /// Create a pool with N connections to the backend
    pub fn new(addr: SocketAddr, num_connections: usize, buffer_size: usize) -> Self {
        let handles: Vec<_> = (0..num_connections)
            .map(|_| spawn_backend_task(addr, buffer_size))
            .collect();

        Self {
            handles,
            counter: AtomicUsize::new(0),
            addr,
        }
    }

    /// Send a request using round-robin connection selection
    #[inline]
    pub async fn send(&self, request: Request<Incoming>) -> Result<Response<Incoming>, BackendError> {
        // Lock-free round-robin
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % self.handles.len();
        self.handles[idx].send(request).await
    }
}

/// Spawn a backend task that owns a persistent connection
pub fn spawn_backend_task(addr: SocketAddr, buffer_size: usize) -> BackendHandle {
    let (tx, rx) = mpsc::channel::<BackendRequest>(buffer_size);

    tokio::spawn(backend_task_loop(addr, rx));

    BackendHandle { tx, addr }
}

/// The backend task loop - owns connection, processes requests
async fn backend_task_loop(
    addr: SocketAddr,
    mut rx: mpsc::Receiver<BackendRequest>,
) {
    let mut sender: Option<SendRequest<Incoming>> = None;

    while let Some(req) = rx.recv().await {
        // Ensure we have a working connection
        let s = match ensure_connection(&mut sender, addr).await {
            Ok(s) => s,
            Err(e) => {
                let _ = req.response_tx.send(Err(BackendError::ConnectionFailed(e.to_string())));
                continue;
            }
        };

        // Send request
        match s.send_request(req.request).await {
            Ok(response) => {
                let _ = req.response_tx.send(Ok(response));
            }
            Err(e) => {
                // Connection broken, clear it for reconnect
                sender = None;
                let _ = req.response_tx.send(Err(BackendError::RequestFailed(e.to_string())));
            }
        }
    }
}

/// Ensure we have a ready connection, reconnect if needed
async fn ensure_connection(
    sender: &mut Option<SendRequest<Incoming>>,
    addr: SocketAddr,
) -> Result<&mut SendRequest<Incoming>, std::io::Error> {
    // Check if existing connection is still ready
    if let Some(ref s) = sender {
        if s.is_ready() {
            return Ok(sender.as_mut().unwrap());
        }
    }

    // Need to create new connection
    let stream = TcpStream::connect(addr).await?;
    stream.set_nodelay(true)?;

    let io = TokioIo::new(stream);

    let (new_sender, conn) = http1::handshake(io)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Spawn connection driver
    tokio::spawn(async move {
        let _ = conn.await;
    });

    *sender = Some(new_sender);
    Ok(sender.as_mut().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backend_handle_creation() {
        let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let handle = spawn_backend_task(addr, 100);
        assert_eq!(handle.addr, addr);
    }
}

```

## File ./crates\server\src\client.rs:
```rust
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

```

## File ./crates\server\src\handler.rs:
```rust
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

```

## File ./crates\server\src\http2_client.rs:
```rust
//! HTTP/2 client with multiplexed connections
//!
//! Key features:
//! - Single HTTP/2 connection per backend handles many concurrent requests
//! - Lock-free sender cloning for zero contention
//! - Auto-reconnect on connection failure

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::RwLock;

use apex_core::Backend;
use crate::client::ClientError;

type Sender = client_http2::SendRequest<Full<Bytes>>;

/// HTTP/2 connection to a single backend
struct Http2Connection {
    sender: RwLock<Option<Sender>>,
    addr: SocketAddr,
}

impl Http2Connection {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: RwLock::new(None),
            addr,
        }
    }

    /// Get or create HTTP/2 sender (connection)
    async fn get_sender(&self) -> Result<Sender, ClientError> {
        // Fast path: check if connection ready (read lock)
        {
            let guard = self.sender.read().await;
            if let Some(ref s) = *guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Slow path: create new connection (write lock)
        let mut guard = self.sender.write().await;

        // Double-check after acquiring write lock
        if let Some(ref s) = *guard {
            if s.is_ready() {
                return Ok(s.clone());
            }
        }

        // Create new HTTP/2 connection
        let stream = TcpStream::connect(self.addr)
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| ClientError::Connection(format!("HTTP/2 handshake failed: {}", e)))?;

        // Spawn connection driver
        tokio::spawn(async move {
            let _ = conn.await;
        });

        *guard = Some(sender.clone());
        Ok(sender)
    }
}

/// HTTP/2 client pool - manages connections to multiple backends
pub struct Http2Client {
    /// Connections per backend address
    connections: RwLock<HashMap<SocketAddr, Arc<Http2Connection>>>,
    timeout: Duration,
}

impl Http2Client {
    /// Create a new HTTP/2 client
    pub fn new(timeout: Duration) -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            timeout,
        }
    }

    /// Get or create connection for backend
    async fn get_connection(&self, addr: SocketAddr) -> Arc<Http2Connection> {
        // Fast path: check if connection exists
        {
            let guard = self.connections.read().await;
            if let Some(conn) = guard.get(&addr) {
                return conn.clone();
            }
        }

        // Slow path: create new connection entry
        let mut guard = self.connections.write().await;

        // Double-check
        if let Some(conn) = guard.get(&addr) {
            return conn.clone();
        }

        let conn = Arc::new(Http2Connection::new(addr));
        guard.insert(addr, conn.clone());
        conn
    }

    /// Forward request to backend using HTTP/2
    pub async fn forward(
        &self,
        backend: &Backend,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ClientError> {
        backend.inc_connections();

        let conn = self.get_connection(backend.addr).await;

        // Build forward request
        let path = req.uri().path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        let uri: hyper::Uri = format!("http://{}{}", backend.addr, path)
            .parse()
            .map_err(|e| ClientError::Request(format!("invalid URI: {}", e)))?;

        // Collect body
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        let forward_req = Request::builder()
            .method(parts.method)
            .uri(uri)
            .body(Full::new(body_bytes))
            .unwrap();

        // Get sender and send request
        let result = tokio::time::timeout(self.timeout, async {
            let mut sender = conn.get_sender().await?;
            sender.send_request(forward_req)
                .await
                .map_err(|e| ClientError::Request(e.to_string()))
        })
        .await
        .map_err(|_| ClientError::Timeout)?;

        backend.dec_connections();

        match result {
            Ok(resp) => {
                backend.inc_requests();
                // Convert response body
                let (parts, body) = resp.into_parts();
                let body_bytes = body.collect()
                    .await
                    .map(|b| b.to_bytes())
                    .unwrap_or_default();
                Ok(Response::from_parts(parts, Full::new(body_bytes)))
            }
            Err(e) => Err(e),
        }
    }
}

impl Default for Http2Client {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

```

## File ./crates\server\src\http2_client_lockfree.rs:
```rust
//! HTTP/2 client with lock-free multiplexed connections
//!
//! Optimizations:
//! - ArcSwap for lock-free sender access  
//! - DashMap for lock-free connection pool
//! - Minimal allocations in hot path

use arc_swap::ArcSwap;
use bytes::Bytes;
use dashmap::DashMap;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http2 as client_http2;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use apex_core::Backend;
use crate::client::ClientError;

type Sender = client_http2::SendRequest<Full<Bytes>>;

/// Lock-free HTTP/2 connection to a single backend
struct Http2Connection {
    /// ArcSwap for lock-free reads, only lock on reconnect
    sender: ArcSwap<Option<Sender>>,
    /// Mutex only for connection creation (rare)
    init_lock: Mutex<()>,
    addr: SocketAddr,
}

impl Http2Connection {
    fn new(addr: SocketAddr) -> Self {
        Self {
            sender: ArcSwap::new(Arc::new(None)),
            init_lock: Mutex::new(()),
            addr,
        }
    }

    /// Get sender - lock-free fast path
    #[inline]
    async fn get_sender(&self) -> Result<Sender, ClientError> {
        // Fast path: lock-free read
        {
            let guard = self.sender.load();
            if let Some(ref s) = **guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Slow path: acquire init lock for connection creation
        let _lock = self.init_lock.lock().await;

        // Double-check after lock
        {
            let guard = self.sender.load();
            if let Some(ref s) = **guard {
                if s.is_ready() {
                    return Ok(s.clone());
                }
            }
        }

        // Create new connection
        let stream = TcpStream::connect(self.addr)
            .await
            .map_err(|e| ClientError::Connection(e.to_string()))?;
        stream.set_nodelay(true).ok();
        let io = TokioIo::new(stream);

        let (sender, conn) = client_http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| ClientError::Connection(format!("HTTP/2 handshake: {}", e)))?;

        // Spawn connection driver
        tokio::spawn(async move { let _ = conn.await; });

        // Store atomically
        self.sender.store(Arc::new(Some(sender.clone())));
        Ok(sender)
    }
}

/// Lock-free HTTP/2 client pool
pub struct Http2ClientLockFree {
    /// DashMap for lock-free connection lookup
    connections: DashMap<SocketAddr, Arc<Http2Connection>>,
    timeout: Duration,
}

impl Http2ClientLockFree {
    /// Create a new lock-free HTTP/2 client
    pub fn new(timeout: Duration) -> Self {
        Self {
            connections: DashMap::new(),
            timeout,
        }
    }

    /// Get or create connection - lock-free
    #[inline]
    fn get_connection(&self, addr: SocketAddr) -> Arc<Http2Connection> {
        // Fast path: existing connection
        if let Some(conn) = self.connections.get(&addr) {
            return conn.clone();
        }

        // Slow path: create new (DashMap handles concurrent inserts)
        self.connections
            .entry(addr)
            .or_insert_with(|| Arc::new(Http2Connection::new(addr)))
            .clone()
    }

    /// Forward request to backend using HTTP/2 - optimized
    #[inline]
    pub async fn forward(
        &self,
        backend: &Backend,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, ClientError> {
        backend.inc_connections();

        let conn = self.get_connection(backend.addr);

        // Build URI without format! allocation
        let path = req.uri().path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        
        // Use authority from backend (pre-computed)
        let uri = hyper::Uri::builder()
            .scheme("http")
            .authority(backend.authority.as_str())
            .path_and_query(path)
            .build()
            .map_err(|_| ClientError::Request("invalid URI".into()))?;

        // Collect body (required for HTTP/2 framing)
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        let forward_req = Request::builder()
            .method(parts.method)
            .uri(uri)
            .body(Full::new(body_bytes))
            .map_err(|_| ClientError::Request("build request failed".into()))?;

        // Get sender and send request with timeout
        let result = tokio::time::timeout(self.timeout, async {
            let mut sender = conn.get_sender().await?;
            sender.send_request(forward_req)
                .await
                .map_err(|e| ClientError::Request(e.to_string()))
        })
        .await
        .map_err(|_| ClientError::Timeout)?;

        backend.dec_connections();

        match result {
            Ok(resp) => {
                backend.inc_requests();
                let (parts, body) = resp.into_parts();
                let body_bytes = body.collect()
                    .await
                    .map(|b| b.to_bytes())
                    .unwrap_or_default();
                Ok(Response::from_parts(parts, Full::new(body_bytes)))
            }
            Err(e) => Err(e),
        }
    }
}

impl Default for Http2ClientLockFree {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

```

## File ./crates\server\src\http2_handler.rs:
```rust
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

use crate::proxy::ProxyService;

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

    /// Run the HTTP/2 server
    pub async fn run(self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        tracing::info!("Apex HTTP/2 listening on {}", self.listen_addr);

        loop {
            let (stream, _remote_addr) = listener.accept().await?;
            stream.set_nodelay(true)?;
            let io = TokioIo::new(stream);
            let proxy = Arc::clone(&self.proxy);

            tokio::spawn(async move {
                let service = service_fn(|req: Request<Incoming>| {
                    let proxy = Arc::clone(&proxy);
                    async move { handle_request_h2(proxy, req).await }
                });

                if let Err(err) = http2::Builder::new(TokioExecutor::new())
                    .serve_connection(io, service)
                    .await
                {
                    if !is_connection_closed_error(&err) {
                        tracing::error!("HTTP/2 connection error: {}", err);
                    }
                }
            });
        }
    }
}

/// Handle a single request - returns Full<Bytes> response
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

/// Check if error is just a closed connection
fn is_connection_closed_error<E: std::fmt::Display>(err: &E) -> bool {
    let msg = err.to_string();
    msg.contains("connection closed")
        || msg.contains("broken pipe")
        || msg.contains("reset by peer")
}

```

## File ./crates\server\src\lib.rs:
```rust
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

pub use handler::ProxyHandler;
pub use http2_client::Http2Client;
pub use http2_client_lockfree::Http2ClientLockFree;
pub use http2_handler::Http2Handler;
pub use proxy::ProxyService;

```

## File ./crates\server\src\pool.rs:
```rust
//! Per-worker connection pool - minimal overhead
//!
//! Simple connection creation without pooling for now.
//! Hyper's HTTP/1.1 keep-alive handles connection reuse at TCP level.

use hyper::body::Incoming;
use hyper::client::conn::http1::{self, SendRequest};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpStream;

/// Create new connection to backend
async fn create_connection(addr: SocketAddr) -> std::io::Result<SendRequest<Incoming>> {
    let stream = TcpStream::connect(addr).await?;
    stream.set_nodelay(true)?;
    
    let io = TokioIo::new(stream);
    
    let (sender, conn) = http1::handshake(io)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Spawn connection driver - runs in background, keeps connection alive
    tokio::spawn(async move {
        let _ = conn.await;
    });

    Ok(sender)
}

/// Send request - creates connection per request for now
/// 
/// This is still faster than hyper-util legacy client because:
/// - No trait objects
/// - No internal locking
/// - Direct path to send_request
#[inline]
pub async fn send_pooled(
    addr: SocketAddr,
    req: Request<Incoming>,
) -> Result<Response<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
    let mut sender = create_connection(addr).await?;
    let response = sender.send_request(req).await?;
    Ok(response)
}

```

## File ./crates\server\src\proxy.rs:
```rust
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

/// Backend protocol mode
#[derive(Clone, Copy, Debug, Default)]
pub enum BackendProtocol {
    /// HTTP/1.1 with connection pooling
    #[default]
    Http1,
    /// HTTP/2 with multiplexing (higher throughput)
    Http2,
}

/// Proxy service that routes requests to backends
pub struct ProxyService {
    /// Router for matching requests
    router: Arc<Router>,

    /// HTTP/1.1 client for backend connections
    http1_client: HttpClient,

    /// HTTP/2 client for backend connections (lock-free)
    http2_client: Http2ClientLockFree,

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

    /// Create a new proxy service with specified protocol
    pub fn from_config_with_protocol(config: &ApexConfig, protocol: BackendProtocol) -> Self {
        let router = Arc::new(Router::new());
        let timeout = std::time::Duration::from_secs(config.server.timeout_secs);
        let http1_client = HttpClient::new(timeout);
        let http2_client = Http2ClientLockFree::new(timeout);

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

        Self { router, http1_client, http2_client, protocol }
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

```

## Cargo.toml dependencies:
```toml
resolver = "2"
members = [
version = "0.1.0"
edition = "2021"
rust-version = "1.82"
license = "MIT OR Apache-2.0"
repository = "https://github.com/example/apex"
authors = ["Apex Team"]
tokio = { version = "1", features = ["full"] }
hyper = { version = "1", features = ["full"] }
hyper-util = { version = "0.1", features = ["full"] }
http-body-util = "0.1"
http = "1"
bytes = "1"
rustls = { version = "0.23", default-features = false }
tokio-rustls = "0.26"
arc-swap = "1"
dashmap = "6"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"
anyhow = "1"
apex-core = { path = "crates/core" }
apex-config = { path = "crates/config" }
apex-server = { path = "crates/server" }
lto = true
codegen-units = 1
panic = "abort"
strip = true
lto = true
codegen-units = 1
```

