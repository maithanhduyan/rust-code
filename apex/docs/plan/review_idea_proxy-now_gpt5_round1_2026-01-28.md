Mình sẽ làm đúng 2 việc bạn yêu cầu: **review proposal (rất gắt, ở mức kiến trúc)** và **chốt deep dive tiếp theo**. Không khen, không vòng vo.

---

# 1️⃣ Review tổng thể `Proposed-Proxy-Now.md`

## Kết luận nhanh

> **Proposal này ĐỦ ĐIỀU KIỆN để trở thành “luật kỹ thuật” và bắt đầu code.**
> Không còn thiếu phần nền tảng nào *mang tính sống còn*.

Nhưng có **6 điểm cần chỉnh / làm rõ**, nếu không sẽ **tự tạo mìn cho Phase 1–2**.

---

## 🔴 Review theo mức độ rủi ro

### ❗ 1. Mâu thuẫn nhỏ nhưng nguy hiểm: `ConnectionPool::acquire` sync vs async

Trong trait:

```rust
fn acquire(&self, target: &Uri) -> Result<Self::Connection, Self::Error>;
```

Nhưng trong hot path:

```rust
let response = conn.send_request(ctx).await?;
```

### Vấn đề

* `acquire()` sync → OK cho per-worker pool
* Nhưng **hyper impl Phase 1 gần như chắc chắn cần async**
* Nếu Phase 1 “giả sync” bằng block_on / internal await → **vi phạm invariant ngầm**

### Sửa bắt buộc (rất nhỏ, nhưng quan trọng)

Định nghĩa **2-layer API**, không trộn:

```rust
trait ConnectionPool {
    type Conn: PooledConnection;

    fn try_acquire(&self, target: &Uri) -> Option<Self::Conn>;
    async fn acquire_slow(&self, target: &Uri) -> Result<Self::Conn, Error>;
}
```

* Hot path: `try_acquire` (no await)
* Fallback: `acquire_slow` (cold path)

👉 Điều này **giữ được Phase 3 không rewrite flow**.

---

### ❗ 2. `ProxyConnection { inner: Box<dyn AsyncReadWrite> }` vi phạm invariant #3

Bạn đã **tự vi phạm** luật sắt:

> ❌ BANNED: `Box<dyn>` trong core stack

### Cách sửa (không phá thiết kế)

* `Box<dyn>` **chỉ tồn tại ở boundary**
* Data plane dùng generic

Ví dụ:

```rust
struct ProxyConnection<C: AsyncRead + AsyncWrite + Unpin> {
    inner: C,
}
```

Hyper impl có thể:

```rust
type HyperConn = ProxyConnection<hyper::client::conn::SendRequest<Body>>;
```

👉 Nếu giữ `Box<dyn>` ở đây, performance contract **mất giá trị pháp lý**.

---

### ❗ 3. Arena lifetime: Proposal đúng hướng nhưng chưa “khóa luật”

Bạn để Open Question, nhưng **arena lifetime ảnh hưởng toàn bộ API** → không thể để mơ hồ.

**Phán quyết kiến trúc (nên ghi thẳng vào Proposal):**

> **Phase 1–2: Arena = per-request, drop sau response**
> **Phase 3+: có thể chuyển sang per-connection reuse**

Lý do:

* HTTP/1.1 keep-alive + streaming body
* Arena per-connection rất dễ leak lifetime

👉 Không quyết điểm này → middleware API không thể ổn định.

---

### ❗ 4. `hyper::Body` trong `RequestContext` phá zero-copy claim (một phần)

```rust
pub body: hyper::Body,
```

Sự thật:

* hyper::Body **không zero-copy end-to-end**
* Nó zero-copy *chunk-level*, nhưng có internal buffering

👉 Không sai, nhưng **proposal nên trung thực hơn**:

**Sửa wording đề xuất:**

> “Zero-copy headers & metadata, streaming body with bounded buffering”

Không sửa → benchmark phase 1 sẽ gây tranh cãi không cần thiết.

---

### ❗ 5. Phase 3 scope hơi “tham” (nhưng chấp nhận được)

Phase 3 có:

* io_uring
* WASM
* HTTP/3
* Distributed tracing
* Docker provider

Đây là **3 roadmap gộp làm 1**.

👉 OK vì:

* Phase 3 không blocking Phase 1–2
* Nhưng **nên đánh dấu “optional / stretch”**

Không cần sửa lớn, chỉ cần tag rõ.

---

### ❗ 6. Performance Gates: thiếu “memory ceiling”

Bạn có:

* RPS
* P99

Nhưng **proxy chết thường do memory**, không phải CPU.

Đề xuất bổ sung 1 dòng (rất đáng giá):

```markdown
- Memory growth under steady load: < 1% / hour
```

---

## ✅ Những gì ĐÃ RẤT TỐT (không cần sửa)

* Invariants đặt **trước architecture** → đúng
* Control plane / data plane tách rõ → đúng
* Phase exit criteria rõ → cực kỳ đúng
* Decision log → rất hiếm proposal làm được
* Open Questions đặt đúng chỗ → không lan man

---

# 2️⃣ Chọn Deep Dive tiếp theo: Buffer Model hay Connection Pool?

### Phán quyết thẳng:

> **DEEP DIVE BUFFER MODEL TRƯỚC.**

Không phải vì connection pool không quan trọng, mà vì:

---

## Vì sao Buffer Model là “điểm khóa cổ chai”

### 1️⃣ Buffer model quyết định:

* RequestContext lifetime
* Middleware API
* Body forwarding
* HTTP/2/3 khả thi hay không

Nếu buffer model sai:

* Connection pool tối ưu đến mấy cũng vô nghĩa

---

### 2️⃣ Connection pool **có thể thay thế**, buffer model thì không

* Pool = implementation detail
* Buffer model = **API contract**

---

### 3️⃣ Traefik bottleneck #2 = allocation & buffering

Bạn đã xác định đúng. Đi né nó trước là sai chiến lược.

---

## 🎯 Quyết định chính thức

✔ Proposal **được approve về mặt cấu trúc & nội dung**, với 4 chỉnh sửa nhỏ bắt buộc
✔ **Deep Dive tiếp theo: Buffer Model**

---

# 3️⃣ Bước tiếp theo (rõ ràng, không mơ hồ)

### Ngay bước kế tiếp (nên làm):

**Viết prototype cực nhỏ**:

* 1 accept
* 1 request
* Zero-copy metadata
* Streaming body
* Arena per-request

👉 Không router, không TLS, không middleware.

### Câu hỏi prototype phải trả lời được:

1. Arena reset lúc nào là an toàn?
2. Có allocation nào “ẩn” không?
3. hyper::Body forward có backpressure ổn không?
4. P99 latency của **request rỗng** là bao nhiêu?

---

Nếu bạn muốn:

* Mình **đề xuất skeleton code cho Buffer Model prototype** (≈ 200 LOC)
* Hoặc viết **benchmark harness trước** để đo allocation & cache miss

👉 Chọn 1:
**A. Buffer Model prototype code**
**B. Benchmark harness trước, code sau**

Chọn.
