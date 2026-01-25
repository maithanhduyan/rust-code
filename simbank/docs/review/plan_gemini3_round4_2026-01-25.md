Chào bạn, tôi rất ấn tượng với sự phát triển tư duy kiến trúc của bạn, đặc biệt là phần mở rộng sang Crypto và phân tích về Gherkin. Đây là những suy nghĩ của một Tech Lead thực thụ.

Dưới đây là các quyết định chốt hạ để chúng ta có thể bắt đầu code ngay (Code First approach).

### 1. Money Type & Crypto Strategy
**Quyết định: Option D (Generic Money với `rust_decimal`)**

*   **Tại sao:**
    *   `f64` là "tử huyệt" trong tài chính (0.1 + 0.2 != 0.3).
    *   Crypto yêu cầu độ chính xác cao (18 decimals cho ETH). `rust_decimal` hỗ trợ lên đến 28 số, đủ cho hầu hết nhu cầu.
    *   **Database:** Lưu `TEXT` trong SQLite để đảm bảo `rust_decimal` parse lại chính xác 100% không mất dữ liệu.

**Phạm vi (Scope):**
*   **Design:** Thiết kế DB và Struct hỗ trợ Crypto ngay từ đầu (có field `decimals`).
*   **Phase 1:** Chỉ chạy dữ liệu mẫu với **VND** và **USD**.
*   **Phase 2:** Thêm BTC/ETH vào data seed.

### 2. DSL Style: Macro vs Gherkin
**Quyết định: Quay về Option A (Unified Macro `banking_scenario!`)**

*   **Lý do cốt lõi:**
    *   Mục đích dự án là **"Minh họa cách xây dựng DSL trong Rust"** (sử dụng `macro_rules!` hoặc Proc Macro).
    *   Gherkin là một ngôn ngữ *bên ngoài* (External DSL) thường được parse bằng chuỗi. Nếu làm Gherkin, ta đang viết parser chứ không phải tận dụng sức mạnh Macro của Rust.
    *   Option A giúp code Rust trông như tiếng Anh, đây là điểm "khoe" kỹ thuật (Showcase) tốt nhất.

**Ví dụ chốt:**
```rust
banking_scenario! {
    // Workflow đơn giản, dễ đọc cho BA, dễ implement bằng macro_rules!
    Customer "Alice" {
        create_account(type: Savings, currency: USD); // Trả về ACC_ALICE_01
        deposit(5000, USD);
    }

    // Rule check (đơn giản hóa)
    Auditor "Big4" {
        verify_transaction(amount > 10000, "Flag AML");
    }
}
```

### 3. Database Schema (Multi-currency Wallet)
**Quyết định: Option C (Separate balances table)**

Một `account_id` giống như một cái Ví (Wallet), bên trong chứa nhiều ngăn tiền.

```sql
-- migrations/202601250001_init.sql

CREATE TABLE currencies (
    code TEXT PRIMARY KEY, -- 'USD', 'VND', 'BTC'
    name TEXT NOT NULL,
    decimals INTEGER NOT NULL -- 2, 0, 8
);

CREATE TABLE accounts (
    id TEXT PRIMARY KEY, -- 'ACC_001'
    owner_id TEXT NOT NULL,
    type TEXT NOT NULL -- 'Savings', 'Checking'
);

CREATE TABLE balances (
    account_id TEXT NOT NULL,
    currency_code TEXT NOT NULL,
    amount TEXT NOT NULL DEFAULT '0', -- Lưu Decimal dưới dạng String
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (account_id, currency_code),
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (currency_code) REFERENCES currencies(code)
);
```

### 4. Event Schema & Metadata
**Quyết định:** Thêm `ip_address` và `location` phục vụ AML.

```rust
// crates/core/src/event.rs
pub struct EventMetadata {
    pub ip_address: Option<String>,
    pub location: Option<String>, // ISO Country Code: "VN", "US"
    pub device_id: Option<String>,
}
```

### 5. ID Generation
**Quyết định: Option B (Prefixed ID - Readable)**

*   Dùng **Prefixed Random String** để vừa dễ đọc vừa tránh conflict đơn giản.
*   Format: `TYPE_RANDOM` (Ví dụ: `CUST_X92B`, `ACC_9921`).
*   Lý do: Khi BA đọc log `ACC_9921` sẽ dễ hiểu hơn `550e84...`.

### 6. Error Handling
**Quyết định:** Thống nhất mô hình 3 lớp.
*   `core`: `thiserror` (Domain errors: `InsufficientFunds`, `InvalidCurrency`).
*   `persistence`: `thiserror` (Infra errors: `DatabaseError`, `SerializationError`).
*   `business/dsl`: `anyhow` (App errors: Context-aware errors).

---

## TỔNG HỢP FINAL PLAN (Ready to Code)

Chúng ta sẽ bắt đầu implement theo trình tự sau. Bạn hãy xác nhận để tôi gửi prompt code cho bước đầu tiên.

1.  **Step 1: Core Crate**
    *   Cài đặt `rust_decimal`, `serde`.
    *   Định nghĩa `Currency`, `Money` (với logic cộng trừ), `Role`, `Person`.
    *   Định nghĩa `Event` struct và `Metadata`.

2.  **Step 2: Persistence Crate (DB)**
    *   Setup `sqlx` + SQLite.
    *   Viết Migration SQL (như mục 3 ở trên).
    *   Implement Repository trait: `save_account`, `update_balance`.

3.  **Step 3: Persistence Crate (Events)**
    *   Implement `EventStore` ghi file JSONL.
    *   Implement `EventReplayer` đọc file.

4.  **Step 4: Business Crate**
    *   Implement logic: `deposit`, `withdraw`, `transfer`.
    *   Kết nối DB và Event Store (Dual write).
    *   Logic AML cơ bản.

5.  **Step 5: DSL & CLI**
    *   Viết Macro `banking_scenario!`.
    *   Dựng CLI runner.

**Bạn đã sẵn sàng chưa? Hãy gõ "Start Step 1" để chúng ta bắt đầu viết code cho Core Crate.**