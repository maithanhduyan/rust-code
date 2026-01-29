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
