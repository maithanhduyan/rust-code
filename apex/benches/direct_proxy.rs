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
