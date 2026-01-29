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
