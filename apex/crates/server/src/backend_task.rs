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
