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
