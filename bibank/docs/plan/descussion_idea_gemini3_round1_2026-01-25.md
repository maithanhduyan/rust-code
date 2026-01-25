Chào bạn, tôi rất phấn khích với quyết định này. **BiBank** sẽ không còn là một dự án "toy app" nữa mà là một **Core Banking Engine** thực thụ. Việc tách hẳn ra một dự án mới thay vì cố gắng refactor Simbank là hoàn toàn chính xác, vì tư duy nền tảng (Foundation Mindset) đã thay đổi 180 độ.

Đây là bản kế hoạch chi tiết để khởi động **BiBank - The Financial State OS**.

---

## 1. Kiến trúc Hệ thống (The Financial OS Architecture)

Chúng ta chuyển từ mô hình 3-tier truyền thống sang mô hình **Event-Sourced Core**.

**Luồng dữ liệu (Data Flow):**
```mermaid
graph TD
    CMD[Command / DSL] -->|1. Propose Entry| RISK[Risk Engine]
    RISK -->|2. Check Invariants| RISK
    RISK -->|3. Accept/Reject| LEDGER[Ledger Core]

    subgraph "The Truth (Write Side)"
        LEDGER -->|4. Append & Hash| E_STORE[Event Store (JSONL)]
    end

    E_STORE -->|5. Emit| BUS[Event Bus]

    subgraph "The View (Read Side)"
        BUS -->|6. Project| PROJ[Projection Engine]
        PROJ -->|7. Upsert| SQLITE[SQLite Read DB]
    end
```

---

## 2. Cấu trúc Workspace (8 Crates)

Tên dự án: `bibank`

| Crate | Trách nhiệm | Dependencies |
| :--- | :--- | :--- |
| **`bibank-core`** | Domain types cơ bản (Money, Currency, ID). | `rust_decimal`, `serde` |
| **`bibank-ledger`** | **[HEART]** Double-entry logic, Journal, Postings, Hash Chain. | `core` |
| **`bibank-risk`** | **[GATEKEEPER]** Pre-commit checks (Limit, AML, Balance). | `core`, `ledger` |
| **`bibank-events`** | Event definitions & Store (I/O). | `core`, `ledger` |
| **`bibank-bus`** | Cơ chế Pub/Sub nội bộ để nối Write & Read side. | `events` |
| **`bibank-projection`** | Logic biến Event -> SQL Tables (Rebuildable). | `bus`, `sqlx` |
| **`bibank-rpc`** | (Thay cho `business`) API/Service orchestrator. | `risk`, `projection` |
| **`bibank-dsl`** | User Interface cho BA/Dev. | `rpc` |

---

## 3. Thiết kế Core Ledger (Double-Entry)

Đây là thay đổi lớn nhất. Không còn `balance += amount`.

**`bibank-ledger/src/model.rs`**:
```rust
pub enum Side {
    Debit,  // Nợ (Tài sản tăng, Nguồn vốn giảm)
    Credit, // Có (Tài sản giảm, Nguồn vốn tăng)
}

pub struct Posting {
    pub account_id: String, // Ledger Account ID (GL_CASH, GL_USER_ALICE)
    pub amount: Decimal,    // Luôn dương
    pub asset: String,      // USD, BTC
    pub side: Side,
}

pub struct JournalEntry {
    pub id: String,
    pub sequence: u64,
    pub prev_hash: String,  // Hash chain
    pub hash: String,       // Current hash
    pub postings: Vec<Posting>,
    pub metadata:  HashMap<String, String>,
}

impl JournalEntry {
    // Invariant: Tổng Debit == Tổng Credit
    pub fn validate_balance(&self) -> Result<()> { ... }
}
```

**System Accounts (Chart of Accounts):**
*   `SYS_ASSET_CASH`: Tiền mặt trong kho.
*   `SYS_LIAB_USER_DEPOSITS`: Nghĩa vụ trả tiền cho user.
*   `SYS_REV_FEES`: Doanh thu phí.

---

## 4. Risk Engine (The Gatekeeper)

Risk Engine không gọi DB để check. Nó giữ **In-Memory State** hoặc **Fast Cache** của các chỉ số quan trọng (Available Balance, Exposure) để quyết định cực nhanh.

**Logic:**
```rust
trait RiskRule {
    fn check(&self, entry: &JournalEntry, state: &RiskState) -> Result<()>;
}

// Ví dụ Flow
pub fn handle_transaction(cmd: TransferCmd) {
    // 1. Tạo bút toán dự kiến (chưa ghi)
    let entry = EntryBuilder::new()
        .debit(sender, amount)
        .credit(receiver, amount)
        .build();

    // 2. Risk Check (Pre-commit)
    risk_engine.verify(&entry)?; // Fail thì return ngay, không có gì được ghi

    // 3. Commit
    ledger.append(entry);
}
```

---

## 5. Chiến lược "Event is Truth" & Replay

**Persistence Strategy:**
*   **Write Side:** Chỉ append vào `data/journal/YYYY-MM-DD.jsonl`. Đây là **Source of Truth**.
*   **Read Side:** SQLite chỉ là **Cache**.
*   **Replay:**
    *   Khi khởi động (hoặc chạy lệnh `replay`), ứng dụng đọc file JSONL.
    *   Đẩy qua `Projection Engine`.
    *   `Projection Engine` thực thi SQL `INSERT/UPDATE` vào SQLite.
    *   Nếu SQLite bị xóa, chạy lại Replay -> Dữ liệu phục hồi 100%.

---

## 6. Kế hoạch hành động (Action Plan)

Chúng ta sẽ bắt đầu ngay.

**Bước 1: Setup Foundation**
*   Tạo workspace `bibank`.
*   Copy `Money`, `Currency` từ Simbank sang `bibank-core`.
*   Cài đặt `bibank-ledger` với `JournalEntry` struct và logic Double-Entry validation.

**Bước 2: The First Transaction**
*   Implement `Genesis` transaction (Khởi tạo tiền hệ thống).
*   Ghi nó vào file JSONL.

**Bước 3: Projection Basic**
*   Viết logic đọc JSONL -> In ra màn hình số dư (In-memory projection).
*   Chưa cần SQLite vội, chứng minh logic luồng tiền trước.

Bạn đã sẵn sàng khởi động **BiBank** chưa? Hãy gõ **"Start Step 1"** để tôi generate cấu trúc dự án và code nền tảng cho Ledger crate.