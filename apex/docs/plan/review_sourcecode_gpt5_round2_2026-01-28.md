OK, mình đọc **toàn bộ code + số đo** rồi.
Không vòng vo nữa, đây là **phán quyết kỹ thuật cuối cùng cho Phase 1**.

---

# 🧨 PHÁN QUYẾT DỨT KHOÁT

> **Phase 1 KHÔNG được phép dừng ở 32k RPS.**
> Nhưng **Phase 1 cũng KHÔNG nên lao vào custom pool phức tạp.**

👉 **Cả hai hướng bạn đề xuất đều… sai thời điểm.**

* ❌ *“Dừng Phase 1 ở đây”* → **FAIL invariant** (vs Traefik, vs echo)
* ❌ *“Implement custom pool crossbeam / affinity”* → **Phase 2.5 masquerading as Phase 1**

👉 **CÓ một con đường thứ 3**, đúng với proposal gốc, đúng với Phase 1, và **có khả năng đưa bạn lên 60–70k RPS mà KHÔNG viết custom pool**.

---

# 🔥 SỰ THẬT KHÓ NUỐT: Bạn đang benchmark SAI THỨ

## Bạn nghĩ bottleneck là `hyper-util::Client`

❌ Sai **một nửa**.

### Bottleneck thật là:

> **Bạn đang dùng hyper như “client library”,
> trong khi echo server đang dùng hyper như “protocol engine”.**

Hai thứ này **KHÔNG cùng tầng trừu tượng**.

---

# 🔍 SO SÁNH TRỰC DIỆN (RẤT QUAN TRỌNG)

## Echo server

```rust
http1::Builder::new()
    .serve_connection(io, service_fn(handle))
```

* 1 TCP connection
* 1 task
* Request loop chạy **inline**
* Không client abstraction
* Không pooling logic
* Không timeout wrapper
* Không future composition

👉 Đây là **fast path tuyệt đối** của hyper.

---

## Apex proxy (hiện tại)

```rust
self.client.forward_streaming(...)
```

Bên trong:

* hyper-util legacy Client
* Timeout wrapper
* Connection pool
* Trait-based Service
* Task hopping
* Error wrapping
* Client-side state machine

👉 Bạn đang **benchmark framework overhead**, không phải protocol overhead.

---

# 🚨 KẾT LUẬN CỐT LÕI

> **Phase 1 không cần “connection pool”.
> Phase 1 cần “connection ownership”.**

---

# 🧠 GIẢI PHÁP ĐÚNG CHO PHASE 1 (KHÔNG CROSSBEAM, KHÔNG MUTEX)

## ✅ MÔ HÌNH: **Connection-per-backend-task**

### Ý tưởng

* Mỗi backend:

  * **1 task riêng**
  * **1 persistent HTTP/1.1 connection**
* Proxy:

  * Gửi request qua `tokio::mpsc`
  * Nhận response qua `oneshot`

👉 Không pool.
👉 Không lock.
👉 Không affinity hack.
👉 Không work-stealing issue.

---

## 🧱 Kiến trúc

```
           ┌───────────────┐
Request →  │ ProxyService  │
           └───────┬───────┘
                   │ mpsc
                   ▼
        ┌────────────────────────┐
        │ BackendTask (1/backend)│
        │ - owns SendRequest     │
        │ - loop send_request    │
        └──────────┬─────────────┘
                   │
                   ▼
              TCP connection
```

---

## ✨ Vì sao cách này ĐÚNG cho Phase 1

| Tiêu chí                | Đánh giá            |
| ----------------------- | ------------------- |
| Không Mutex             | ✅                   |
| Không crossbeam         | ✅                   |
| Không custom pool       | ✅                   |
| Không hyper-util Client | ✅                   |
| Gần echo server model   | ✅                   |
| Phù hợp proposal        | ✅                   |
| Đạt 60–70k RPS?         | **RẤT CÓ KHẢ NĂNG** |

---

# 📌 TẠI SAO thread_local + pool FAIL (bạn đã thấy)

Bạn đúng khi kết luận:

> ❌ thread_local không work với tokio work-stealing

→ Vì request **không stick với OS thread**.
→ Đây là lý do proposal gốc **chưa bao giờ hứa thread_local Phase 1**.

---

# 🧪 VÌ SAO connection-per-request = 2.7k RPS

* TCP handshake
* HTTP handshake
* No keep-alive reuse

→ Đây là **expected**. Không phải failure.

---

# 🎯 LỘ TRÌNH CHUẨN (KHÔNG ĐI TẮT)

## Phase 1 (bắt buộc)

❌ hyper-util::Client
❌ connection pool
❌ per-worker pool

✅ **BackendTask + mpsc**
✅ Persistent SendRequest
✅ 1 backend = 1 task

👉 **Target:** 60–70k RPS
👉 **Overhead:** ~15–25% (chấp nhận được)

---

## Phase 2

* Pool nhiều connections / backend
* Adaptive concurrency
* Health + circuit breaker gắn vào task
* Backpressure-aware routing

---

## Phase 3

* Per-worker pool
* Cache-line tuning
* io_uring
* HTTP/2, H3

---

# 🧨 PHÁN QUYẾT CUỐI

> **Phase 1 KHÔNG được dừng.
> Nhưng Phase 1 cũng KHÔNG được “đốt não” vào custom pool.**

👉 **BackendTask model là con đường duy nhất vừa đúng kỹ thuật, vừa đúng chiến lược.**

---

## Bạn chọn bước tiếp theo nào?

**A.** Mình viết **prototype BackendTask + mpsc** (drop-in thay HttpClient)
**B.** Mình vẽ **sequence diagram + invariants** cho BackendTask
**C.** Mình chỉ ra **exact code cần xóa / thay** trong repo hiện tại
**D.** Dừng Phase 1 (⚠️ mình sẽ phản đối)

Chọn **1**.
