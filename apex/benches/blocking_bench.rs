//! Blocking HTTP/1.1 Benchmark - Pure Rust, no external dependencies
//!
//! Stress test for blocking proxy

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let target = args.get(1).map(|s| s.as_str()).unwrap_or("127.0.0.1:8080");
    let threads: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    println!("=== Blocking HTTP/1.1 Benchmark ===");
    println!("Target: {}", target);
    println!("Threads: {}", threads);
    println!("Duration: {}s", duration_secs);
    println!();

    let success = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    // RPS reporter
    let success_clone = Arc::clone(&success);
    let running_clone = Arc::clone(&running);
    thread::spawn(move || {
        let mut prev = 0u64;
        while running_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            let current = success_clone.load(Ordering::Relaxed);
            println!("RPS: {}", current - prev);
            prev = current;
        }
    });

    let start = Instant::now();
    let mut handles = vec![];

    // Spawn worker threads
    for _ in 0..threads {
        let target = target.to_string();
        let success = Arc::clone(&success);
        let errors = Arc::clone(&errors);
        let running = Arc::clone(&running);

        let handle = thread::spawn(move || {
            // Each thread creates one persistent connection
            let mut conn: Option<(TcpStream, BufReader<TcpStream>)> = None;

            while running.load(Ordering::Relaxed) {
                // Get or create connection
                let (stream, reader) = match conn.take() {
                    Some(c) => c,
                    None => {
                        match TcpStream::connect(&target) {
                            Ok(s) => {
                                s.set_nodelay(true).ok();
                                s.set_read_timeout(Some(Duration::from_secs(5))).ok();
                                let r = BufReader::new(s.try_clone().unwrap());
                                (s, r)
                            }
                            Err(_) => {
                                errors.fetch_add(1, Ordering::Relaxed);
                                thread::sleep(Duration::from_millis(10));
                                continue;
                            }
                        }
                    }
                };

                // Send request
                let request = "GET /test HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n";

                let mut stream_mut = stream;
                let mut reader_mut = reader;

                if stream_mut.write_all(request.as_bytes()).is_err() {
                    errors.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // Read response
                let mut response_line = String::new();
                if reader_mut.read_line(&mut response_line).is_err() {
                    errors.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // Check for 200 OK
                if !response_line.contains("200") {
                    errors.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // Read headers
                let mut content_length: usize = 0;
                loop {
                    let mut header = String::new();
                    if reader_mut.read_line(&mut header).is_err() {
                        break;
                    }
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
                    if reader_mut.read_exact(&mut body).is_err() {
                        errors.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                }

                success.fetch_add(1, Ordering::Relaxed);

                // Reuse connection
                conn = Some((stream_mut, reader_mut));
            }
        });

        handles.push(handle);
    }

    // Wait for duration
    thread::sleep(Duration::from_secs(duration_secs));
    running.store(false, Ordering::Relaxed);

    // Wait for threads
    for h in handles {
        let _ = h.join();
    }

    let elapsed = start.elapsed();
    let total_success = success.load(Ordering::Relaxed);
    let total_errors = errors.load(Ordering::Relaxed);

    println!();
    println!("=== Results ===");
    println!("Duration: {:.2}s", elapsed.as_secs_f64());
    println!("Total requests: {}", total_success + total_errors);
    println!("Successful: {}", total_success);
    println!("Errors: {}", total_errors);
    println!("RPS: {:.0}", total_success as f64 / elapsed.as_secs_f64());
}
