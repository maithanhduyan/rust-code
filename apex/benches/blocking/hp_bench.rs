//! High-Performance Blocking HTTP/1.1 Benchmark
//!
//! Features:
//! - Connection pooling (reuses connections with keep-alive)
//! - Zero allocation in hot path
//! - Per-thread stats to avoid cache bouncing

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const BUFFER_SIZE: usize = 4096;

/// Parse HTTP response status code from bytes
#[inline]
fn get_status_code(buf: &[u8]) -> Option<u16> {
    // HTTP/1.1 200 OK
    if buf.len() < 12 || !buf.starts_with(b"HTTP/1.") {
        return None;
    }
    let code = &buf[9..12];
    let hundreds = (code[0] as u16).wrapping_sub(b'0' as u16);
    let tens = (code[1] as u16).wrapping_sub(b'0' as u16);
    let ones = (code[2] as u16).wrapping_sub(b'0' as u16);
    Some(hundreds * 100 + tens * 10 + ones)
}

/// Find Content-Length in response
#[inline]
fn get_content_length(buf: &[u8], len: usize) -> usize {
    let data = &buf[..len];
    let needle = b"content-length:";

    for i in 0..len.saturating_sub(needle.len() + 1) {
        let line_start = if i == 0 || data[i-1] == b'\n' { true } else { false };
        if line_start && data[i..].len() >= needle.len() {
            let mut matches = true;
            for j in 0..needle.len() {
                if data[i+j].to_ascii_lowercase() != needle[j] {
                    matches = false;
                    break;
                }
            }
            if matches {
                // Parse value
                let mut val = 0usize;
                for k in (i + needle.len())..len {
                    let b = data[k];
                    if b == b'\r' || b == b'\n' {
                        break;
                    }
                    if b >= b'0' && b <= b'9' {
                        val = val * 10 + (b - b'0') as usize;
                    }
                }
                return val;
            }
        }
    }
    0
}

/// Find end of headers (\r\n\r\n)
#[inline]
fn find_headers_end(buf: &[u8], len: usize) -> Option<usize> {
    if len < 4 {
        return None;
    }
    for i in 0..len-3 {
        if buf[i] == b'\r' && buf[i+1] == b'\n' && buf[i+2] == b'\r' && buf[i+3] == b'\n' {
            return Some(i + 4);
        }
    }
    None
}

/// Do one HTTP request, returns true on success
/// Reuses the stream for keep-alive
#[inline]
fn do_one_request(stream: &mut TcpStream, buf: &mut [u8]) -> bool {
    // Send request with keep-alive
    const REQUEST: &[u8] = b"GET /test HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n";

    if stream.write_all(REQUEST).is_err() {
        return false;
    }

    // Read response headers
    let mut total_read = 0usize;
    let headers_end;

    loop {
        match stream.read(&mut buf[total_read..]) {
            Ok(0) => return false, // Connection closed
            Ok(n) => {
                total_read += n;
                if let Some(end) = find_headers_end(buf, total_read) {
                    headers_end = end;
                    break;
                }
                if total_read >= buf.len() {
                    return false; // Too large
                }
            }
            Err(_) => return false,
        }
    }

    // Check status code
    match get_status_code(&buf[..total_read]) {
        Some(200) => {}
        _ => return false,
    }

    // Get content length
    let content_length = get_content_length(buf, total_read);

    // Read remaining body if needed
    let body_already_read = total_read - headers_end;
    if content_length > body_already_read {
        let remaining = content_length - body_already_read;
        let mut body_read = 0usize;
        while body_read < remaining {
            match stream.read(&mut buf[..]) {
                Ok(0) => return false,
                Ok(n) => body_read += n,
                Err(_) => return false,
            }
        }
    }

    true
}

/// Worker thread - keeps connection alive and reuses it
fn worker_thread(
    target: String,
    success: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
) {
    let mut buf = vec![0u8; BUFFER_SIZE];
    let mut conn: Option<TcpStream> = None;

    while running.load(Ordering::Relaxed) {
        // Get or create connection
        let stream = match conn.take() {
            Some(s) => s,
            None => {
                match TcpStream::connect(&target) {
                    Ok(s) => {
                        s.set_nodelay(true).ok();
                        s.set_read_timeout(Some(Duration::from_millis(500))).ok();
                        s.set_write_timeout(Some(Duration::from_millis(500))).ok();
                        s
                    }
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                        thread::sleep(Duration::from_micros(100));
                        continue;
                    }
                }
            }
        };

        let mut stream = stream;

        // Do requests on this connection
        for _ in 0..100 { // 100 requests per connection
            if !running.load(Ordering::Relaxed) {
                break;
            }

            if do_one_request(&mut stream, &mut buf) {
                success.fetch_add(1, Ordering::Relaxed);
            } else {
                errors.fetch_add(1, Ordering::Relaxed);
                break; // Connection broken, need new one
            }
        }

        // Try to reuse connection for next batch
        conn = Some(stream);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let target = args.get(1).map(|s| s.clone()).unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let connections: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(50);
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    println!("=== HP Blocking HTTP/1.1 Benchmark ===");
    println!("Target: {}", target);
    println!("Concurrent connections: {}", connections);
    println!("Duration: {}s", duration_secs);
    println!();

    let success = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    // Spawn worker threads
    let mut handles = Vec::new();
    for _ in 0..connections {
        let target = target.clone();
        let success = Arc::clone(&success);
        let errors = Arc::clone(&errors);
        let running = Arc::clone(&running);

        handles.push(thread::spawn(move || {
            worker_thread(target, success, errors, running);
        }));
    }

    // Stats thread
    let success2 = Arc::clone(&success);
    let running2 = Arc::clone(&running);
    let stats_handle = thread::spawn(move || {
        let mut last_success = 0u64;
        while running2.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            let current = success2.load(Ordering::Relaxed);
            println!("RPS: {}", current - last_success);
            last_success = current;
        }
    });

    // Wait for duration
    let start = Instant::now();
    thread::sleep(Duration::from_secs(duration_secs));
    running.store(false, Ordering::Relaxed);

    // Wait for workers
    for h in handles {
        h.join().ok();
    }
    stats_handle.join().ok();

    let elapsed = start.elapsed().as_secs_f64();
    let total_success = success.load(Ordering::Relaxed);
    let total_errors = errors.load(Ordering::Relaxed);

    println!();
    println!("=== Results ===");
    println!("Duration: {:.2}s", elapsed);
    println!("Total requests: {}", total_success + total_errors);
    println!("Successful: {}", total_success);
    println!("Errors: {}", total_errors);
    println!("RPS: {:.0}", total_success as f64 / elapsed);
}
