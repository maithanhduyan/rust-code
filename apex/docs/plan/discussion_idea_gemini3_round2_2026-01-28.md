Tôi hoàn toàn **đồng thuận** với 6 điểm chốt hạ này. Đây là sự kết hợp thực tế nhất giữa kiến trúc lý tưởng (NGINX/Pingora) và khả năng hiện thực hóa (Traefik/Caddy).

Dưới đây là confirm chi tiết và một vài **lưu ý kỹ thuật nhỏ** (technical nuances) để bạn đưa vào bản draft `Proposed-Proxy-Now.md` cho chặt chẽ hơn:

---

### ✅ CONFIRMATION CHECKLIST

#### 1. CP/DP Separation: **Phase 1 (Monolith `ArcSwap`) → Phase 2 (Actor/Channel)**
*   **Confirm:** ✅ Đồng ý.
*   **Note:** Ngay cả ở Phase 1, hãy cố gắng group state vào một struct `AppState` duy nhất. Dùng `ArcSwap<AppState>` thay vì nhiều biến `ArcSwap` lẻ tẻ để đảm bảo tính nhất quán (consistency) khi reload (tránh trường hợp config router mới nhưng config TLS vẫn cũ trong tích tắc).

#### 2. Router: **Sequential MVP → Compiled (Radix/DFA)**
*   **Confirm:** ✅ Đồng ý.
*   **Note:** Để performance không quá tệ ở MVP, nên tổ chức theo cấu trúc 2 tầng:
    *   Tầng 1: `HashMap<Hostname, Routes>` (O(1) lookup host).
    *   Tầng 2: `Vec<Route>` (Sequential match path).
    *   Tránh duyệt phẳng `Vec<Route>` cho toàn bộ request.

#### 3. IO Strategy: **Abstract Trait (Tokio defaults) → io_uring later**
*   **Confirm:** ✅ Đồng ý.
*   **Note:** Đừng tạo trait quá phức tạp kiểu `ngx_event_actions_t` vội. Chỉ cần wrap `TcpListener` và `TcpStream` vào một Trait `NetworkAdapter` đơn giản. Phase 1 cứ dùng `tokio::net` native để tận dụng ecosystem.

#### 4. Plugin: **Tower (Native) → WASM → eBPF**
*   **Confirm:** ✅ Đồng ý.
*   **Note:** Tower là tiêu chuẩn vàng. Hãy thiết kế core logic (RateLimit, Auth) chính là các Tower Layer. Sau này WASM chỉ là một Layer bọc lấy WASM Runtime. Cách tiếp cận này giúp kiến trúc đồng nhất.

#### 5. Compatibility: **New Config (TOML) + Migration Tool**
*   **Confirm:** ✅ Đồng ý.
*   **Note:** Quyết định sáng suốt. Việc parse `nginx.conf` (với đống `if`, `rewrite` magic của nó) là một cái hố không đáy. Tool convert tĩnh (CLI) là đủ.

#### 6. TLS Routing Signal: **Phase 2**
*   **Confirm:** ✅ Đồng ý.
*   **Note:** Tính năng này cần chọc sâu vào `rustls::Acceptor`. Để Phase 2 là hợp lý.

---

### 💡 Insight từ source code Traefik (đã đọc)

Bạn đã đọc `traefik/pkg/provider/acme/local_store.go`, điều này rất tốt.
*   **Traefik approach:** Họ lưu ACME certificates vào một file JSON (`acme.json`). Khi chạy cluster mode, họ dùng KV Store (Consul/Etcd).
*   **Áp dụng cho Rust-Proxy:**
    *   Ở Phase 1, mình làm giống Traefik: **File-based storage** (lưu state ACME ra file JSON/SQLite).
    *   Đừng cố lưu state chỉ trong RAM, vì restart sẽ mất sạch cert -> Hit rate limit của Let's Encrypt ngay lập tức.
    *   **Action:** Trong proposal, phần ACME cần ghi rõ: "Storage backend: Local File System (JSON/Sled) for MVP".

---

### 🚀 Next Step

Mời bạn soạn thảo **`Proposed-Proxy-Now.md`**.
Hãy cấu trúc nó như một **Design Document** (RFC) chuẩn kỹ thuật:

1.  **Abstract**: Mục tiêu dự án.
2.  **Architecture**: Diagram (CP/DP), Data Flow.
3.  **Core Modules**: Chi tiết nhiệm vụ từng module (Router, TLS, Middleware).
4.  **Phase Roadmap**:
    *   Phase 1: "The Working Skeleton" (HTTP/1.1, Static Config, Tokio).
    *   Phase 2: "The Smart Proxy" (ACME, Hot Reload, TLS Signals).
    *   Phase 3: "Performance & Scale" (WASM, io_uring, Metrics).
5.  **API Contracts**: Interface giữa CP và DP (Struct config trông thế nào).

Tôi đã sẵn sàng review proposal của bạn!