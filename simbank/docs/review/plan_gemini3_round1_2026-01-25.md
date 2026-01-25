Dựa trên nội dung file `README.md` và phần **Plan** bạn đã phác thảo, đây là bản phân tích chi tiết và lộ trình triển khai dự án **SIMBANK**. Dự án này kết hợp sự thú vị của việc xây dựng DSL (Domain Specific Language) với các yêu cầu kỹ thuật thực tế (SQLite, Event Sourcing, AML Compliance).

Dưới đây là đề xuất chi tiết để hiện thực hóa kế hoạch này.

---

## 1. Kiến trúc hệ thống (Refined Architecture)

Chúng ta sẽ điều chỉnh cấu trúc thư mục để khớp giữa `README.md` và phần `Plan` kỹ thuật. Sử dụng Rust Workspace là lựa chọn chính xác.

**Cấu trúc Workspace (`simbank`):**

| Crate | Vai trò | Công nghệ chính |
| :--- | :--- | :--- |
| **`crates/core`** | Chứa các Type cơ bản (Domain Entities). Không phụ thuộc DB hay DSL. | `struct`, `enum`, `trait` |
| **`crates/db`** | Quản lý trạng thái hiện tại (Current State) vào SQLite. | `sqlx` (SQLite) |
| **`crates/events`** | Ghi log lịch sử (Event Store) dạng JSONL phục vụ Audit & AML. | `serde_json`, `std::fs` |
| **`crates/business`** | Logic nghiệp vụ thuần túy (Service Layer). Gọi xuống `db` và `events`. | Business Logic |
| **`crates/dsl-macros`** | Định nghĩa cú pháp tiếng Anh cho BA/User. Biên dịch thành code gọi `business`. | `macro_rules!`, `proc_macro` |
| **`crates/reports`** | Logic tạo báo cáo cho Cổ đông và Kiểm toán (Big 4). | Report generation |
| **`simbank-cli`** | (Optional) Giao diện dòng lệnh để chạy ứng dụng. | `clap` |

---

## 2. Chiến lược triển khai DSL (Tiếng Anh + Comment Tiếng Việt)

Đây là điểm nhấn của dự án. DSL cần đọc như tiếng Anh tự nhiên nhưng map trực tiếp vào code Rust.

**Ví dụ cú pháp DSL mong muốn:**

```rust
// Trong file examples/scenario_1.rs

use simbank_dsl::banking_scenario;

banking_scenario! {
    // 1. Nghiệp vụ Khách hàng
    // Tạo tài khoản mới cho Alice với số dư khởi tạo
    Customer "Alice" performs {
        create_account(type: Savings, currency: USD);
        deposit(amount: 5000, currency: USD);
    }

    // 2. Nghiệp vụ Nhân viên
    // Nhân viên Bob mua bảo hiểm
    Employee "Bob" performs {
        buy_insurance(plan: "Health_Premium", cost: 200, currency: USD);
    }

    // 3. Nghiệp vụ Kiểm toán (The Big 4)
    // Deloitte thực hiện kiểm tra các giao dịch đáng ngờ (AML)
    Auditor "Deloitte" performs {
        audit_report(focus: "Large_Transactions", threshold: 10000);
    }
}
```

**Cách hiện thực:**
Chúng ta sẽ dùng `macro_rules!` (declarative macros) cho đơn giản hoặc `proc_macros` nếu cần parse phức tạp. Với yêu cầu này, `macro_rules!` kết hợp với Builder Pattern ở tầng `business` là đủ mạnh và dễ debug.

---

## 3. Chi tiết kỹ thuật & Dữ liệu

### A. Core (`crates/core`)
Định nghĩa các thực thể (Entities) đa tiền tệ và vai trò:

```rust
pub enum Role {
    Customer,
    Employee,
    Manager,
    Shareholder,
    Auditor,
}

pub enum Currency {
    VND,
    USD,
}

pub struct Money {
    pub amount: f64, // Hoặc dùng decimal crate để chính xác
    pub currency: Currency,
}
```

### B. Persistence Strategy (SQLite + JSONL)

Đây là mô hình **CQRS lai (Hybrid CQRS)**:
1.  **SQLite (`crates/db`):** Lưu trạng thái *hiện tại* (Snapshot).
    *   Ví dụ: Alice đang có 5000 USD.
    *   Mục đích: Truy vấn nhanh số dư, check logic giao dịch (không được rút quá số dư).
    *   *Schema:* `users`, `accounts`, `transactions`.
2.  **JSONL (`crates/events`):** Lưu *chuỗi sự kiện* (Event Log/Audit Trail).
    *   Ví dụ: Alice mở tài khoản -> Alice nạp 5000 USD.
    *   Mục đích: Phục vụ Compliance, AML (Anti-Money Laundering), và Replay khi cần khôi phục dữ liệu.
    *   *Format:* Append-only file.

**Ví dụ mẫu JSONL cho AML:**
```json
{"timestamp": "2026-01-25T19:00:00Z", "event": "Deposit", "user": "Alice", "amount": 9900, "currency": "USD", "flags": ["near_threshold"]}
```

### C. Anti-Money Laundering (AML) Logic
Logic AML sẽ nằm trong `business` hoặc `reports`, quét qua file JSONL:
1.  **Large Amount:** Giao dịch > 10,000 USD.
2.  **Smurfing/Structuring:** Nhiều giao dịch nhỏ (ví dụ: 2,000 USD x 5 lần) trong thời gian ngắn để né mức báo cáo 10,000 USD.
3.  **Cross-border:** (Nếu có field location) Giao dịch đột ngột từ quốc gia lạ.

---

## 4. Trả lời các vấn đề thảo luận (Discussion Points)

Dựa trên phần "Further Considerations" của bạn:

**1. Event Sourcing Granularity?**
*   **Chọn Option C (Ghi tất cả):** Trong ngân hàng, *mọi* thay đổi trạng thái đều quan trọng. Dung lượng ổ cứng rẻ hơn rủi ro pháp lý.
    *   Ghi log tất cả các lệnh: `Create`, `Deposit`, `Withdraw`, `Transfer`.
    *   Khi query cho AML, ta sẽ filter: `events.filter(|e| e.amount > 10000)`.

**2. Database Migrations?**
*   **Chọn Option B (`sqlx migrate`):** Đây là chuẩn công nghiệp trong Rust.
    *   Tạo folder `migrations/`.
    *   File `20260125_init.sql`: `CREATE TABLE accounts ...`
    *   Giúp đồng bộ schema giữa các môi trường dev/test/prod dễ dàng.

**3. API Layer?**
*   **Giai đoạn 1:** Chỉ dùng **CLI + DSL**. Mục đích chính là minh họa DSL. Việc dựng REST API (Actix-web/Axum) sẽ làm loãng sự tập trung vào DSL và Logic nghiệp vụ.
*   **Giai đoạn 2 (Mở rộng):** Có thể wrap DSL vào một API endpoint nếu cần.

---

## 5. Kế hoạch hành động (Step-by-Step Plan)

**Tuần 1: Nền tảng (Core & DB)**
1.  Khởi tạo Rust Workspace.
2.  Viết `crates/core`: Defines types, errors.
3.  Viết `crates/db`: Setup SQLite với `sqlx`, viết migration script tạo bảng.
4.  Viết `crates/events`: Module ghi/đọc file JSONL.

**Tuần 2: Nghiệp vụ & DSL (The Heart)**
1.  Viết `crates/business`:
    *   Implement logic: `deposit`, `withdraw` (cập nhật SQLite + ghi Event).
    *   Logic tính phí thường niên, logic bảo hiểm.
2.  Viết `crates/dsl-macros`:
    *   Tạo macro `simbank!` hoặc `workflow!`.
    *   Parse cú pháp tiếng Anh đơn giản.

**Tuần 3: Compliance & Reporting (The Big 4)**
1.  Implement logic đọc JSONL trong `crates/reports`.
2.  Viết các rule AML (phát hiện giao dịch lớn, bất thường).
3.  Tạo report cho cổ đông (tính toán cổ tức dựa trên lợi nhuận giả định).

**Tuần 4: Testing & Polish**
1.  Viết Integration Test trong `tests/`.
2.  Viết ví dụ mẫu trong `examples/`.
3.  Hoàn thiện README và tài liệu.

Bạn có đồng ý với kiến trúc và lộ trình này không? Nếu đồng ý, chúng ta có thể bắt đầu với **Bước 1: Setup Workspace và Core Crate**.