//! Benchmark Tool for WebSocket Server

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    Chat = 1,
    Join = 2,
    Leave = 3,
    System = 4,
    Ping = 5,
    Pong = 6,
    Ack = 7,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: u64,
    pub msg_type: MessageType,
    pub user_id: u64,
    pub username: String,
    pub payload: Vec<u8>,
    pub timestamp: i64,
    pub room: String,
}

impl ChatMessage {
    pub fn chat(id: u64, payload: &str) -> Self {
        Self {
            id,
            msg_type: MessageType::Chat,
            user_id: id,
            username: format!("bench_{}", id),
            payload: payload.as_bytes().to_vec(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            room: "general".to_string(),
        }
    }

    pub fn encode(&self) -> Bytes {
        Bytes::from(bincode::serialize(self).unwrap())
    }
}

struct Stats {
    connected: AtomicUsize,
    failed: AtomicUsize,
    msg_sent: AtomicU64,
    msg_recv: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_recv: AtomicU64,
}

impl Stats {
    fn new() -> Self {
        Self {
            connected: AtomicUsize::new(0),
            failed: AtomicUsize::new(0),
            msg_sent: AtomicU64::new(0),
            msg_recv: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_recv: AtomicU64::new(0),
        }
    }
}

async fn run_client(id: u64, url: String, stats: Arc<Stats>, duration: Duration, interval: Duration) {
    let ws = match connect_async(&url).await {
        Ok((stream, _)) => {
            stats.connected.fetch_add(1, Ordering::Relaxed);
            stream
        }
        Err(_) => {
            stats.failed.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };

    let (mut tx, mut rx) = ws.split();
    let stats_rx = Arc::clone(&stats);
    let start = Instant::now();

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = rx.next().await {
            match msg {
                WsMessage::Binary(d) => {
                    stats_rx.msg_recv.fetch_add(1, Ordering::Relaxed);
                    stats_rx.bytes_recv.fetch_add(d.len() as u64, Ordering::Relaxed);
                }
                WsMessage::Text(t) => {
                    stats_rx.msg_recv.fetch_add(1, Ordering::Relaxed);
                    stats_rx.bytes_recv.fetch_add(t.len() as u64, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    });

    let mut n = 0u64;
    while start.elapsed() < duration {
        let msg = ChatMessage::chat(id, &format!("msg_{}", n));
        let bytes = msg.encode();
        let len = bytes.len() as u64;

        if tx.send(WsMessage::Binary(bytes.to_vec())).await.is_ok() {
            stats.msg_sent.fetch_add(1, Ordering::Relaxed);
            stats.bytes_sent.fetch_add(len, Ordering::Relaxed);
        }

        n += 1;
        sleep(interval).await;
    }

    let _ = tx.send(WsMessage::Close(None)).await;
    recv_task.abort();
    stats.connected.fetch_sub(1, Ordering::Relaxed);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let url = args.get(1).cloned().unwrap_or("ws://127.0.0.1:8080".into());
    let num: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(30);
    let limit: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(100);

    println!("🔥 WebSocket Benchmark");
    println!("   URL: {}", url);
    println!("   Connections: {}", num);
    println!("   Duration: {}s", secs);
    println!("   Concurrent limit: {}", limit);
    println!();

    let stats = Arc::new(Stats::new());
    let sem = Arc::new(Semaphore::new(limit));
    let duration = Duration::from_secs(secs);
    let interval = Duration::from_millis(1000);
    let start = Instant::now();

    // Reporter
    let stats_r = Arc::clone(&stats);
    let reporter = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            println!(
                "📊 Conn: {} | Fail: {} | TX: {} | RX: {}",
                stats_r.connected.load(Ordering::Relaxed),
                stats_r.failed.load(Ordering::Relaxed),
                stats_r.msg_sent.load(Ordering::Relaxed),
                stats_r.msg_recv.load(Ordering::Relaxed),
            );
        }
    });

    let mut handles = vec![];
    for id in 0..num {
        let url = url.clone();
        let stats = Arc::clone(&stats);
        let sem = Arc::clone(&sem);

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            run_client(id as u64, url, stats, duration, interval).await;
        }));

        if id % 100 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }

    for h in handles {
        let _ = h.await;
    }

    reporter.abort();

    let elapsed = start.elapsed();
    println!();
    println!("✅ Done in {:.2}s", elapsed.as_secs_f64());
    println!("   Sent: {} msgs ({:.2} MB)", 
        stats.msg_sent.load(Ordering::Relaxed),
        stats.bytes_sent.load(Ordering::Relaxed) as f64 / 1024.0 / 1024.0);
    println!("   Recv: {} msgs ({:.2} MB)",
        stats.msg_recv.load(Ordering::Relaxed),
        stats.bytes_recv.load(Ordering::Relaxed) as f64 / 1024.0 / 1024.0);
    println!("   Failed: {}", stats.failed.load(Ordering::Relaxed));

    Ok(())
}
