Dựa trên những phân tích rất chi tiết từ source code Traefik và các mục tiêu hiệu suất bạn đã đề ra, tôi hoàn toàn đồng ý: **Nếu không giải quyết triệt để các bottleneck này ngay từ thiết kế (Design Phase), dự án sẽ thất bại về mặt mục tiêu hiệu suất.**

Dưới đây là quyết định chốt (Final Decision) cho 6 điểm chưa đồng thuận, dựa trên nguyên tắc **"Performance First"**:

---

### ✅ CHỐT: Các Quyết định Kiến trúc (Architectural Decisions)

Tôi xác nhận **ĐỒNG THUẬN** với cả 6 điểm đề xuất của bạn, kèm theo các ghi chú kỹ thuật cụ thể để đội dev không đi sai hướng:

#### 1. Buffer Ownership Model: **Zero-Copy + Arena**
*   **Quyết định:** **CONFIRMED**.
*   **Lý do:** Traefik chậm do GC và Allocation trên mỗi request. Rust mà clone dữ liệu thì cũng sẽ chậm y hệt.
*   **Kỹ thuật thực thi:**
    *   Sử dụng `bytes::Bytes` để giữ reference count đến buffer gốc của OS/Network.
    *   Header parsing không được tạo `String` mới, mà phải slice từ `Bytes` gốc.
    *   Sử dụng crate `bumpalo` (Arena allocator) cho các struct tồn tại ngắn hạn trong 1 request lifecycle. Giải phóng bộ nhớ trong 1 nốt nhạc (pointer reset) thay vì free từng object.

#### 2. CP/DP Boundary: **Lock-Free Reads (RCU)**
*   **Quyết định:** **CONFIRMED**.
*   **Lý do:** Lock contention là kẻ thù số 1 của concurrency. `RwLock` vẫn có chi phí, và khi có Write (reload config), toàn bộ Read bị block -> gây ra latency spike (bottleneck #5 của Traefik).
*   **Kỹ thuật thực thi:**
    *   Sử dụng `arc-swap` (triển khai RCU - Read Copy Update).
    *   Data Plane luôn `load()` được config cũ hoặc mới ngay lập tức mà không bao giờ phải chờ đợi Control Plane.
    *   Tuyệt đối cấm `Mutex` hoặc `RwLock` trong đường dẫn xử lý request (Hot Path).

#### 3. Router Metrics: **MANDATORY từ Phase 1**
*   **Quyết định:** **CONFIRMED**.
*   **Lý do:** "Sequential matching" là chấp nhận được cho MVP để code nhanh, NHƯNG ta phải biết khi nào nó trở thành vấn đề. Nếu không đo, ta sẽ không biết nó chậm hơn Traefik.
*   **Kỹ thuật thực thi:**
    *   Tích hợp `metrics` crate.
    *   Đo thời gian từ lúc nhận request -> lúc xác định được Backend service.
    *   Log warning nếu `p99 > 50μs`.

#### 4. Middleware Stack: **Generic Composition (Static Dispatch)**
*   **Quyết định:** **CONFIRMED**.
*   **Lý do:** Dynamic dispatch (`Box<dyn Service>`) ngăn cản compiler tối ưu hóa (inlining code). Traefik bị chậm do Go interface dispatch.
*   **Kỹ thuật thực thi:**
    *   Sử dụng pattern của `tower`: `ServiceBuilder::new().layer(A).layer(B).service(C)`.
    *   Kết quả là một type khổng lồ (monomorphization) nhưng chạy cực nhanh vì code được inline toàn bộ.
    *   Chỉ dùng `Box` ở ranh giới plugin (nếu sau này hỗ trợ WASM/Lua).

#### 5. TLS Cache: **DashMap (Concurrent Hash Map)**
*   **Quyết định:** **CONFIRMED**.
*   **Lý do:** Traefik dùng `sync.RWMutex` cho certificate store. Khi traffic cao, các luồng tranh nhau cái lock này chỉ để đọc certificate.
*   **Kỹ thuật thực thi:**
    *   Dùng `dashmap` hoặc `scc`. Các map này thiết kế cho high concurrency, sử dụng sharded locks hoặc lock-free bucket, giảm thiểu contention về gần 0.

#### 6. Performance Gate: **CI/CD Benchmark**
*   **Quyết định:** **CONFIRMED**.
*   **Lý do:** Code Rust an toàn bộ nhớ không có nghĩa là nó nhanh. Logic sai vẫn chậm.
*   **Kỹ thuật thực thi:**
    *   Viết script `k6` hoặc `wrk`.
    *   Trước khi merge PR vào `main`, chạy benchmark so sánh với bản build trước đó. Nếu throughput giảm > 5% -> Block merge.

---

### 🚀 Kế hoạch hành động tiếp theo

Với việc chốt 6 điểm này, cấu trúc dự án (Scaffold) cần phản ánh ngay tư duy này:

1.  **Cargo.toml:** Thêm ngay `bytes`, `arc-swap`, `dashmap`, `bumpalo`, `tower`.
2.  **Core Structs:** Định nghĩa `ProxyRequest` với lifetime (zero-copy) ngay từ đầu. Đừng viết `String`, hãy viết `&'a str` hoặc `Bytes`.
3.  **Benchmark Baseline:** Dựng một môi trường đo Traefik ngay bây giờ để lấy số liệu làm chuẩn (Baseline).

**Kết luận:** Chúng ta không viết lại Traefik bằng Rust. Chúng ta viết một Proxy hiệu năng cao, khắc phục các lỗi kiến trúc của Traefik bằng lợi thế của Rust.

Bạn có thể tiến hành cập nhật **Technical Specification (Tech Spec)** với 6 quyết định "cứng" này.