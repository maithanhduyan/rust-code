## Điểm cần chú ý - Blind Spots chưa thảo luận

Sau khi đồng thuận 6 điểm hiệu suất, tôi thấy còn **5 điểm quan trọng** chưa được đề cập sâu:

---

### 🟡 1. Connection Pooling Strategy (Chưa ai bàn)

Traefik có connection pool nhưng vẫn dùng lock:
```go
// traefik/pkg/proxy/fast/connpool.go
type connPool struct {
    lock sync.Mutex  // Lock mỗi lần lấy/trả connection
}
```

**Vấn đề**: Nếu Rust cũng lock per-connection → bottleneck mới.

| Option | Pros | Cons |
|--------|------|------|
| Global pool + Mutex | Đơn giản | Lock contention |
| Per-worker pool | Lock-free trong worker | Connection imbalance |
| Work-stealing pool | Balanced | Phức tạp |

**Đề xuất**: Per-worker pool (mỗi tokio worker có pool riêng), steal khi cần.

---

### 🟡 2. HTTP/2 & HTTP/3 Multiplexing (Chưa chốt)

Các thảo luận focus HTTP/1.1. Nhưng:
- HTTP/2: 1 connection, nhiều streams → khác hoàn toàn connection model
- HTTP/3 (QUIC): UDP-based, cần quinn crate

**Vấn đề**: Zero-copy model có hoạt động với HTTP/2 frame multiplexing không?

**Đề xuất**: 
- Phase 1: HTTP/1.1 only (chốt architecture)
- Phase 2: HTTP/2 via hyper (đã hỗ trợ)
- Phase 3: HTTP/3 via quinn

---

### 🟡 3. Graceful Shutdown & Drain (Critical cho production)

Khi reload/shutdown:
- Connections đang active phải drain
- Không drop request giữa chừng

Traefik dùng `context.Context` propagation. Rust cần:
```rust
// Graceful shutdown với tokio
let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

// Mỗi connection check shutdown signal
tokio::select! {
    _ = handle_request() => {},
    _ = shutdown_rx.changed() => { /* drain */ }
}
```

**Đề xuất**: Định nghĩa drain timeout trong config (default 30s).

---

### 🟡 4. Error Handling Strategy (Chưa thống nhất)

Traefik dùng Go error wrapping. Rust có nhiều options:

| Approach | Use case |
|----------|----------|
| `anyhow` | Application errors, good stacktrace |
| `thiserror` | Library errors, typed |
| Custom enum | Maximum control |

**Vấn đề**: Error handling trong hot path có thể allocate.

**Đề xuất**: 
- Hot path: Custom enum, no allocation
- Control plane: `anyhow` cho flexibility
- Never panic in data plane

---

### 🟡 5. Observability Beyond Metrics (Tracing, Logging)

Đã chốt metrics, nhưng:
- **Tracing**: OpenTelemetry integration? `tracing` crate overhead?
- **Logging**: Structured logs allocate per-log

**Vấn đề**: `tracing` spans có thể allocate mỗi request.

**Đề xuất**:
- Dùng `tracing` với **sampling** (không trace 100% requests)
- Log level runtime configurable
- Async logging (không block hot path)

---

## 📋 Checklist bổ sung

| # | Điểm | Đề xuất | Cần confirm? |
|---|------|---------|--------------|
| 7 | **Connection pool** | Per-worker, lock-free trong worker | ⬜ |
| 8 | **HTTP version** | Phase 1 = HTTP/1.1 only | ⬜ |
| 9 | **Graceful shutdown** | Drain với configurable timeout | ⬜ |
| 10 | **Error handling** | Enum hot path, anyhow control plane | ⬜ |
| 11 | **Tracing** | Sampling-based, async logging | ⬜ |

---

## 🎯 Tổng kết: 11 điểm đồng thuận

Nếu confirm thêm 5 điểm trên, proposal sẽ có đầy đủ:

| Category | Điểm |
|----------|------|
| **Performance Core** | 1-6 (đã chốt) |
| **Production Readiness** | 7-11 (cần confirm) |

Bạn có muốn thảo luận thêm về 5 điểm này, hay đủ để viết `Proposed-Proxy-Now.md`?