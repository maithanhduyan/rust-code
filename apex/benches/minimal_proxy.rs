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
