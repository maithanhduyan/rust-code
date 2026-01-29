//! High-Performance HTTP/1.1 Benchmark
//!
//! Features:
//! - Keep-alive connection reuse
//! - Zero-copy response parsing
//! - Per-thread stats

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const BUFFER_SIZE: usize = 4096;
const REQUEST: &[u8] = b"GET /test HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n";

fn find_crlf2(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if data[i] == b'\r' && data[i+1] == b'\n' && data[i+2] == b'\r' && data[i+3] == b'\n' {
            return Some(i + 4);
        }
    }
    None
}

fn get_content_length(buf: &[u8]) -> usize {
    let needle = b"content-length:";
    for i in 0..buf.len().saturating_sub(needle.len() + 1) {
        if buf[i..].len() >= needle.len() {
            let mut matches = true;
            for j in 0..needle.len() {
                if buf[i+j].to_ascii_lowercase() != needle[j] {
                    matches = false;
                    break;
                }
            }
            if matches {
                let mut val = 0usize;
                for k in (i + needle.len())..buf.len() {
                    let b = buf[k];
                    if b == b'\r' || b == b'\n' { break; }
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

fn do_request(stream: &mut TcpStream, buf: &mut [u8]) -> bool {
    if stream.write_all(REQUEST).is_err() { return false; }
    
    let mut total = 0;
    loop {
        match stream.read(&mut buf[total..]) {
            Ok(0) => return false,
            Ok(n) => {
                total += n;
                if let Some(he) = find_crlf2(&buf[..total]) {
                    if &buf[9..12] != b"200" { return false; }
                    let cl = get_content_length(&buf[..he]);
                    let body_have = total - he;
                    if body_have >= cl { return true; }
                    let need = cl - body_have;
                    let mut got = 0;
                    while got < need {
                        match stream.read(&mut buf[..]) {
                            Ok(0) => return false,
                            Ok(n) => got += n,
                            Err(_) => return false,
                        }
                    }
                    return true;
                }
                if total >= buf.len() - 100 { return false; }
            }
            Err(_) => return false,
        }
    }
}

fn worker(target: String, success: Arc<AtomicU64>, errors: Arc<AtomicU64>, running: Arc<AtomicBool>) {
    let mut buf = vec![0u8; BUFFER_SIZE];
    let mut conn: Option<TcpStream> = None;
    
    while running.load(Ordering::Relaxed) {
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
        
        // 100 requests per connection batch
        for _ in 0..100 {
            if !running.load(Ordering::Relaxed) { break; }
            
            if do_request(&mut stream, &mut buf) {
                success.fetch_add(1, Ordering::Relaxed);
            } else {
                errors.fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
        
        conn = Some(stream);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    let target = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let connections: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
    
    println!("╔═══════════════════════════════════╗");
    println!("║   HP Benchmark - Keep-Alive Mode  ║");
    println!("╠═══════════════════════════════════╣");
    println!("║ Target: {:>25} ║", target);
    println!("║ Connections: {:>20} ║", connections);
    println!("║ Duration: {:>21}s ║", duration_secs);
    println!("╚═══════════════════════════════════╝");
    println!();
    
    let success = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));
    
    let mut handles = Vec::new();
    for _ in 0..connections {
        let t = target.clone();
        let s = Arc::clone(&success);
        let e = Arc::clone(&errors);
        let r = Arc::clone(&running);
        handles.push(thread::spawn(move || worker(t, s, e, r)));
    }
    
    let s2 = Arc::clone(&success);
    let r2 = Arc::clone(&running);
    let stats = thread::spawn(move || {
        let mut prev = 0u64;
        while r2.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            let curr = s2.load(Ordering::Relaxed);
            println!("RPS: {:>8}", curr - prev);
            prev = curr;
        }
    });
    
    let start = Instant::now();
    thread::sleep(Duration::from_secs(duration_secs));
    running.store(false, Ordering::Relaxed);
    
    for h in handles { h.join().ok(); }
    stats.join().ok();
    
    let elapsed = start.elapsed().as_secs_f64();
    let total_success = success.load(Ordering::Relaxed);
    let total_errors = errors.load(Ordering::Relaxed);
    
    println!();
    println!("╔═══════════════════════════════════╗");
    println!("║            RESULTS                ║");
    println!("╠═══════════════════════════════════╣");
    println!("║ Duration:   {:>18.2}s ║", elapsed);
    println!("║ Total:      {:>20} ║", total_success + total_errors);
    println!("║ Successful: {:>20} ║", total_success);
    println!("║ Errors:     {:>20} ║", total_errors);
    println!("║ RPS:        {:>20.0} ║", total_success as f64 / elapsed);
    println!("╚═══════════════════════════════════╝");
}
