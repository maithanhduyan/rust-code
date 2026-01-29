//! HP Proxy V8 - Backend Connection Pooling
//!
//! Key optimizations:
//! - Backend connection pool (M connections for N clients)
//! - Zero-copy buffer exchange
//! - Single-pass header parsing (no to_ascii_lowercase in hot path)
//! - Chunked Transfer-Encoding support
//! - No unnecessary memory zeroing

use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::net::{TcpListener, TcpStream};
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const BACKEND: &str = "127.0.0.1:9001";
const LISTEN: &str = "0.0.0.0:8080";
const WORKERS: usize = 4;
const BUFSZ: usize = 16384;
const POOL_SIZE: usize = 64; // Backend connections per worker

/// Cache-line aligned stats to prevent false sharing
#[repr(align(64))]
struct Stats {
    reqs: AtomicU64,
    errs: AtomicU64,
    conns: AtomicU64,
    bytes: AtomicU64,
}

impl Stats {
    fn new() -> Self {
        Self {
            reqs: AtomicU64::new(0),
            errs: AtomicU64::new(0),
            conns: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        }
    }
}

/// Backend Connection Pool - single-threaded, no runtime borrow checks
/// SAFETY: Only used within single-threaded monoio runtime
struct BackendPool {
    addr: SocketAddr,
    inner: UnsafeCell<VecDeque<TcpStream>>,
    max_size: usize,
}

impl BackendPool {
    fn new(addr: SocketAddr, max_size: usize) -> Self {
        Self {
            addr,
            inner: UnsafeCell::new(VecDeque::with_capacity(max_size)),
            max_size,
        }
    }
    
    /// Try to get a connection from pool
    /// SAFETY: Single-threaded, non-reentrant
    fn try_get(&self) -> Option<TcpStream> {
        unsafe { (*self.inner.get()).pop_front() }
    }
    
    fn addr(&self) -> SocketAddr {
        self.addr
    }
    
    /// Return connection to pool
    /// SAFETY: Single-threaded, non-reentrant  
    fn put(&self, conn: TcpStream) {
        unsafe {
            let pool = &mut *self.inner.get();
            if pool.len() < self.max_size {
                pool.push_back(conn);
            }
        }
    }
}

#[inline(always)]
fn find_headers_end(d: &[u8]) -> Option<usize> {
    memchr::memmem::find(d, b"\r\n\r\n").map(|p| p + 4)
}

/// Parse Content-Length - case insensitive, single pass
#[inline(always)]
fn parse_cl_fast(h: &[u8]) -> Option<usize> {
    // Look for "ontent-length:" (skip first char for case insensitivity)
    let mut i = 0;
    while i + 16 < h.len() {
        // Fast skip using memchr for 'o' or 'O'
        if let Some(pos) = memchr::memchr2(b'c', b'C', &h[i..]) {
            let start = i + pos;
            if start + 16 <= h.len() {
                let candidate = &h[start..start + 15];
                // Check "ontent-length:" case-insensitive
                if (candidate[0] == b'c' || candidate[0] == b'C') &&
                   (candidate[1] == b'o' || candidate[1] == b'O') &&
                   (candidate[2] == b'n' || candidate[2] == b'N') &&
                   (candidate[3] == b't' || candidate[3] == b'T') &&
                   (candidate[4] == b'e' || candidate[4] == b'E') &&
                   (candidate[5] == b'n' || candidate[5] == b'N') &&
                   (candidate[6] == b't' || candidate[6] == b'T') &&
                   candidate[7] == b'-' &&
                   (candidate[8] == b'l' || candidate[8] == b'L') &&
                   (candidate[9] == b'e' || candidate[9] == b'E') &&
                   (candidate[10] == b'n' || candidate[10] == b'N') &&
                   (candidate[11] == b'g' || candidate[11] == b'G') &&
                   (candidate[12] == b't' || candidate[12] == b'T') &&
                   (candidate[13] == b'h' || candidate[13] == b'H') &&
                   candidate[14] == b':' {
                    // Parse number
                    let mut v = 0usize;
                    let mut found = false;
                    for &b in &h[start + 15..] {
                        if b == b' ' || b == b'\t' { 
                            if found { break; }
                            continue; 
                        }
                        if b.is_ascii_digit() {
                            found = true;
                            v = v * 10 + (b - b'0') as usize;
                        } else {
                            break;
                        }
                    }
                    if found { return Some(v); }
                }
            }
            i = start + 1;
        } else {
            break;
        }
    }
    None
}

/// Check chunked encoding - fast path
#[inline(always)]
fn is_chunked_fast(h: &[u8]) -> bool {
    // Look for "ransfer-encoding:" then "chunked"
    let mut i = 0;
    while i + 18 < h.len() {
        if let Some(pos) = memchr::memchr2(b't', b'T', &h[i..]) {
            let start = i + pos;
            if start + 18 <= h.len() {
                let candidate = &h[start..];
                if (candidate[0] == b't' || candidate[0] == b'T') &&
                   (candidate[1] == b'r' || candidate[1] == b'R') &&
                   (candidate[2] == b'a' || candidate[2] == b'A') &&
                   (candidate[3] == b'n' || candidate[3] == b'N') &&
                   (candidate[4] == b's' || candidate[4] == b'S') &&
                   (candidate[5] == b'f' || candidate[5] == b'F') &&
                   (candidate[6] == b'e' || candidate[6] == b'E') &&
                   (candidate[7] == b'r' || candidate[7] == b'R') &&
                   candidate[8] == b'-' {
                    // Found "transfer-", now check for "chunked" after ':'
                    if let Some(colon) = memchr::memchr(b':', &candidate[9..40.min(candidate.len())]) {
                        let after_colon = &candidate[9 + colon..];
                        if memchr::memmem::find(&after_colon[..50.min(after_colon.len())], b"chunked").is_some() ||
                           memchr::memmem::find(&after_colon[..50.min(after_colon.len())], b"Chunked").is_some() {
                            return true;
                        }
                    }
                }
            }
            i = start + 1;
        } else {
            break;
        }
    }
    false
}

#[inline(always)]
fn is_keepalive_fast(h: &[u8]) -> bool {
    // Look for "onnection: close"
    !memchr::memmem::find(h, b"onnection: close").is_some() &&
    !memchr::memmem::find(h, b"onnection: Close").is_some() &&
    !memchr::memmem::find(h, b"ONNECTION: CLOSE").is_some()
}

/// Extract Host header - fast
#[inline]
fn extract_host_fast(headers: &[u8]) -> Option<&[u8]> {
    // Find "Host:" or "host:"
    let mut i = 0;
    while i + 6 < headers.len() {
        if let Some(pos) = memchr::memchr2(b'h', b'H', &headers[i..]) {
            let start = i + pos;
            if start + 5 <= headers.len() {
                let c = &headers[start..];
                if (c[0] == b'h' || c[0] == b'H') &&
                   (c[1] == b'o' || c[1] == b'O') &&
                   (c[2] == b's' || c[2] == b'S') &&
                   (c[3] == b't' || c[3] == b'T') &&
                   c[4] == b':' {
                    // Skip whitespace
                    let mut j = 5;
                    while j < c.len() && (c[j] == b' ' || c[j] == b'\t') { j += 1; }
                    // Find end
                    if let Some(end) = memchr::memchr(b'\r', &c[j..]) {
                        return Some(&c[j..j + end]);
                    }
                }
            }
            i = start + 1;
        } else {
            break;
        }
    }
    None
}

/// Check if chunked body is complete
#[inline]
fn is_chunked_complete(data: &[u8]) -> bool {
    if data.len() < 5 { return false; }
    let tail = &data[data.len().saturating_sub(7)..];
    memchr::memmem::find(tail, b"0\r\n\r\n").is_some()
}

/// Buffer pool for zero-copy operations
struct BufferPool {
    buffers: Vec<Vec<u8>>,
}

impl BufferPool {
    fn new(count: usize, size: usize) -> Self {
        Self {
            buffers: (0..count).map(|_| Vec::with_capacity(size)).collect(),
        }
    }
    
    #[inline]
    fn get(&mut self) -> Vec<u8> {
        self.buffers.pop().unwrap_or_else(|| Vec::with_capacity(BUFSZ))
    }
    
    #[inline]
    fn put(&mut self, mut buf: Vec<u8>) {
        if self.buffers.len() < 8 {
            unsafe { buf.set_len(buf.capacity()); }
            self.buffers.push(buf);
        }
    }
}

async fn proxy_conn(
    mut cli: TcpStream, 
    be_pool: Rc<BackendPool>,
    st: Arc<Stats>
) {
    st.conns.fetch_add(1, Ordering::Relaxed);
    
    let mut buf_pool = BufferPool::new(4, BUFSZ);
    let mut req_builder: Vec<u8> = Vec::with_capacity(BUFSZ);
    
    loop {
        // === READ REQUEST ===
        let mut buf = buf_pool.get();
        unsafe { buf.set_len(buf.capacity()); }
        
        let (res, returned_buf) = cli.read(buf).await;
        let mut buf = returned_buf;
        
        let n = match res {
            Ok(0) => { buf_pool.put(buf); return; }
            Ok(n) => n,
            Err(_) => { buf_pool.put(buf); st.errs.fetch_add(1, Ordering::Relaxed); return; }
        };
        
        unsafe { buf.set_len(n); }
        
        let he = match find_headers_end(&buf) {
            Some(h) => h,
            None => { buf_pool.put(buf); st.errs.fetch_add(1, Ordering::Relaxed); return; }
        };
        
        let cl = parse_cl_fast(&buf[..he]).unwrap_or(0);
        let ka = is_keepalive_fast(&buf[..he]);
        
        // Parse request line
        let fl_end = memchr::memchr(b'\r', &buf).unwrap_or(n);
        let sp1 = memchr::memchr(b' ', &buf[..fl_end]).unwrap_or(0);
        let sp2 = sp1 + 1 + memchr::memchr(b' ', &buf[sp1+1..fl_end]).unwrap_or(0);
        
        let method = &buf[..sp1];
        let path = &buf[sp1+1..sp2];
        let host = extract_host_fast(&buf[..he]).unwrap_or(b"backend");
        
        // Build request (minimal headers for speed)
        req_builder.clear();
        req_builder.extend_from_slice(method);
        req_builder.push(b' ');
        req_builder.extend_from_slice(path);
        req_builder.extend_from_slice(b" HTTP/1.1\r\nHost: ");
        req_builder.extend_from_slice(host);
        req_builder.extend_from_slice(b"\r\nConnection: keep-alive\r\n");
        
        if cl > 0 {
            req_builder.extend_from_slice(b"Content-Length: ");
            let mut itoa_buf = [0u8; 20];
            let len = write_usize(cl, &mut itoa_buf);
            req_builder.extend_from_slice(&itoa_buf[..len]);
            req_builder.extend_from_slice(b"\r\n");
        }
        req_builder.extend_from_slice(b"\r\n");
        
        if cl > 0 && he + cl <= n {
            req_builder.extend_from_slice(&buf[he..he+cl]);
        }
        
        buf_pool.put(buf);
        
        // === GET BACKEND CONNECTION FROM POOL ===
        let be_addr = be_pool.addr();
        let mut be = match be_pool.try_get() {
            Some(c) => c,
            None => {
                // Create new connection
                match TcpStream::connect(be_addr).await {
                    Ok(c) => c,
                    Err(_) => { st.errs.fetch_add(1, Ordering::Relaxed); return; }
                }
            }
        };
        
        // === SEND TO BACKEND ===
        let send_buf = std::mem::replace(&mut req_builder, Vec::with_capacity(BUFSZ));
        let (res, returned_req) = be.write_all(send_buf).await;
        req_builder = returned_req;
        
        if res.is_err() {
            // Discard bad connection, try fresh one
            drop(be);
            be = match TcpStream::connect(be_addr).await {
                Ok(c) => c,
                Err(_) => { st.errs.fetch_add(1, Ordering::Relaxed); return; }
            };
            req_builder.clear();
            req_builder.extend_from_slice(b"GET / HTTP/1.1\r\nHost: backend\r\nConnection: keep-alive\r\n\r\n");
            let send_buf = std::mem::replace(&mut req_builder, Vec::with_capacity(256));
            if be.write_all(send_buf).await.0.is_err() {
                // Just drop the bad connection
                st.errs.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
        
        // === STREAM RESPONSE ===
        let mut got_headers = false;
        let mut body_left = 0usize;
        let mut chunked = false;
        let mut chunked_done = false;
        let mut be_healthy = true;
        
        loop {
            let mut resp_buf = buf_pool.get();
            unsafe { resp_buf.set_len(resp_buf.capacity()); }
            
            let (res, returned_buf) = be.read(resp_buf).await;
            let mut resp_buf = returned_buf;
            
            let rn = match res {
                Ok(0) => { 
                    buf_pool.put(resp_buf); 
                    be_healthy = false;
                    break; 
                }
                Ok(n) => n,
                Err(_) => { 
                    buf_pool.put(resp_buf);
                    be_healthy = false;
                    st.errs.fetch_add(1, Ordering::Relaxed); 
                    break;
                }
            };
            
            unsafe { resp_buf.set_len(rn); }
            
            if !got_headers {
                if let Some(rhe) = find_headers_end(&resp_buf) {
                    chunked = is_chunked_fast(&resp_buf[..rhe]);
                    if !chunked {
                        if let Some(resp_cl) = parse_cl_fast(&resp_buf[..rhe]) {
                            body_left = resp_cl.saturating_sub(rn - rhe);
                        }
                    }
                    got_headers = true;
                }
            } else if !chunked {
                body_left = body_left.saturating_sub(rn);
            }
            
            if chunked && !chunked_done {
                chunked_done = is_chunked_complete(&resp_buf);
            }
            
            let write_buf = std::mem::replace(&mut resp_buf, buf_pool.get());
            let (res, returned_buf) = cli.write_all(write_buf).await;
            
            buf_pool.put(returned_buf);
            buf_pool.put(resp_buf);
            
            if res.is_err() { 
                be_healthy = false;
                st.errs.fetch_add(1, Ordering::Relaxed); 
                break;
            }
            
            st.bytes.fetch_add(rn as u64, Ordering::Relaxed);
            
            if got_headers {
                if chunked {
                    if chunked_done { break; }
                } else if body_left == 0 {
                    break;
                }
            }
        }
        
        // Return backend connection to pool (if healthy)
        if be_healthy {
            be_pool.put(be);
        }
        // else: just drop the bad connection
        
        st.reqs.fetch_add(1, Ordering::Relaxed);
        if !ka { return; }
    }
}

/// Fast usize to bytes conversion
#[inline]
fn write_usize(mut n: usize, buf: &mut [u8; 20]) -> usize {
    if n == 0 { buf[0] = b'0'; return 1; }
    let mut i = 20;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    let len = 20 - i;
    buf.copy_within(i..20, 0);
    len
}

fn worker(id: usize, listen: SocketAddr, backend: SocketAddr, st: Arc<Stats>, run: Arc<AtomicBool>) {
    #[cfg(target_os = "linux")]
    unsafe {
        let cpus = libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as usize;
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(id % cpus, &mut set);
        libc::sched_setaffinity(0, std::mem::size_of_val(&set), &set);
    }
    
    let mut rt = monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
        .enable_all().build().expect("rt");
    
    rt.block_on(async {
        let ln = match make_listener(listen) {
            Ok(l) => l,
            Err(e) => { eprintln!("W{} bind: {}", id, e); return; }
        };
        
        // Create backend connection pool for this worker
        let be_pool = Rc::new(BackendPool::new(backend, POOL_SIZE));
        
        println!("Worker {} ok (pool: {} conns)", id, POOL_SIZE);
        
        loop {
            if !run.load(Ordering::Relaxed) { break; }
            if let Ok((s, _)) = ln.accept().await {
                let st = Arc::clone(&st);
                let pool = Rc::clone(&be_pool);
                monoio::spawn(proxy_conn(s, pool, st));
            }
        }
    });
}

fn make_listener(addr: SocketAddr) -> std::io::Result<TcpListener> {
    use std::os::unix::io::FromRawFd;
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, 0);
        if fd < 0 { return Err(std::io::Error::last_os_error()); }
        let one: i32 = 1;
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, &one as *const _ as _, 4);
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEPORT, &one as *const _ as _, 4);
        libc::setsockopt(fd, libc::IPPROTO_TCP, libc::TCP_NODELAY, &one as *const _ as _, 4);
        let bufsz: i32 = 262144;
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_RCVBUF, &bufsz as *const _ as _, 4);
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_SNDBUF, &bufsz as *const _ as _, 4);
        
        let sa = match addr {
            SocketAddr::V4(v4) => {
                let mut s: libc::sockaddr_in = std::mem::zeroed();
                s.sin_family = libc::AF_INET as _;
                s.sin_port = v4.port().to_be();
                s.sin_addr.s_addr = u32::from_ne_bytes(v4.ip().octets());
                s
            }
            _ => { libc::close(fd); return Err(std::io::Error::new(std::io::ErrorKind::Other, "v6")); }
        };
        if libc::bind(fd, &sa as *const _ as _, std::mem::size_of_val(&sa) as _) < 0 {
            libc::close(fd); return Err(std::io::Error::last_os_error());
        }
        if libc::listen(fd, 4096) < 0 {
            libc::close(fd); return Err(std::io::Error::last_os_error());
        }
        let std = std::net::TcpListener::from_raw_fd(fd);
        std.set_nonblocking(true)?;
        TcpListener::from_std(std)
    }
}

fn main() {
    let listen: SocketAddr = LISTEN.parse().unwrap();
    let backend: SocketAddr = BACKEND.parse().unwrap();
    
    println!("╔═══════════════════════════════════════════╗");
    println!("║  HP Proxy V8 - Backend Connection Pool    ║");
    println!("╠═══════════════════════════════════════════╣");
    println!("║  {} → {}              ║", listen, backend);
    println!("║  Workers: {} | Pool: {} | Buf: {}KB       ║", WORKERS, POOL_SIZE, BUFSZ/1024);
    println!("╚═══════════════════════════════════════════╝\n");
    
    let run = Arc::new(AtomicBool::new(true));
    let stats: Vec<Arc<Stats>> = (0..WORKERS).map(|_| Arc::new(Stats::new())).collect();
    
    let hs: Vec<_> = (0..WORKERS).map(|i| {
        let s = Arc::clone(&stats[i]);
        let r = Arc::clone(&run);
        thread::spawn(move || worker(i, listen, backend, s, r))
    }).collect();
    
    let st2: Vec<Arc<Stats>> = stats.iter().map(Arc::clone).collect();
    let r2 = Arc::clone(&run);
    thread::spawn(move || {
        let mut prev = 0u64;
        while r2.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            let (mut t, mut e, mut c, mut b) = (0u64, 0u64, 0u64, 0u64);
            for s in &st2 {
                t += s.reqs.load(Ordering::Relaxed);
                e += s.errs.load(Ordering::Relaxed);
                c += s.conns.load(Ordering::Relaxed);
                b += s.bytes.load(Ordering::Relaxed);
            }
            println!("RPS: {:>6} | Total: {:>9} | Conn: {:>5} | Err: {:>4} | {:.1}MB/s",
                     t - prev, t, c, e, b as f64 / 1_000_000.0);
            prev = t;
        }
    });
    
    println!("Ctrl+C to stop\n");
    for h in hs { h.join().ok(); }
}
