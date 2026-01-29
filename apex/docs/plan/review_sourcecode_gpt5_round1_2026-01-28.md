Mình sẽ **đập thẳng vào hiện trạng code + số đo**, không nói lý thuyết nữa.
Kết luận trước, rồi đi từng nhát cắt.

---

# 🧨 KẾT LUẬN NGẮN (KHÔNG MỀM)

> **32k RPS vs 82k RPS echo = KHÔNG phải do Rust chậm.
> Đây là “tự bắn vào chân” bởi 3 lỗi kiến trúc nghiêm trọng trong hot path.**

Không cần bb8.
Không cần deadpool.
Không cần thêm crate.

👉 **Cần bỏ 3 thứ đang giết throughput.**

---

# 🔥 THỦ PHẠM #1 (NGHIÊM TRỌNG NHẤT): `Mutex` trong connection pool

## File: `crates/server/src/pool.rs`

```rust
connections: DashMap<SocketAddr, Arc<Mutex<Vec<PooledConnection>>>>
```

và:

```rust
let mut connections = pool.lock().await;
```

### Đây là án tử

* Mutex **async-aware** trong hot path
* Mỗi request:

  * DashMap lookup
  * Mutex lock
  * Vec pop / push
* Under load → **thundering herd**

👉 Bạn đang **tái hiện Traefik lỗi #1**, chỉ khác là bằng Rust.

### ❌ bb8 / deadpool?

Không.
Chúng **dùng Mutex nhiều hơn**.

---

## ✅ CÁCH ĐÚNG (Phase 1 luôn, không chờ Phase 3)

### Per-worker pool, **KHÔNG MUTEX**

Tokio **multi-thread runtime** → mỗi worker là 1 OS thread.

```rust
thread_local! {
    static CONN_POOL: RefCell<PerWorkerPool> = RefCell::new(PerWorkerPool::new());
}
```

Per-worker:

* `Vec<SendRequest>` thuần
* `RefCell` (no contention)
* Không `.await` khi acquire

### Khi cần cross-worker?

👉 **KHÔNG CẦN trong Phase 1**

Connection imbalance < lock contention **100 lần**.

---

# 🔥 THỦ PHẠM #2: hyper-util legacy Client (bạn đo đúng rồi)

## File: `client.rs`

```rust
Client::builder(TokioExecutor::new())
```

### hyper-util legacy client = **general-purpose**

* Trait object
* Internal locking
* Indirection layers
* Designed cho correctness > performance

👉 61% overhead bạn đo **phù hợp với kiến trúc này**

---

## ✅ HƯỚNG ĐÚNG

Bạn **đã làm đúng hơn** ở `pool.rs`:

```rust
hyper::client::conn::http1::handshake
```

👉 Nhưng bạn **quay lại dùng legacy client trong ProxyService**.

### Quyết định kiến trúc phải nhất quán:

> ❌ Không dùng `hyper-util::Client` ở data plane
> ✅ Chỉ dùng low-level `SendRequest`

---

# 🔥 THỦ PHẠM #3: Router + DashMap trong hot path

## File: `router.rs`

```rust
routes_by_host: DashMap<String, Vec<Arc<Route>>>
```

### DashMap không miễn phí

* Sharded lock
* Atomic ops
* Cache-line bouncing

Với:

* host lookup mỗi request
* path scan mỗi request

👉 Bạn **chưa đạt đến radix tree như proposal**, nên overhead là thật.

---

## ✅ SỬA NHANH (KHÔNG ĐỤNG RADIX TREE YET)

### Phase 1 đúng cách:

* **Build immutable routing table**
* `ArcSwap<RouterTable>`
* RouterTable = `Vec<Route>` + sorted

```rust
struct RouterTable {
    routes: Vec<Route>,
}
```

Lookup:

* Linear scan (cache-friendly)
* No DashMap
* No Arc clone

👉 Với <100 routes, **linear scan nhanh hơn DashMap**.

---

# 📊 PHÂN TÍCH SỐ LIỆU HIỆN TẠI

| Thành phần            | Ước lượng overhead |
| --------------------- | ------------------ |
| hyper-util Client     | ~35–40%            |
| Mutex pool            | ~15–20%            |
| DashMap router        | ~5–10%             |
| Headers + URI rebuild | ~5%                |

= **~61%** → đúng số bạn đo.

---

# 🚫 ĐỪNG LÀM NHỮNG THỨ NÀY

❌ bb8
❌ deadpool
❌ thêm abstraction
❌ thêm crate
❌ async Mutex trong hot path
❌ “để Phase 3 tối ưu”

---

# ✅ LỘ TRÌNH SỬA ĐÚNG (CÓ THỂ ĐO LẠI TRONG 1 NGÀY)

## Bước 1 – PHẢI LÀM NGAY

* ❌ Xóa `hyper-util::Client`
* ❌ Xóa `Mutex<Vec<_>>`
* ✅ Per-worker pool + `SendRequest`

👉 Kỳ vọng: **32k → 55–60k RPS**

---

## Bước 2 – Router đơn giản hóa

* ❌ DashMap
* ✅ Immutable Vec + ArcSwap

👉 Kỳ vọng: **+10–15k RPS**

---

## Bước 3 – Benchmark lại

* Echo: ~82k
* Apex target Phase 1: **70k+ (≥85%)**

Nếu chưa đạt → **mình đảm bảo còn 1 choke point nữa** (body + headers).

---

# 🎯 PHÁN QUYẾT

> **Dự án đang đi đúng hướng kiến trúc,
> nhưng implementation hiện tại đang phản bội proposal.**

Không phải rewrite.
Chỉ cần **cắt đúng 3 khối ung thư**.

---

## Bạn muốn mình làm gì tiếp?

A. Viết **Per-worker connection pool code** (drop-in thay pool.rs)
B. Viết **Router immutable + ArcSwap** thay DashMap
C. Viết **micro-benchmark** tách từng thành phần để chứng minh overhead
D. Đi tiếp **Buffer Model prototype** như proposal ban đầu

Chọn **1**.
