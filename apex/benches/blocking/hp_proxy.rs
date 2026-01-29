//! High-Performance Blocking Proxy - Pure Rust
//!
//! Architecture:
//! - Fixed thread pool (N = num_cpus)
//! - Per-worker backend connection pool
//! - Byte-slice HTTP parsing (zero String allocation)
//! - Reusable buffers
//! - Per-worker RPS counter (no cache bouncing)
//!
//! Target: 50-100k RPS

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ============================================================================
// CONFIGURATION
// ============================================================================

const BACKEND_ADDR: &str = "127.0.0.1:9001";
const LISTEN_ADDR: &str = "0.0.0.0:8080";
const NUM_WORKERS: usize = 8; // Adjust to your CPU cores
const POOL_SIZE_PER_WORKER: usize = 50; // Backend connections per worker
const BUFFER_SIZE: usize = 4096;
const MAX_QUEUE_SIZE: usize = 10000;

// ============================================================================
// LOCK-FREE STATISTICS (per-worker to avoid cache bouncing)
// ============================================================================

struct WorkerStats {
    requests: AtomicU64,
    errors: AtomicU64,
}

impl WorkerStats {
    fn new() -> Self {
        Self {
            requests: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    #[inline(always)]
    fn inc_requests(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    fn inc_errors(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    fn get_requests(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }

    fn get_errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }
}

// ============================================================================
// BACKEND CONNECTION POOL (per-worker, no lock contention)
// ============================================================================

struct ConnectionPool {
    connections: VecDeque<TcpStream>,
    backend_addr: SocketAddr,
    max_size: usize,
}

impl ConnectionPool {
    fn new(backend_addr: SocketAddr, max_size: usize) -> Self {
        Self {
            connections: VecDeque::with_capacity(max_size),
            backend_addr,
            max_size,
        }
    }

    /// Get a connection from pool or create new one
    #[inline]
    fn get(&mut self) -> Option<TcpStream> {
        // Try to get from pool
        while let Some(conn) = self.connections.pop_front() {
            // Check if connection is still alive
            if is_connection_alive(&conn) {
                return Some(conn);
            }
            // Connection dead, drop it
        }

        // Create new connection
        match TcpStream::connect(self.backend_addr) {
            Ok(conn) => {
                conn.set_nodelay(true).ok();
                conn.set_read_timeout(Some(Duration::from_secs(5))).ok();
                conn.set_write_timeout(Some(Duration::from_secs(5))).ok();
                Some(conn)
            }
            Err(_) => None,
        }
    }

    /// Return connection to pool
    #[inline]
    fn put(&mut self, conn: TcpStream) {
        if self.connections.len() < self.max_size {
            self.connections.push_back(conn);
        }
        // Else drop the connection
    }
}

/// Check if TCP connection is still alive (non-blocking peek)
#[inline]
fn is_connection_alive(conn: &TcpStream) -> bool {
    conn.set_nonblocking(true).ok();
    let mut buf = [0u8; 1];
    let result = match conn.peek(&mut buf) {
        Ok(0) => false,      // Connection closed
        Ok(_) => true,       // Data available
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => true, // No data, still alive
        Err(_) => false,     // Error
    };
    conn.set_nonblocking(false).ok();
    result
}

// ============================================================================
// BYTE-SLICE HTTP PARSER (zero allocation)
// ============================================================================

/// Parse HTTP request from byte buffer
/// Returns: (method_slice, path_slice, content_length, headers_end, keep_alive)
#[inline]
fn parse_request(buf: &[u8], len: usize) -> Option<ParsedRequest> {
    let data = &buf[..len];

    // Find end of headers
    let headers_end = find_headers_end(data)?;

    // Parse request line
    let first_line_end = memchr(b'\r', data).unwrap_or(memchr(b'\n', data)?);
    let first_line = &data[..first_line_end];

    // Split: "GET /path HTTP/1.1"
    let method_end = memchr(b' ', first_line)?;
    let path_start = method_end + 1;
    let path_end = path_start + memchr(b' ', &first_line[path_start..])?;

    // Parse headers for Content-Length and Connection
    let mut content_length = 0usize;
    let mut keep_alive = true; // HTTP/1.1 default

    let mut pos = first_line_end + 2; // Skip \r\n
    while pos < headers_end {
        let line_end = pos + memchr(b'\r', &data[pos..]).unwrap_or(headers_end - pos);
        let line = &data[pos..line_end];

        if line.is_empty() {
            break;
        }

        // Check Content-Length (case-insensitive compare on first char)
        if line.len() > 15 && (line[0] == b'C' || line[0] == b'c') {
            if starts_with_ignore_case(line, b"content-length:") {
                if let Some(val) = parse_header_value(line, 15) {
                    content_length = parse_usize(val);
                }
            } else if starts_with_ignore_case(line, b"connection:") {
                if let Some(val) = parse_header_value(line, 11) {
                    keep_alive = !contains_ignore_case(val, b"close");
                }
            }
        }

        pos = line_end + 2; // Skip \r\n
    }

    Some(ParsedRequest {
        method_end,
        path_start,
        path_end,
        content_length,
        headers_end,
        keep_alive,
    })
}

/// Parsed request info (positions, not slices to avoid borrow issues)
struct ParsedRequest {
    method_end: usize,
    path_start: usize,
    path_end: usize,
    content_length: usize,
    headers_end: usize,
    keep_alive: bool,
}

/// Find \r\n\r\n in buffer
#[inline]
fn find_headers_end(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if data[i] == b'\r' && data[i+1] == b'\n' && data[i+2] == b'\r' && data[i+3] == b'\n' {
            return Some(i + 4);
        }
    }
    None
}

/// Find byte in slice
#[inline]
fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}

/// Case-insensitive prefix check
#[inline]
fn starts_with_ignore_case(data: &[u8], prefix: &[u8]) -> bool {
    if data.len() < prefix.len() {
        return false;
    }
    for i in 0..prefix.len() {
        if data[i].to_ascii_lowercase() != prefix[i].to_ascii_lowercase() {
            return false;
        }
    }
    true
}

/// Check if slice contains substring (case-insensitive)
#[inline]
fn contains_ignore_case(data: &[u8], needle: &[u8]) -> bool {
    if data.len() < needle.len() {
        return false;
    }
    for i in 0..=(data.len() - needle.len()) {
        if starts_with_ignore_case(&data[i..], needle) {
            return true;
        }
    }
    false
}

/// Parse header value (skip "Header: " prefix)
#[inline]
fn parse_header_value(line: &[u8], prefix_len: usize) -> Option<&[u8]> {
    if line.len() <= prefix_len {
        return None;
    }
    let mut start = prefix_len;
    while start < line.len() && line[start] == b' ' {
        start += 1;
    }
    Some(&line[start..])
}

/// Parse usize from bytes
#[inline]
fn parse_usize(data: &[u8]) -> usize {
    let mut result = 0usize;
    for &b in data {
        if b >= b'0' && b <= b'9' {
            result = result * 10 + (b - b'0') as usize;
        } else {
            break;
        }
    }
    result
}

// ============================================================================
// WORK QUEUE (shared between acceptor and workers)
// ============================================================================

struct WorkQueue {
    queue: Mutex<VecDeque<TcpStream>>,
    max_size: usize,
}

impl WorkQueue {
    fn new(max_size: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(max_size)),
            max_size,
        }
    }

    fn push(&self, conn: TcpStream) -> bool {
        let mut queue = self.queue.lock().unwrap();
        if queue.len() < self.max_size {
            queue.push_back(conn);
            true
        } else {
            false // Queue full, drop connection
        }
    }

    fn pop(&self) -> Option<TcpStream> {
        let mut queue = self.queue.lock().unwrap();
        queue.pop_front()
    }

    fn pop_batch(&self, batch: &mut Vec<TcpStream>, max: usize) {
        let mut queue = self.queue.lock().unwrap();
        for _ in 0..max {
            if let Some(conn) = queue.pop_front() {
                batch.push(conn);
            } else {
                break;
            }
        }
    }
}

// ============================================================================
// WORKER THREAD
// ============================================================================

fn worker_thread(
    id: usize,
    work_queue: Arc<WorkQueue>,
    stats: Arc<WorkerStats>,
    running: Arc<AtomicBool>,
    backend_addr: SocketAddr,
) {
    // Per-worker connection pool (no lock contention!)
    let mut pool = ConnectionPool::new(backend_addr, POOL_SIZE_PER_WORKER);

    // Reusable buffers
    let mut client_buf = vec![0u8; BUFFER_SIZE];
    let mut backend_buf = vec![0u8; BUFFER_SIZE * 2];
    let mut batch = Vec::with_capacity(16);

    while running.load(Ordering::Relaxed) {
        // Get batch of connections to reduce lock contention
        batch.clear();
        work_queue.pop_batch(&mut batch, 8);

        if batch.is_empty() {
            // No work, brief sleep
            thread::sleep(Duration::from_micros(100));
            continue;
        }

        for mut client in batch.drain(..) {
            // Handle all requests on this connection (keep-alive)
            loop {
                match handle_one_request(&mut client, &mut pool, &mut client_buf, &mut backend_buf) {
                    Ok(true) => {
                        // Keep-alive, continue
                        stats.inc_requests();
                    }
                    Ok(false) => {
                        // Connection close requested
                        stats.inc_requests();
                        break;
                    }
                    Err(_) => {
                        stats.inc_errors();
                        break;
                    }
                }
            }
        }
    }

    println!("Worker {} shutting down", id);
}

/// Handle one HTTP request, return Ok(keep_alive)
#[inline]
fn handle_one_request(
    client: &mut TcpStream,
    pool: &mut ConnectionPool,
    client_buf: &mut [u8],
    backend_buf: &mut [u8],
) -> std::io::Result<bool> {
    // Read request
    let mut total_read = 0;
    loop {
        let n = client.read(&mut client_buf[total_read..])?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "client closed"));
        }
        total_read += n;

        // Check if we have complete headers
        if find_headers_end(&client_buf[..total_read]).is_some() {
            break;
        }

        if total_read >= client_buf.len() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "request too large"));
        }
    }

    // Parse request - extract positions to avoid borrow issues
    let parsed = parse_request(client_buf, total_read)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "parse error"))?;

    let method_end = parsed.method_end;
    let path_start = parsed.path_start;
    let path_end = parsed.path_end;
    let content_length = parsed.content_length;
    let headers_end = parsed.headers_end;
    let keep_alive = parsed.keep_alive;

    // Copy method and path to avoid borrow issues
    let mut method_buf = [0u8; 16];
    let mut path_buf = [0u8; 2048];
    method_buf[..method_end].copy_from_slice(&client_buf[..method_end]);
    let path_len = path_end - path_start;
    path_buf[..path_len].copy_from_slice(&client_buf[path_start..path_end]);

    // Now we can safely read more into client_buf
    let body_already_read = total_read - headers_end;
    if content_length > body_already_read {
        let remaining = content_length - body_already_read;
        let mut body_read = 0;
        while body_read < remaining {
            let n = client.read(&mut client_buf[total_read + body_read..])?;
            if n == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "body incomplete"));
            }
            body_read += n;
        }
    }

    // Forward to backend
    let mut backend = pool.get()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "no backend"))?;

    // Build backend request
    let req_len = build_backend_request(backend_buf, &method_buf[..method_end], &path_buf[..path_len], content_length,
        &client_buf[headers_end..headers_end + content_length]);

    // Send to backend
    if backend.write_all(&backend_buf[..req_len]).is_err() {
        // Connection failed, try new one
        backend = pool.get()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "no backend"))?;
        backend.write_all(&backend_buf[..req_len])?;
    }

    // Read response from backend
    let response_len = read_http_response(&mut backend, backend_buf)?;

    // Return connection to pool
    pool.put(backend);

    // Send response to client
    client.write_all(&backend_buf[..response_len])?;

    Ok(keep_alive)
}

/// Build backend request into buffer, return length
#[inline]
fn build_backend_request(buf: &mut [u8], method: &[u8], path: &[u8], content_length: usize, body: &[u8]) -> usize {
    let mut pos = 0;

    // Method
    buf[pos..pos + method.len()].copy_from_slice(method);
    pos += method.len();
    buf[pos] = b' ';
    pos += 1;

    // Path
    buf[pos..pos + path.len()].copy_from_slice(path);
    pos += path.len();

    // Version + headers
    let suffix = b" HTTP/1.1\r\nHost: 127.0.0.1:9001\r\nConnection: keep-alive\r\n";
    buf[pos..pos + suffix.len()].copy_from_slice(suffix);
    pos += suffix.len();

    // Content-Length if body
    if content_length > 0 {
        let cl_header = format!("Content-Length: {}\r\n", content_length);
        let cl_bytes = cl_header.as_bytes();
        buf[pos..pos + cl_bytes.len()].copy_from_slice(cl_bytes);
        pos += cl_bytes.len();
    }

    // End headers
    buf[pos] = b'\r';
    buf[pos + 1] = b'\n';
    pos += 2;

    // Body
    if content_length > 0 {
        buf[pos..pos + content_length].copy_from_slice(body);
        pos += content_length;
    }

    pos
}

/// Read complete HTTP response from backend
#[inline]
fn read_http_response(conn: &mut TcpStream, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;

    // Read until we have headers
    loop {
        let n = conn.read(&mut buf[total..])?;
        if n == 0 {
            if total == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "no response"));
            }
            break;
        }
        total += n;

        if let Some(headers_end) = find_headers_end(&buf[..total]) {
            // Parse Content-Length
            let content_length = extract_content_length(&buf[..headers_end]);
            let body_needed = content_length;
            let body_have = total - headers_end;

            // Read remaining body
            if body_have < body_needed {
                let remaining = body_needed - body_have;
                let mut read = 0;
                while read < remaining {
                    let n = conn.read(&mut buf[total + read..])?;
                    if n == 0 {
                        break;
                    }
                    read += n;
                }
                total += read;
            }
            break;
        }

        if total >= buf.len() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "response too large"));
        }
    }

    Ok(total)
}

/// Extract Content-Length from response headers
#[inline]
fn extract_content_length(headers: &[u8]) -> usize {
    // Simple scan for "Content-Length: "
    for i in 0..headers.len().saturating_sub(16) {
        if starts_with_ignore_case(&headers[i..], b"content-length:") {
            let start = i + 15;
            let mut end = start;
            while end < headers.len() && headers[end] != b'\r' && headers[end] != b'\n' {
                end += 1;
            }
            return parse_usize(&headers[start..end]);
        }
    }
    0
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    let listen_addr: SocketAddr = LISTEN_ADDR.parse().expect("Invalid listen address");
    let backend_addr: SocketAddr = BACKEND_ADDR.parse().expect("Invalid backend address");

    println!("=== High-Performance Blocking Proxy ===");
    println!("Listen: {}", listen_addr);
    println!("Backend: {}", backend_addr);
    println!("Workers: {}", NUM_WORKERS);
    println!("Pool size per worker: {}", POOL_SIZE_PER_WORKER);
    println!();

    let work_queue = Arc::new(WorkQueue::new(MAX_QUEUE_SIZE));
    let running = Arc::new(AtomicBool::new(true));
    let worker_stats: Vec<Arc<WorkerStats>> = (0..NUM_WORKERS)
        .map(|_| Arc::new(WorkerStats::new()))
        .collect();

    // Start workers
    let mut worker_handles = Vec::new();
    for i in 0..NUM_WORKERS {
        let queue = Arc::clone(&work_queue);
        let stats = Arc::clone(&worker_stats[i]);
        let running = Arc::clone(&running);

        let handle = thread::spawn(move || {
            worker_thread(i, queue, stats, running, backend_addr);
        });
        worker_handles.push(handle);
    }

    // Stats reporter
    let stats_clone: Vec<Arc<WorkerStats>> = worker_stats.iter().map(Arc::clone).collect();
    let running_clone = Arc::clone(&running);
    thread::spawn(move || {
        let mut prev_total = 0u64;
        while running_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));

            let mut total_requests = 0u64;
            let mut total_errors = 0u64;
            for s in &stats_clone {
                total_requests += s.get_requests();
                total_errors += s.get_errors();
            }

            let rps = total_requests - prev_total;
            prev_total = total_requests;

            println!("RPS: {} | Total: {} | Errors: {}", rps, total_requests, total_errors);
        }
    });

    // Acceptor loop
    let listener = TcpListener::bind(listen_addr).expect("Failed to bind");
    listener.set_nonblocking(false).ok(); // Blocking accept is fine

    println!("Accepting connections...");

    for stream in listener.incoming() {
        match stream {
            Ok(conn) => {
                conn.set_nodelay(true).ok();
                conn.set_read_timeout(Some(Duration::from_secs(30))).ok();
                conn.set_write_timeout(Some(Duration::from_secs(30))).ok();

                if !work_queue.push(conn) {
                    // Queue full, connection dropped
                }
            }
            Err(_) => {
                // Accept error, continue
            }
        }
    }
}
