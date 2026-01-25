Tôi sẽ tối ưu hóa server cho 1 triệu connections. Lưu ý: `tokio-uring` và `SO_REUSEPORT` chỉ hoạt động trên Linux. Tôi sẽ tạo version high-performance với conditional compilation. 


Cần cài đặt protoc. Tôi sẽ dùng cách khác - sử dụng pre-generated protobuf hoặc dùng library không cần protoc: 
# 1M+ Users Connect với High-Performance WebSocket Server
Build thành công! 🎉 Đây là tổng quan về ứng dụng WebSocket high-performance:

## Tổng quan Kiến trúc

```
┌──────────────────────────────────────────────────────────────┐
│                High-Performance WebSocket Server              │
│                                                               │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐     ┌──────────┐    │
│  │ Worker 0 │ │ Worker 1 │ │ Worker 2 │ ... │ Worker N │    │
│  │ (CPU 0)  │ │ (CPU 1)  │ │ (CPU 2)  │     │ (CPU N)  │    │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘     └────┬─────┘    │
│       │            │            │                │           │
│       └────────────┴─────┬──────┴────────────────┘           │
│                    SO_REUSEPORT (Linux)                      │
│                          │                                    │
│   ┌──────────┐     ┌─────▼─────┐     ┌───────────┐          │
│   │ DashMap  │     │ Broadcast │     │  Bincode  │          │
│   │256 shards│     │  Channel  │     │  Protocol │          │
│   └──────────┘     └───────────┘     └───────────┘          │
└──────────────────────────────────────────────────────────────┘
```

## Các Tối Ưu Cho 1M+ Connections

| Tối ưu | Mô tả |
|--------|-------|
| **Bincode** | Binary serialization ~10x nhanh hơn JSON |
| **DashMap** | 256 shards, lock-free concurrent HashMap |
| **SO_REUSEPORT** | Kernel load balancing (Linux) |
| **CPU Pinning** | Pin worker thread vào CPU core (Linux) |
| **Broadcast Channel** | Tokio broadcast cho pub/sub hiệu quả |
| **Socket Buffers** | 256KB send/recv buffers |

## Binaries

| Binary | Mô tả |
|--------|-------|
| `server` | Server đơn giản (JSON) |
| `server-hp` | **Server high-performance (Binary)** |
| `client` | Client đơn giản (JSON) |
| `client-binary` | Client binary protocol |
| `bench` | Benchmark tool |

## Chạy thử

```powershell
# Server HP
cargo run --release --bin server-hp

# Client (terminal khác)  
cargo run --release --bin client-binary

# Benchmark 10k connections
cargo run --release --bin bench -- ws://127.0.0.1:8080 10000 30 500
```

**Lưu ý:** `SO_REUSEPORT` và CPU pinning chỉ hoạt động trên Linux. Trên Windows, server vẫn chạy nhưng không có các tối ưu này.
