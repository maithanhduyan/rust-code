//! HP Echo Server V2 - io_uring Edition
//!
//! Simple echo backend for testing the proxy

use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::net::{TcpListener, TcpStream};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const LISTEN_ADDR: &str = "0.0.0.0:9001";
const NUM_WORKERS: usize = 2;

const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: keep-alive\r\n\r\nHello, World!";

#[repr(align(64))]
struct Stats {
    requests: AtomicU64,
    _pad: [u64; 7],
}

impl Stats {
    fn new() -> Self {
        Self {
            requests: AtomicU64::new(0),
            _pad: [0; 7],
        }
    }
}

#[inline(always)]
fn find_crlf2(data: &[u8]) -> Option<usize> {
    let len = data.len();
    if len < 4 { return None; }
    for i in 0..len - 3 {
        if data[i] == b'\r' && data[i + 1] == b'\n' && data[i + 2] == b'\r' && data[i + 3] == b'\n' {
            return Some(i + 4);
        }
    }
    None
}

async fn handle_connection(mut stream: TcpStream, stats: Arc<Stats>) {
    let mut buf = vec![0u8; 4096];

    loop {
        // Read request
        let mut total = 0usize;
        loop {
            let read_buf = buf.split_off(total);
            let (res, read_buf) = stream.read(read_buf).await;
            buf = [&buf[..total], &read_buf[..]].concat();

            match res {
                Ok(0) => return,
                Ok(n) => {
                    total += n;
                    if find_crlf2(&buf[..total]).is_some() {
                        break;
                    }
                    if total >= 4000 {
                        return;
                    }
                }
                Err(_) => return,
            }
        }

        // Send response
        let resp = RESPONSE.to_vec();
        let (res, _) = stream.write_all(resp).await;
        if res.is_err() {
            return;
        }

        stats.requests.fetch_add(1, Ordering::Relaxed);

        // Reset buffer
        buf.truncate(4096);
        buf.resize(4096, 0);
    }
}

fn worker_thread(id: usize, listen_addr: SocketAddr, stats: Arc<Stats>, running: Arc<AtomicBool>) {
    let mut rt = monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
        .enable_all()
        .build()
        .expect("Failed to build runtime");

    rt.block_on(async move {
        let listener = match create_reuseport_listener(listen_addr) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Echo worker {} failed: {}", id, e);
                return;
            }
        };

        println!("Echo worker {} listening (io_uring)", id);

        loop {
            if !running.load(Ordering::Relaxed) {
                break;
            }

            match listener.accept().await {
                Ok((stream, _)) => {
                    let stats = Arc::clone(&stats);
                    monoio::spawn(async move {
                        handle_connection(stream, stats).await;
                    });
                }
                Err(_) => {}
            }
        }
    });
}

fn create_reuseport_listener(addr: SocketAddr) -> std::io::Result<TcpListener> {
    use std::os::unix::io::FromRawFd;

    unsafe {
        let fd = libc::socket(
            libc::AF_INET,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        );
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }

        let optval: libc::c_int = 1;
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &optval as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEPORT,
            &optval as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );

        let sockaddr = match addr {
            SocketAddr::V4(v4) => {
                let mut sa: libc::sockaddr_in = std::mem::zeroed();
                sa.sin_family = libc::AF_INET as libc::sa_family_t;
                sa.sin_port = v4.port().to_be();
                sa.sin_addr.s_addr = u32::from_ne_bytes(v4.ip().octets());
                sa
            }
            SocketAddr::V6(_) => {
                libc::close(fd);
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "IPv6 not supported"));
            }
        };

        if libc::bind(fd, &sockaddr as *const _ as *const libc::sockaddr, std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t) < 0 {
            libc::close(fd);
            return Err(std::io::Error::last_os_error());
        }

        if libc::listen(fd, 4096) < 0 {
            libc::close(fd);
            return Err(std::io::Error::last_os_error());
        }

        let std_listener = std::net::TcpListener::from_raw_fd(fd);
        std_listener.set_nonblocking(true)?;
        TcpListener::from_std(std_listener)
    }
}

fn main() {
    let listen_addr: SocketAddr = LISTEN_ADDR.parse().unwrap();

    println!("=== HP Echo Server V2 (io_uring) ===");
    println!("Listen: {}", listen_addr);
    println!("Workers: {}", NUM_WORKERS);
    println!();

    let running = Arc::new(AtomicBool::new(true));
    let stats: Vec<Arc<Stats>> = (0..NUM_WORKERS).map(|_| Arc::new(Stats::new())).collect();

    let mut handles = Vec::new();
    for i in 0..NUM_WORKERS {
        let s = Arc::clone(&stats[i]);
        let r = Arc::clone(&running);
        handles.push(thread::spawn(move || {
            worker_thread(i, listen_addr, s, r);
        }));
    }

    let stats_clone: Vec<Arc<Stats>> = stats.iter().map(Arc::clone).collect();
    let running2 = Arc::clone(&running);
    thread::spawn(move || {
        let mut prev = 0u64;
        while running2.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            let total: u64 = stats_clone.iter().map(|s| s.requests.load(Ordering::Relaxed)).sum();
            println!("RPS: {}", total - prev);
            prev = total;
        }
    });

    for h in handles {
        h.join().ok();
    }
}
