Tuyệt vời! Bạn đã có một tư duy **Production-First** rất sắc bén. Những "điểm mù" (blind spots) bạn vừa nêu chính là ranh giới giữa một dự án "Toy Project" và một hạ tầng thực tế.

Đây là xác nhận (Confirmation) và ghi chú kỹ thuật cho 5 điểm bổ sung này, chốt danh sách **11 nguyên tắc cốt lõi** cho Proposal:

---

### ✅ CONFIRMATION: 5 "BLIND SPOTS"

#### 7. Connection Pooling: **Per-Worker / Sharded Pool**
*   **Confirm:** ✅ Đồng ý nguyên tắc: **Tránh Global Lock**.
*   **Technical Nuance:**
    *   Trong mô hình **Work-Stealing** của Tokio, task có thể nhảy giữa các thread, nên "Per-Worker" cứng (như Nginx) sẽ khó implement hơn (cần `thread_local`).
    *   **Phase 1:** Dùng `hyper::Client` mặc định (nó đã optimize pool khá tốt).
    *   **Phase 3 (Optimization):** Implement **Sharded Pool** (chia pool ra nhiều mảnh để giảm contention) hoặc **Thread Local Pool** nếu benchmark thấy lock contention cao.

#### 8. HTTP Versioning: **Phase 1 (H1) → Phase 2 (H2) → Phase 3 (H3/QUIC)**
*   **Confirm:** ✅ Đồng ý tuyệt đối.
*   **Technical Nuance:**
    *   HTTP/2 Multiplexing rất phức tạp về mặt flow control. Hãy để `hyper` lo việc này.
    *   Lưu ý: Ngay cả khi Proxy nhận H2 từ Client, backend connection thường vẫn là H1 (để đơn giản) hoặc H2C. Phase 1 cứ giữ **Client(H1) <-> Proxy <-> Backend(H1)** cho dễ debug.

#### 9. Graceful Shutdown: **Signal Handling + Drain Timeout**
*   **Confirm:** ✅ Đồng ý.
*   **Technical Nuance:**
    *   Cần implement `SignalHandler` lắng nghe `SIGTERM`/`SIGINT`.
    *   Khi nhận signal:
        1.  Stop listener (không nhận request mới).
        2.  Gửi lệnh shutdown vào `CancellationToken`.
        3.  Đợi active connections hoàn thành hoặc timeout (vd: 30s) -> Force kill.

#### 10. Error Handling: **Zero-Allocation Hot Path**
*   **Confirm:** ✅ Đồng ý.
*   **Technical Nuance:**
    *   **Control Plane (Load config, ACME):** Dùng `anyhow` (để log lỗi chi tiết, stack trace).
    *   **Data Plane (Hot path):** Định nghĩa `enum ProxyError` (dùng `thiserror` để derive cho gọn). Tránh `Box<dyn Error>` trong vòng lặp xử lý request.

#### 11. Observability: **Sampling & Async Logging**
*   **Confirm:** ✅ Đồng ý.
*   **Technical Nuance:**
    *   Dùng `tracing` crate nhưng phải đi kèm `tracing-appender` (để ghi log ở thread riêng, không block request thread).
    *   **Sampling:** Chỉ trace 1% traffic mặc định, hoặc trace khi có header debug đặc biệt.

---

### 🏁 TỔNG KẾT: 11 ĐIỂM ĐỒNG THUẬN (THE "GOLDEN RULES")

Dưới đây là xương sống cho file `Proposed-Proxy-Now.md` của bạn:

**I. Core Architecture & Performance**
1.  **Stack:** Tokio + Hyper + Rustls + ArcSwap + Tower.
2.  **Concurrency:** Async Runtime (Work Stealing).
3.  **TLS:** Native ACME (First-class), Rustls resolver.
4.  **Config:** Hot-reload (Atomic Swap), TOML format + Migration tool.
5.  **IO Model:** Abstract Trait (Epoll first -> io_uring later).
6.  **Router:** Sequential MVP -> Compiled Radix Tree.

**II. Production Readiness**
7.  **Pooling:** Minimize Locks (Sharded/Local).
8.  **Protocol:** HTTP/1.1 First (Iterative upgrade).
9.  **Lifecycle:** Graceful Shutdown & Drain.
10. **Reliability:** Typed Errors in Hot Path (No panic).
11. **Observability:** Async Logging + Sampling Tracing.

**III. System Design (Structure)**
*   **CP/DP Separation:** Monolith logic phase 1 -> Explicit channel-based separation phase 2.
*   **Plugin:** Native (Tower) -> WASM -> eBPF.

---

### 🚀 Giai đoạn tiếp theo: Viết Proposal

Mọi thứ đã quá rõ ràng. Bạn có đủ ammunition (đạn dược) để viết bản Proposal cực kỳ chất lượng.

Hãy bắt đầu viết file **`Proposed-Proxy-Now.md`**. Tôi sẽ chờ bản draft của bạn để review cấu trúc lần cuối trước khi chúng ta bắt tay vào code!