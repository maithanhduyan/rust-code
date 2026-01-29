//! Blocking Echo Server - Pure Rust, no external dependencies
//!
//! Simple HTTP/1.1 echo server for testing blocking proxy

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

fn main() {
    let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
    
    println!("=== Blocking Echo Server ===");
    println!("Listening on: {}", addr);
    println!();

    // Metrics thread
    thread::spawn(|| {
        let mut prev = 0u64;
        loop {
            thread::sleep(Duration::from_secs(1));
            let current = REQUEST_COUNT.load(Ordering::Relaxed);
            let rps = current - prev;
            prev = current;
            if rps > 0 {
                println!("Echo RPS: {}", rps);
            }
        }
    });

    let listener = TcpListener::bind(addr).expect("Failed to bind");

    for stream in listener.incoming() {
        if let Ok(client) = stream {
            thread::spawn(move || {
                let _ = handle_client(client);
            });
        }
    }
}

fn handle_client(mut client: TcpStream) -> std::io::Result<()> {
    client.set_nodelay(true)?;
    client.set_read_timeout(Some(Duration::from_secs(30)))?;

    let mut reader = BufReader::new(client.try_clone()?);

    loop {
        // Read request line
        let mut request_line = String::new();
        let bytes_read = reader.read_line(&mut request_line)?;
        if bytes_read == 0 {
            break;
        }

        // Read headers until empty line
        let mut content_length: usize = 0;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
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
        if content_length > 0 {
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body)?;
        }

        // Send response
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nOK";
        client.write_all(response.as_bytes())?;

        REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    Ok(())
}
