//! HTTP/2 benchmark tool v2 - No lock per request

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::client::conn::http2;
use hyper::Request;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().collect();

    let url = args.get(1).map(|s| s.as_str()).unwrap_or("http://127.0.0.1:8080/");
    let concurrency: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    let url_parsed: hyper::Uri = url.parse()?;
    let host = url_parsed.host().unwrap_or("127.0.0.1");
    let port = url_parsed.port_u16().unwrap_or(8080);
    let addr = format!("{}:{}", host, port);

    println!("HTTP/2 Benchmark v2 (no-lock)");
    println!("URL: {}", url);
    println!("Concurrency: {}", concurrency);
    println!("Duration: {}s", duration_secs);
    println!("Target: {}", addr);
    println!();

    // Create single HTTP/2 connection
    let stream = TcpStream::connect(&addr).await?;
    stream.set_nodelay(true)?;
    let io = TokioIo::new(stream);

    let (sender, conn) = http2::handshake(TokioExecutor::new(), io).await?;

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("Connection error: {}", e);
        }
    });

    // Clone sender once per worker (no lock needed - HTTP/2 allows concurrent sends)
    let senders: Vec<_> = (0..concurrency).map(|_| sender.clone()).collect();

    let success_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    let total_latency_us = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);

    // Spawn RPS reporter
    let success_clone = Arc::clone(&success_count);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut prev = 0u64;
        loop {
            interval.tick().await;
            let current = success_clone.load(Ordering::Relaxed);
            println!("RPS: {}", current - prev);
            prev = current;
        }
    });

    let mut handles = vec![];

    // Spawn worker tasks
    for mut sender in senders {
        let url = url.to_string();
        let success = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);
        let latency = Arc::clone(&total_latency_us);
        let running = Arc::clone(&running);

        let handle = tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                let req_start = Instant::now();
                let req = Request::builder()
                    .uri(&url)
                    .body(Empty::<Bytes>::new())
                    .unwrap();

                match sender.send_request(req).await {
                    Ok(resp) => {
                        let _ = resp.into_body().collect().await;
                        success.fetch_add(1, Ordering::Relaxed);
                        latency.fetch_add(req_start.elapsed().as_micros() as u64, Ordering::Relaxed);
                    }
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait
    tokio::time::sleep(duration).await;
    running.store(false, Ordering::Relaxed);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let elapsed = start.elapsed();
    let success = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let total_latency = total_latency_us.load(Ordering::Relaxed);
    let avg_latency = if success > 0 { total_latency / success } else { 0 };

    println!();
    println!("=== Results ===");
    println!("Duration: {:.2}s", elapsed.as_secs_f64());
    println!("Total requests: {}", success + errors);
    println!("Successful: {}", success);
    println!("Errors: {}", errors);
    println!("RPS: {:.0}", success as f64 / elapsed.as_secs_f64());
    println!("Avg latency: {}μs", avg_latency);

    Ok(())
}
