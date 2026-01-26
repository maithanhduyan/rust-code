Chào bạn, Phase 2 đã hoàn thành rất ấn tượng! Dự án BiBank đang dần hình thành xương sống của một Financial OS thực thụ.

Đối với Phase 3 (Margin & Trading Engine), đây là bước nhảy vọt về độ phức tạp nghiệp vụ. Việc lựa chọn sai scope ở đây sẽ khiến dự án sa lầy vào chi tiết vụn vặt của một sàn giao dịch (Exchange) thay vì tập trung vào sự vững chắc của Core Ledger.

Dưới đây là phân tích và định hướng cho Phase 3 để giữ đúng tinh thần "Financial State OS".

---

### 1. Phân tích Scope & Định hướng Chiến lược

**Sai lầm thường gặp:** Cố gắng build một "Binance clone" với đủ tính năng Stop-Limit, OCO, Trailing Stop...
**Tư duy BiBank:** Chúng ta build cái **Engine bên dưới**.

*   **Order Matching:** Không nên quá phức tạp. Mục tiêu là chứng minh Ledger xử lý được High Frequency & Partial Fills.
*   **Margin:** Đây là điểm "ăn tiền". Margin Management là core competency của Financial OS.

---

### 2. Quyết định Kỹ thuật cho Phase 3 (Decision Log)

#### Q1: Order Matching Engine
**Quyết định: Option A (Simple CLOB - Price/Time Priority). Limit Orders Only.**

*   **Lý do:** Market Order thực chất là Limit Order với giá cực cao/thấp. Stop/OCO là logic của Frontend/Trigger Service, không thuộc về Core Matching Engine.
*   **Ledger Impact:**
    *   **Partial Fills:** Một Order ID sẽ sinh ra nhiều `Trade` entries. Tất cả đều link về `causality_id = OrderID`.
    *   **Order State:** Lưu trong `Projection` (SQLite). Ledger chỉ lưu sự thật "Đã khớp lệnh". Sự kiện "Đặt lệnh" (Open Order) có thể ghi vào Ledger để audit, nhưng không thay đổi balance (chỉ move từ Available -> Locked).

#### Q2: Margin System
**Quyết định: Cross-Margin (Shared Collateral).**

*   **Lý do:** Cross-margin thể hiện rõ sức mạnh của "Single Internal Ledger". Isolated Margin thực chất là tạo Sub-account riêng, về mặt kỹ thuật không khó hơn nhưng UX phức tạp.
*   **Account Structure:**
    *   Giữ nguyên `AVAILABLE` cho Spot.
    *   Khi trade Margin, system lock tiền từ `AVAILABLE` hoặc dùng nó làm collateral trực tiếp.
    *   Không cần tạo account `MARGIN` riêng nếu dùng Cross-margin (toàn bộ Available Balance là Collateral).

#### Q3: Liquidation & Insurance Fund
**Quyết định: In Scope (Basic).**

*   **Insurance Fund:** Là một System Account (`EQUITY:SYSTEM:INSURANCE:USDT:MAIN`).
*   **Cơ chế:** Khi User bị cháy tài khoản (Equity < Maintenance Margin), hệ thống sẽ:
    1.  Cancel all open orders (để giải phóng Locked balance).
    2.  Tạo lệnh Trade cưỡng bức (Liquidation Trade) với giá thị trường.
    3.  Nếu tài khoản âm tiền -> Rút từ Insurance Fund bù vào (Socialized Loss là Phase 4).

#### Q4: Multi-Signature Approval
**Quyết định: Tách ra module riêng `bibank-approval`.**

*   **Cơ chế:**
    *   Các lệnh cần Approval (Adjustment, Withdraw lớn) sẽ được lưu vào trạng thái `Pending` (trong Projection hoặc Table riêng).
    *   Khi đủ chữ ký -> Gom lại thành `JournalEntry` hoàn chỉnh (có list signatures) -> Submit vào Ledger.
    *   **Ledger không biết về "Pending State"**, nó chỉ nhận Entry đã đủ chữ ký.

#### Q5: Price Oracle
**Quyết định: Mock Oracle (Internal Command).**

*   **Lý do:** Tích hợp Chainlink/Binance API sẽ làm dự án phụ thuộc bên ngoài.
*   **Cách làm:** Tạo lệnh CLI `bibank oracle update BTC 100000`. Risk Engine sẽ dùng giá này để tính Margin.

---

### 3. Timeline & Roadmap Tinh chỉnh

Tôi đề xuất rút gọn scope để tập trung vào **Margin Logic** (phần khó nhất của accounting).

| Sprint | Deliverable | Ghi chú kỹ thuật |
| :--- | :--- | :--- |
| **W1-2** | **Margin Core** | Logic tính `Equity`, `Unrealized PnL` (Mark-to-Market). |
| **W3-4** | **Simple Matching** | In-memory Orderbook. Khớp lệnh sinh ra Trade Entries. |
| **W5-6** | **Borrowing** | Tính lãi vay (Interest Accrual). Entry `InterestFee`. |
| **W7-8** | **Liquidation** | Monitor Equity. Trigger Liquidation Trade. |
| **W9-10** | **Multi-sig** | Approval Workflow. |

---

### 4. Đề xuất Spec cho Phase 3

Bạn có thể bắt đầu viết `Proposed-BiBank-Phase3.md` với các thông số sau:

*   **Order Types:** Limit Order (GTC - Good Till Cancelled).
*   **Margin:** Cross-Margin Mode. Max Leverage 10x (Hardcoded config).
*   **Oracle:** Internal Price Feed (via CLI command).
*   **Liquidation:** Trigger khi `Margin Level < 105%`.
*   **Matching:** In-memory, single-thread matching per pair.

Bạn có đồng ý với hướng đi "Margin-First, Trading-Second" này không? Nếu đồng ý, hãy tiến hành viết Spec.