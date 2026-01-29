Rất đúng hướng. 5 “blind spots” này **đều có thể trở thành bottleneck mới** nếu xử lý hời hợt. Mình sẽ đi **từng điểm**, tập trung vào:
👉 *cái gì là quyết định kiến trúc không được sai*,
👉 *cái gì có thể để Phase sau*,
👉 *và chỗ nào Rust thực sự có lợi thế hơn Traefik*.

Không khen, không vòng vo.

---

# 1️⃣ Connection Pooling Strategy – Đây là bottleneck TIỀM ẨN số 1

Bạn chạm đúng điểm rất nguy hiểm:

> *“Nếu Rust cũng lock per-connection → bottleneck mới”*

### Traefik sai ở đâu?

* Global pool
* `sync.Mutex` mỗi get/put
* Under load → contention y như TLS, LB

### Phân tích các option (thực tế, không lý thuyết)

#### ❌ Global pool + Mutex

* Đơn giản
* **Chết chắc ở scale**
* Không có lý do gì Rust phải đi con đường này

#### ⚠️ Work-stealing pool

* Nghe “ngầu”
* Nhưng:

  * Atomic ops nhiều
  * Cache line bouncing
* Overkill cho Phase 1–2

#### ✅ Per-worker pool (đúng hướng nhất)

* Mỗi tokio worker:

  * Pool riêng
  * Không lock trong fast path
* Nhược điểm:

  * Imbalance

👉 Nhưng imbalance **có thể chấp nhận** vì:

* HTTP keep-alive tự nhiên “sticky”
* Backend connection không cần perfectly balanced

**Quyết định kiến trúc nên chốt:**

* **Per-worker pool là default**
* Có *optional slow-path steal* khi pool empty (không trong hot path)

```rust
// Pseudo
thread_local! {
    static CONN_POOL: RefCell<Pool> = RefCell::new(Pool::new());
}
```

📌 **Confirm**: Đây là quyết định *ảnh hưởng trực tiếp throughput*, nên **nên chốt ngay**.

---

# 2️⃣ HTTP/2 & HTTP/3 – Không phải bottleneck, nhưng là “architecture trap”

### Insight quan trọng

HTTP/2 & HTTP/3 **không làm proxy nhanh hơn**, nhưng:

* Làm **design phức tạp hơn gấp 3**
* Dễ phá zero-copy & ownership model

### Quyết định đúng (mình đồng ý với đề xuất của bạn):

#### ✅ Phase 1: HTTP/1.1 only

* Chốt:

  * Buffer model
  * CP/DP boundary
  * Router
  * Pooling
* Không bị multiplexing làm nhiễu tư duy

#### Phase 2: HTTP/2 (hyper)

* Hyper đã handle stream multiplex
* Proxy chỉ cần:

  * map stream → backend request
* Zero-copy **vẫn OK** nếu:

  * Body streaming
  * Không buffer frame

#### Phase 3: HTTP/3 (quinn)

* Đây là **project con**, không phải feature nhỏ
* QUIC = different transport layer

📌 **Confirm**: Phase 1 = HTTP/1.1 only là **quyết định rất khôn**. Không có downside thực tế.

---

# 3️⃣ Graceful Shutdown & Drain – Không ảnh hưởng throughput, nhưng ảnh hưởng adoption

Hiệu suất thuần:

* Drain **không nằm hot path**
* Nhưng nếu làm sai → production reject ngay

### Điểm cần chốt (ngắn gọn):

* Shutdown signal:

  * `watch::channel` hoặc `Notify`
* Mỗi connection:

  * Check signal **chỉ khi idle**
  * Không poll liên tục

```rust
if shutdown.load(Ordering::Relaxed) {
    // stop accepting new requests
    // allow in-flight to finish
}
```

### Cảnh báo:

* **Không được** check shutdown flag mỗi request frame
* Không được allocate context cho drain

📌 **Confirm**: Đây là production hygiene, không ảnh hưởng perf → có thể chốt nhẹ, không tranh cãi nhiều.

---

# 4️⃣ Error Handling Strategy – Nhỏ nhưng dễ phá hot path

Bạn nhìn rất chuẩn:

> “Error handling trong hot path có thể allocate”

### Nguyên tắc sắt đá:

#### Data plane (hot path):

* ❌ `anyhow`
* ❌ dynamic string
* ❌ backtrace
* ✅ `enum Error { … }`
* ✅ `#[cold]` cho slow error path

```rust
#[derive(Debug)]
enum ProxyError {
    BackendUnavailable,
    Timeout,
    BadRequest,
}
```

#### Control plane:

* `anyhow` OK
* Stacktrace OK
* Allocate OK

📌 **Confirm**: Cái này nên chốt rõ trong proposal, nhưng **không gây tranh cãi kỹ thuật**.

---

# 5️⃣ Observability – Bottleneck “tàng hình”

### Sự thật:

* Metrics: rẻ
* Logging: có thể rất đắt
* Tracing: **rất đắt nếu bật bừa**

Traefik:

* `context.Context`
* log/tracing lan truyền mỗi request

### Quyết định cần chốt:

#### ✅ Metrics

* Always-on
* No allocation per request

#### ⚠️ Tracing

* **Sampling-based**
* Default: OFF hoặc very low rate
* Không tạo span trong hot path nếu không sample

```rust
if sampled {
    let span = tracing::span!(...);
}
```

#### Logging

* Async
* Structured
* Log level runtime-configurable

📌 **Confirm**: Observability **không được phép phá P99**. Đây là constraint, không phải feature.

---

# VI. Đánh giá 5 blind spots theo mức độ “phải chốt ngay”

| # | Điểm              | Phải chốt trước proposal? | Lý do                          |
| - | ----------------- | ------------------------- | ------------------------------ |
| 1 | Connection pool   | ✅ BẮT BUỘC                | Ảnh hưởng throughput trực tiếp |
| 2 | HTTP version      | ✅ NÊN                     | Tránh architectural trap       |
| 3 | Graceful shutdown | ⚠️ Có thể nhẹ             | Không ảnh hưởng perf           |
| 4 | Error handling    | ⚠️ Có thể nhẹ             | Dễ điều chỉnh                  |
| 5 | Observability     | ✅ BẮT BUỘC                | Dễ phá P99 âm thầm             |

---

# VII. Kết luận thẳng

Sau 11 điểm, mình nói rất rõ:

> **Nếu 3 thứ này không chốt cứng, dự án mất lý do tồn tại:**

1. Lock-free data plane
2. Zero-copy + arena buffer model
3. Per-worker connection pooling

Các điểm còn lại:

* không làm proxy nhanh hơn
* nhưng làm proxy **đủ tin cậy để được dùng**

---

## Đề xuất bước tiếp theo (không lan man)

Mình **chưa khuyên** viết `Proposed-Proxy-Now.md` ngay.

👉 Thứ cần viết trước là:

### `Performance & Architecture Contract.md`

* Những thứ **bị cấm** trong hot path
* Những decision **không được đảo**
* Performance gates per phase

Nếu bạn muốn, mình có thể:

* Draft **contract này (1–2 trang)**
  hoặc
* Vẽ **request hot path + cache line reasoning**

Chọn 1. Đi sâu. Không thỏa hiệp.
