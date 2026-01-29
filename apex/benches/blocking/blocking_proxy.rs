//! Blocking HTTP/1.1 Proxy - Pure Rust, no external dependencies
//!
//! Uses only std library to test theoretical maximum without async overhead.
//! This is a simple proxy that forwards requests to a backend.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Global request counter
static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);
static ERROR_COUNT: AtomicU64 = AtomicU64::new(0);

/// Backend address
const BACKEND: &str = "127.0.0.1:9001";

fn main() {
    let listen_addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();

    println!("=== Blocking HTTP/1.1 Proxy ===");
    println!("Listening on: {}", listen_addr);
    println!("Backend: {}", BACKEND);
    println!("Threads: per-connection");
    println!();

    // Start metrics thread
    thread::spawn(|| {
        let mut prev = 0u64;
        loop {
            thread::sleep(Duration::from_secs(1));
            let current = REQUEST_COUNT.load(Ordering::Relaxed);
            let errors = ERROR_COUNT.load(Ordering::Relaxed);
            let rps = current - prev;
            prev = current;
            if rps > 0 || errors > 0 {
                println!("RPS: {} (errors: {})", rps, errors);
            }
        }
    });

    let listener = TcpListener::bind(listen_addr).expect("Failed to bind");

    for stream in listener.incoming() {
        match stream {
            Ok(client) => {
                thread::spawn(move || {
                    if let Err(_) = handle_connection(client) {
                        ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
                    }
                });
            }
            Err(_) => {
                ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Handle a single HTTP/1.1 connection (may have multiple requests via keep-alive)
fn handle_connection(mut client: TcpStream) -> std::io::Result<()> {
    client.set_nodelay(true)?;
    client.set_read_timeout(Some(Duration::from_secs(30)))?;
    client.set_write_timeout(Some(Duration::from_secs(30)))?;

    let mut reader = BufReader::new(client.try_clone()?);

    loop {
        // Read request line
        let mut request_line = String::new();
        let bytes_read = reader.read_line(&mut request_line)?;
        if bytes_read == 0 {
            break; // Connection closed
        }

        // Parse request line: "GET /path HTTP/1.1\r\n"
        let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
        if parts.len() < 3 {
            break;
        }

        let method = parts[0];
        let path = parts[1];
        let _version = parts[2];

        // Read headers
        let mut content_length: usize = 0;
        let mut keep_alive = true;

        loop {
            let mut header_line = String::new();
            reader.read_line(&mut header_line)?;
            if header_line == "\r\n" || header_line == "\n" {
                break;
            }

            let lower = header_line.to_lowercase();
            if lower.starts_with("content-length:") {
                content_length = lower
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
            }
            if lower.starts_with("connection:") && lower.contains("close") {
                keep_alive = false;
            }
        }

        // Read body if present
        let mut body = vec![0u8; content_length];
        if content_length > 0 {
            reader.read_exact(&mut body)?;
        }

        // Forward to backend
        let response = forward_to_backend(method, path, &body)?;

        // Send response back to client
        client.write_all(&response)?;

        REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);

        if !keep_alive {
            break;
        }
    }

    Ok(())
}

/// Forward request to backend and get response
fn forward_to_backend(method: &str, path: &str, body: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut backend = TcpStream::connect(BACKEND)?;
    backend.set_nodelay(true)?;
    backend.set_read_timeout(Some(Duration::from_secs(10)))?;

    // Build request with Connection: close to ensure backend closes
    let request = if body.is_empty() {
        format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            method, path, BACKEND
        )
    } else {
        format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            method, path, BACKEND, body.len()
        )
    };

    backend.write_all(request.as_bytes())?;
    if !body.is_empty() {
        backend.write_all(body)?;
    }

    // Parse response properly (don't rely on connection close)
    let mut reader = BufReader::new(backend);

    // Read status line
    let mut status_line = String::new();
    reader.read_line(&mut status_line)?;

    // Read headers
    let mut headers = String::new();
    let mut content_length: usize = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        headers.push_str(&header);
        if header == "\r\n" || header == "\n" {
            break;
        }
        if header.to_lowercase().starts_with("content-length:") {
            content_length = header
                .split(':')
                .nth(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
        }
    }

    // Read body
    let mut body_data = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body_data)?;
    }

    // Build response
    let mut response = Vec::with_capacity(status_line.len() + headers.len() + body_data.len());
    response.extend_from_slice(status_line.as_bytes());
    response.extend_from_slice(headers.as_bytes());
    response.extend_from_slice(&body_data);

    Ok(response)
}
