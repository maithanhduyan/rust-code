//! HP Proxy V2 - Zero-Copy, No Syscall Waste
//!
//! Fixes:
//! 1. ❌ is_connection_alive → fail-fast
//! 2. ❌ method/path copy → offset-based
//! 3. ❌ format!() → write_decimal()
//! 4. ❌ Single mutex queue → per-worker queues + round-robin
//! 5. Shorter timeouts to reduce HOL blocking
//!
//! Target: 80-120k RPS

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ============================================================================
// CONFIGURATION
// ============================================================================

const BACKEND_ADDR: &str = "127.0.0.1:9001";
const LISTEN_ADDR: &str = "0.0.0.0:8080";
const NUM_WORKERS: usize = 8;
const POOL_SIZE_PER_WORKER: usize = 20; // Smaller = less stale connections
const BUFFER_SIZE: usize = 8192;
const QUEUE_SIZE_PER_WORKER: usize = 1024;

// Short timeouts to avoid HOL blocking
const READ_TIMEOUT_MS: u64 = 500;
const WRITE_TIMEOUT_MS: u64 = 500;
const BACKEND_TIMEOUT_MS: u64 = 200;

// ============================================================================
// STATS
// ============================================================================

struct Stats {
    requests: AtomicU64,
    errors: AtomicU64,
    pool_hits: AtomicU64,
    pool_misses: AtomicU64,
}

impl Stats {
    fn new() -> Self {
        Self {
            requests: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            pool_hits: AtomicU64::new(0),
            pool_misses: AtomicU64::new(0),
        }
    }
}

// ============================================================================
// BACKEND CONNECTION POOL - NO ALIVE CHECK, FAIL-FAST
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

    /// Get connection - NO ALIVE CHECK, just pop
    #[inline(always)]
    fn get(&mut self) -> Option<TcpStream> {
        // Try pool first - NO VALIDATION
        if let Some(conn) = self.connections.pop_front() {
            return Some(conn);
        }

        // Create new
        TcpStream::connect(self.backend_addr).ok().map(|conn| {
            conn.set_nodelay(true).ok();
            conn.set_read_timeout(Some(Duration::from_millis(BACKEND_TIMEOUT_MS))).ok();
            conn.set_write_timeout(Some(Duration::from_millis(BACKEND_TIMEOUT_MS))).ok();
            conn
        })
    }

    /// Return connection - just push, no validation
    #[inline(always)]
    fn put(&mut self, conn: TcpStream) {
        if self.connections.len() < self.max_size {
            self.connections.push_back(conn);
        }
    }
}

// ============================================================================
// PER-WORKER QUEUE (eliminates central mutex contention)
// ============================================================================

struct WorkerQueue {
    queue: Mutex<VecDeque<TcpStream>>,
}

impl WorkerQueue {
    fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    #[inline(always)]
    fn push(&self, conn: TcpStream) -> bool {
        if let Ok(mut q) = self.queue.try_lock() {
            if q.len() < QUEUE_SIZE_PER_WORKER {
                q.push_back(conn);
                return true;
            }
        }
        false
    }

    #[inline(always)]
    fn pop_batch(&self, batch: &mut Vec<TcpStream>, max: usize) {
        if let Ok(mut q) = self.queue.try_lock() {
            for _ in 0..max {
                match q.pop_front() {
                    Some(c) => batch.push(c),
                    None => break,
                }
            }
        }
    }
}

// ============================================================================
// ZERO-COPY HTTP PARSER
// ============================================================================

/// Parsed request - offsets only, no slices
#[derive(Clone, Copy)]
struct ParsedRequest {
    method_start: usize,
    method_end: usize,
    path_start: usize,
    path_end: usize,
    headers_end: usize,
    content_length: usize,
    keep_alive: bool,
}

/// Parse HTTP request, return offsets
#[inline(always)]
fn parse_request(buf: &[u8], len: usize) -> Option<ParsedRequest> {
    let data = &buf[..len];

    // Find \r\n\r\n
    let headers_end = find_crlf2(data)?;

    // First line: "GET /path HTTP/1.1\r\n"
    let line_end = memchr(b'\r', data)?;
    let first_space = memchr(b' ', &data[..line_end])?;
    let second_space = first_space + 1 + memchr(b' ', &data[first_space + 1..line_end])?;

    let method_start = 0;
    let method_end = first_space;
    let path_start = first_space + 1;
    let path_end = second_space;

    // Scan headers
    let mut content_length = 0usize;
    let mut keep_alive = true;
    let mut pos = line_end + 2;

    while pos < headers_end - 2 {
        let line_end = pos + memchr(b'\r', &data[pos..]).unwrap_or(headers_end - pos);
        let line_len = line_end - pos;

        if line_len == 0 {
            break;
        }

        // Content-Length: (15 chars)
        if line_len > 15 {
            let first = data[pos];
            if first == b'C' || first == b'c' {
                if eq_ignore_case(&data[pos..pos + 15], b"content-length:") {
                    content_length = parse_int(&data[pos + 15..line_end]);
                } else if line_len > 11 && eq_ignore_case(&data[pos..pos + 11], b"connection:") {
                    keep_alive = !contains_close(&data[pos + 11..line_end]);
                }
            }
        }

        pos = line_end + 2;
    }

    Some(ParsedRequest {
        method_start,
        method_end,
        path_start,
        path_end,
        headers_end,
        content_length,
        keep_alive,
    })
}

#[inline(always)]
fn find_crlf2(data: &[u8]) -> Option<usize> {
    let len = data.len();
    if len < 4 {
        return None;
    }
    let mut i = 0;
    while i < len - 3 {
        if data[i] == b'\r' && data[i + 1] == b'\n' && data[i + 2] == b'\r' && data[i + 3] == b'\n' {
            return Some(i + 4);
        }
        i += 1;
    }
    None
}

#[inline(always)]
fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[inline(always)]
fn eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    if a.len() < b.len() {
        return false;
    }
    let mut i = 0;
    while i < b.len() {
        if a[i].to_ascii_lowercase() != b[i].to_ascii_lowercase() {
            return false;
        }
        i += 1;
    }
    true
}

#[inline(always)]
fn contains_close(data: &[u8]) -> bool {
    if data.len() < 5 {
        return false;
    }
    let mut i = 0;
    while i <= data.len() - 5 {
        if eq_ignore_case(&data[i..], b"close") {
            return true;
        }
        i += 1;
    }
    false
}

#[inline(always)]
fn parse_int(data: &[u8]) -> usize {
    let mut result = 0usize;
    for &b in data {
        if b >= b'0' && b <= b'9' {
            result = result * 10 + (b - b'0') as usize;
        } else if b != b' ' {
            break;
        }
    }
    result
}

/// Write decimal to buffer, return bytes written
#[inline(always)]
fn write_decimal(mut n: usize, buf: &mut [u8]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }

    let mut digits = [0u8; 20];
    let mut len = 0;
    while n > 0 {
        digits[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }

    // Reverse into buf
    let mut i = 0;
    while i < len {
        buf[i] = digits[len - 1 - i];
        i += 1;
    }
    len
}

// ============================================================================
// ZERO-COPY REQUEST BUILDER
// ============================================================================

/// Build backend request directly from client buffer offsets
/// Returns length written to backend_buf
#[inline(always)]
fn build_request_zerocopy(
    backend_buf: &mut [u8],
    client_buf: &[u8],
    req: &ParsedRequest,
) -> usize {
    let mut pos = 0;

    // Method (directly from client_buf)
    let method_len = req.method_end - req.method_start;
    backend_buf[pos..pos + method_len].copy_from_slice(&client_buf[req.method_start..req.method_end]);
    pos += method_len;

    backend_buf[pos] = b' ';
    pos += 1;

    // Path (directly from client_buf)
    let path_len = req.path_end - req.path_start;
    backend_buf[pos..pos + path_len].copy_from_slice(&client_buf[req.path_start..req.path_end]);
    pos += path_len;

    // Static suffix
    const SUFFIX: &[u8] = b" HTTP/1.1\r\nHost: backend\r\nConnection: keep-alive\r\n";
    backend_buf[pos..pos + SUFFIX.len()].copy_from_slice(SUFFIX);
    pos += SUFFIX.len();

    // Content-Length (no format!, use write_decimal)
    if req.content_length > 0 {
        const CL_PREFIX: &[u8] = b"Content-Length: ";
        backend_buf[pos..pos + CL_PREFIX.len()].copy_from_slice(CL_PREFIX);
        pos += CL_PREFIX.len();

        let digits = write_decimal(req.content_length, &mut backend_buf[pos..]);
        pos += digits;

        backend_buf[pos] = b'\r';
        backend_buf[pos + 1] = b'\n';
        pos += 2;
    }

    // End headers
    backend_buf[pos] = b'\r';
    backend_buf[pos + 1] = b'\n';
    pos += 2;

    // Body (directly from client_buf)
    if req.content_length > 0 {
        let body_start = req.headers_end;
        let body_end = body_start + req.content_length;
        backend_buf[pos..pos + req.content_length].copy_from_slice(&client_buf[body_start..body_end]);
        pos += req.content_length;
    }

    pos
}

// ============================================================================
// RESPONSE READER
// ============================================================================

#[inline(always)]
fn read_response(conn: &mut TcpStream, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;

    loop {
        let n = conn.read(&mut buf[total..])?;
        if n == 0 {
            if total == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, ""));
            }
            break;
        }
        total += n;

        // Check for complete response
        if let Some(headers_end) = find_crlf2(&buf[..total]) {
            let content_length = extract_cl(&buf[..headers_end]);
            let body_have = total - headers_end;

            if body_have >= content_length {
                break;
            }

            // Read remaining body
            let need = content_length - body_have;
            let mut got = 0;
            while got < need {
                let n = conn.read(&mut buf[total..])?;
                if n == 0 {
                    break;
                }
                total += n;
                got += n;
            }
            break;
        }

        if total >= buf.len() - 1024 {
            break;
        }
    }

    Ok(total)
}

#[inline(always)]
fn extract_cl(headers: &[u8]) -> usize {
    let mut i = 0;
    while i < headers.len().saturating_sub(16) {
        if eq_ignore_case(&headers[i..], b"content-length:") {
            return parse_int(&headers[i + 15..]);
        }
        i += 1;
    }
    0
}

// ============================================================================
// WORKER
// ============================================================================

fn worker_thread(
    id: usize,
    queue: Arc<WorkerQueue>,
    stats: Arc<Stats>,
    running: Arc<AtomicBool>,
    backend_addr: SocketAddr,
) {
    let mut pool = ConnectionPool::new(backend_addr, POOL_SIZE_PER_WORKER);
    let mut client_buf = vec![0u8; BUFFER_SIZE];
    let mut backend_buf = vec![0u8; BUFFER_SIZE * 2];
    let mut batch = Vec::with_capacity(16);

    while running.load(Ordering::Relaxed) {
        batch.clear();
        queue.pop_batch(&mut batch, 8);

        if batch.is_empty() {
            thread::yield_now();
            continue;
        }

        for mut client in batch.drain(..) {
            loop {
                match handle_request(&mut client, &mut pool, &mut client_buf, &mut backend_buf, &stats) {
                    Ok(true) => continue,  // keep-alive
                    Ok(false) => break,    // close
                    Err(_) => {
                        stats.errors.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                }
            }
        }
    }
}

#[inline(always)]
fn handle_request(
    client: &mut TcpStream,
    pool: &mut ConnectionPool,
    client_buf: &mut [u8],
    backend_buf: &mut [u8],
    stats: &Stats,
) -> std::io::Result<bool> {
    // Read request
    let mut total = 0;
    loop {
        let n = client.read(&mut client_buf[total..])?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, ""));
        }
        total += n;

        if find_crlf2(&client_buf[..total]).is_some() {
            break;
        }
        if total >= client_buf.len() - 1024 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, ""));
        }
    }

    // Parse (zero-copy)
    let req = parse_request(client_buf, total)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, ""))?;

    // Read body if needed
    let body_have = total - req.headers_end;
    if req.content_length > body_have {
        let need = req.content_length - body_have;
        let mut got = 0;
        while got < need {
            let n = client.read(&mut client_buf[total..])?;
            if n == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, ""));
            }
            total += n;
            got += n;
        }
    }

    // Build backend request (zero-copy from client_buf)
    let req_len = build_request_zerocopy(backend_buf, client_buf, &req);

    // Get backend connection (fail-fast, no alive check)
    let mut backend = pool.get()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, ""))?;

    // Send request - if fails, try once more with fresh connection
    if backend.write_all(&backend_buf[..req_len]).is_err() {
        // Drop failed connection, get new one
        backend = pool.get()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, ""))?;
        backend.write_all(&backend_buf[..req_len])?;
        stats.pool_misses.fetch_add(1, Ordering::Relaxed);
    } else {
        stats.pool_hits.fetch_add(1, Ordering::Relaxed);
    }

    // Read response
    let resp_len = read_response(&mut backend, backend_buf)?;

    // Return connection to pool (no validation)
    pool.put(backend);

    // Send to client
    client.write_all(&backend_buf[..resp_len])?;

    stats.requests.fetch_add(1, Ordering::Relaxed);
    Ok(req.keep_alive)
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    let listen_addr: SocketAddr = LISTEN_ADDR.parse().unwrap();
    let backend_addr: SocketAddr = BACKEND_ADDR.parse().unwrap();

    println!("=== HP Proxy V2 - Zero Copy ===");
    println!("Listen: {}", listen_addr);
    println!("Backend: {}", backend_addr);
    println!("Workers: {}", NUM_WORKERS);
    println!();

    let running = Arc::new(AtomicBool::new(true));
    let stats = Arc::new(Stats::new());

    // Per-worker queues
    let queues: Vec<Arc<WorkerQueue>> = (0..NUM_WORKERS)
        .map(|_| Arc::new(WorkerQueue::new(QUEUE_SIZE_PER_WORKER)))
        .collect();

    // Start workers
    let mut handles = Vec::new();
    for i in 0..NUM_WORKERS {
        let queue = Arc::clone(&queues[i]);
        let stats = Arc::clone(&stats);
        let running = Arc::clone(&running);

        handles.push(thread::spawn(move || {
            worker_thread(i, queue, stats, running, backend_addr);
        }));
    }

    // Stats reporter
    let stats2 = Arc::clone(&stats);
    let running2 = Arc::clone(&running);
    thread::spawn(move || {
        let mut prev = 0u64;
        while running2.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            let curr = stats2.requests.load(Ordering::Relaxed);
            let errs = stats2.errors.load(Ordering::Relaxed);
            let hits = stats2.pool_hits.load(Ordering::Relaxed);
            let misses = stats2.pool_misses.load(Ordering::Relaxed);
            let hit_rate = if hits + misses > 0 {
                hits * 100 / (hits + misses)
            } else {
                0
            };
            println!("RPS: {} | Total: {} | Errors: {} | Pool hit: {}%",
                curr - prev, curr, errs, hit_rate);
            prev = curr;
        }
    });

    // Round-robin counter for distributing to workers
    let rr_counter = AtomicUsize::new(0);

    // Accept loop
    let listener = TcpListener::bind(listen_addr).unwrap();
    println!("Accepting...");

    for stream in listener.incoming() {
        if let Ok(conn) = stream {
            conn.set_nodelay(true).ok();
            conn.set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS))).ok();
            conn.set_write_timeout(Some(Duration::from_millis(WRITE_TIMEOUT_MS))).ok();

            // Round-robin to workers
            let idx = rr_counter.fetch_add(1, Ordering::Relaxed) % NUM_WORKERS;
            queues[idx].push(conn);
        }
    }
}
