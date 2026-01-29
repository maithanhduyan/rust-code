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
