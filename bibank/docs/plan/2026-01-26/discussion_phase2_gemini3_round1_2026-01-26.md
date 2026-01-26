Chào bạn, bản **Proposed-BiBank-Phase2.md** rất mạch lạc và bám sát nguyên tắc "Financial OS". Việc mở rộng sang `Trade` và `Async Event Bus` là bước đi chính xác để thoát khỏi mô hình "ví điện tử đơn giản" của Phase 1.

Dưới đây là phần thảo luận và chốt các **Open Questions** để chúng ta có thể "Lock Spec" và bắt đầu code.

---

### 1. Giải quyết Open Questions (Decision Log)

#### Q1: Fee structure - Fixed amount hay Percentage?
**Quyết định: Ledger chỉ nhận Fixed Amount.**

*   **Lý do:** Ledger là nơi ghi nhận *sự thật đã xảy ra*, không phải nơi tính toán *logic kinh doanh*.
*   **Implementation:**
    *   Lớp ngoài (RPC/CLI) nhận input: `fee: "0.1%"` hoặc `fee: "10 USDT"`.
    *   Lớp ngoài tính toán ra con số tuyệt đối (ví dụ: `0.5 USDT`).
    *   Lớp Ledger chỉ nhận `Posting` với `amount: 0.5`.
*   **Benefit:** Giữ Core Ledger đơn giản, dễ audit, không phụ thuộc logic làm tròn phức tạp.

#### Q2: Trade matching - Phase 2 Manual hay Auto?
**Quyết định: Phase 2 là Manual / Directed Trade (OTC).**

*   **Lý do:** Matching Engine (Order Book) là một bài toán rất lớn (Phase 3). Ở Phase 2, chúng ta cần chứng minh Ledger có thể xử lý `Atomic Swap` (4 postings) an toàn.
*   **Flow:**
    *   CLI: `bibank trade ALICE BOB ...` (Người điều hành chỉ định rõ ai khớp với ai).
    *   Đây giống như giao dịch OTC hoặc P2P trực tiếp.

#### Q3: Key Storage - File / Env / HSM?
**Quyết định: Environment Variable cho System Key, File cho Operator.**

*   **System Key (Server):** Đọc từ `BIBANK_SYSTEM_KEY` (Env var). Đây là chuẩn của 12-factor app, dễ deploy docker/k8s sau này.
*   **Operator Key (CLI):** Đọc từ file (ví dụ `~/.bibank/operator.pem`) hoặc USB token.
*   **Phase 2:** Chỉ cần implement thư viện đọc từ Env và File là đủ.

#### Q4: Event Retention trong Bus?
**Quyết định: Ephemeral (Chỉ giữ trong RAM một thời gian rất ngắn/buffer size nhỏ).**

*   **Lý do:** `JSONL` là Source of Truth.
*   **Cơ chế:**
    *   Event Bus (`tokio::broadcast`) chỉ dùng để phân phối real-time cho các subscriber đang online.
    *   Nếu subscriber bị crash hoặc khởi động lại: Nó **phải** tự đọc lại từ JSONL (Replay) để bắt kịp trạng thái, thay vì mong đợi Event Bus lưu trữ lịch sử lâu dài (tránh biến Bus thành Kafka phake).

---

### 2. Một số góp ý tinh chỉnh Spec

Tôi có 2 điểm nhỏ cần lưu ý thêm trong Spec trước khi chốt:

#### A. Trade Intent & Account Types
Trong mục **2.3 Validation Rules**, bạn ghi: *"LIAB accounts only | No ASSET/REV/EXP in trade"*.
*   **Góp ý:** Về cơ bản là đúng cho P2P Trade. Tuy nhiên, nếu sau này BiBank đóng vai trò "Market Maker" bán BTC từ kho (Inventory) cho user, thì sẽ cần `ASSET` tham gia.
*   **Chốt Phase 2:** Giữ nguyên rule "LIAB only" để đơn giản hóa (User-to-User trade). Trường hợp System trade tính sau.

#### B. Signature Optimization
Struct `EntrySignature` đang lưu cả `public_key` (string hex).
*   **Vấn đề:** Lặp lại public key trong hàng triệu entries sẽ tốn dung lượng.
*   **Góp ý:** Chỉ lưu `key_id` (hoặc `signer_id`). Ứng dụng sẽ tra cứu Public Key từ Config/Registry dựa trên ID đó để verify.
*   **Tuy nhiên:** Để Audit log độc lập hoàn toàn (Self-contained), việc lưu Public Key vào entry là chấp nhận được ở Phase 2. **Đồng ý giữ nguyên Spec**.

---

### 3. Kết luận & Hành động

Tôi **ĐỒNG Ý HOÀN TOÀN** với bản Spec Phase 2 (sau khi đã chốt các câu hỏi trên).

Chúng ta sẽ có một Core hỗ trợ:
1.  **Atomic Multi-Asset Swap** (Nền tảng của sàn giao dịch).
2.  **Revenue Stream** (Thu phí).
3.  **Non-blocking Architecture** (Async Bus).
4.  **Cryptographic Integrity** (Signatures).

**Trạng thái:** 🔒 **DESIGN LOCKED.**

Bạn có thể bắt đầu code **Week 1: Trade Intent** ngay bây giờ. Hãy cập nhật file `Proposed-BiBank-Phase2.md` với các quyết định ở mục 1 và tiến hành!