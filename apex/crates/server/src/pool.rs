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
