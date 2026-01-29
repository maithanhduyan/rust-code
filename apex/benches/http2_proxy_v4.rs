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
