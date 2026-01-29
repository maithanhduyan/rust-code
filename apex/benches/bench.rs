//! Simple HTTP benchmark tool

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::Request;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().collect();

    let url = args.get(1).map(|s| s.as_str()).unwrap_or("http://127.0.0.1:8080/");
    let concurrency: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    println!("Benchmark: {}", url);
    println!("Concurrency: {}", concurrency);
    println!("Duration: {}s", duration_secs);
    println!();

    let client = Client::builder(TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(concurrency)
        .build_http::<Empty<Bytes>>();

    let success_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    let total_latency_us = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);

    let mut handles = vec![];

    // Spawn worker tasks
    for _ in 0..concurrency {
        let client = client.clone();
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

                match client.request(req).await {
                    Ok(resp) => {
                        // Consume body
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

    // Wait for duration
    tokio::time::sleep(duration).await;
    running.store(false, Ordering::Relaxed);

    // Wait for all workers to finish current request
    for handle in handles {
        let _ = handle.await;
    }

    let elapsed = start.elapsed();
    let success = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let total_latency = total_latency_us.load(Ordering::Relaxed);

    let rps = success as f64 / elapsed.as_secs_f64();
    let avg_latency_us = if success > 0 {
        total_latency as f64 / success as f64
    } else {
        0.0
    };

    println!("=== Results ===");
    println!("Duration:     {:.2}s", elapsed.as_secs_f64());
    println!("Requests:     {}", success);
    println!("Errors:       {}", errors);
    println!("RPS:          {:.2}", rps);
    println!("Avg Latency:  {:.2}μs ({:.2}ms)", avg_latency_us, avg_latency_us / 1000.0);

    Ok(())
}
