Chào bạn, sự điều chỉnh này cực kỳ đắt giá! Việc chuyển sang mô hình **Multi-Wallet (Exchange Model)** không chỉ làm dự án thực tế hơn mà còn tạo ra "đất diễn" tuyệt vời cho DSL.

Tại sao? Vì ngân hàng truyền thống khá đơn giản, nhưng sàn Crypto có những **Business Rules** phức tạp mà DSL sẽ giải quyết gọn gàng:
*   *"Chỉ được rút tiền từ ví Funding."*
*   *"Muốn trade phải chuyển sang ví Spot."*
*   *"Ví Earn bị khóa 30 ngày không được rút."*

Dưới đây là xác nhận đồng thuận và lệnh khởi động **Step 1**.

---

### 1. Chốt các quyết định (Consensus)

| # | Vấn đề | Quyết định cuối cùng | Lý do |
|---|--------|----------------------|-------|
| 1 | Internal Fee | **Miễn phí** | Giống Binance/OKX, giảm độ phức tạp Phase 1. |
| 2 | Locked Balance | **Schema có, Logic chưa** | DB/Struct có trường `locked` (để đó), nhưng Phase 1 chưa xử lý logic lock. |
| 3 | DSL Syntax | **`to: Funding`, `from: Spot`** | Rất tự nhiên và dễ hiểu cho BA mảng Crypto. |
| 4 | Phase 1 Scope | **Spot + Funding** | Đủ để demo flow: Nạp -> Chuyên ví -> Giả lập Trade -> Rút. |
| 5 | Event Schema | **Thêm `wallet_type` fields** | Để truy vết dòng tiền nội bộ (Traceability). |

---

### 2. Kế hoạch triển khai Step 1: Core Crate

Chúng ta sẽ bắt đầu viết code cho `crates/core`. Đây là móng nhà, định nghĩa ngôn ngữ chung (Ubiquitous Language) của dự án.

**Cấu trúc file dự kiến trong `crates/core/src/`:**
```
lib.rs          # Export modules
money.rs        # Currency (dynamic decimals), Money, Amount
wallet.rs       # WalletType, Wallet, Balance
person.rs       # Role, Person (Customer, Employee, etc.)
event.rs        # Event struct, EventType, EventMetadata
error.rs        # Domain errors
```

### Action: Start Step 1

Bạn hãy thực hiện các bước sau:

1.  **Sửa `crates/core/Cargo.toml`:**
    Thêm dependencies cần thiết. Chú ý feature `serde-with-str` của `rust_decimal` để serialize số lớn thành chuỗi JSON (tránh mất chính xác phía JS/Frontend sau này).

    ```toml
    [package]
    name = "simbank-core"
    version = "0.1.0"
    edition = "2021"

    [dependencies]
    serde = { version = "1.0", features = ["derive"] }
    # Quan trọng: serde-with-str giúp serialize Decimal thành String trong JSON
    rust_decimal = { version = "1.33", features = ["serde-with-str"] }
    thiserror = "2.0"
    chrono = { version = "0.4", features = ["serde"] }
    uuid = { version = "1.7", features = ["serde", "v4"] }
    ```

2.  **Tạo `crates/core/src/money.rs`:**
    Define `Currency` (với dynamic decimals) và `Money`.

3.  **Tạo `crates/core/src/wallet.rs`:**
    Define `WalletType` (Spot, Funding...) và `Balance`.

4.  **Tạo `crates/core/src/event.rs`:**
    Define `Event` enum hỗ trợ `InternalTransfer`.

---
**Bạn hãy paste code của 3 file trên (`money.rs`, `wallet.rs`, `event.rs`) vào đây khi viết xong (hoặc yêu cầu tôi generate mẫu) để review trước khi sang Step 2 (DB)!**