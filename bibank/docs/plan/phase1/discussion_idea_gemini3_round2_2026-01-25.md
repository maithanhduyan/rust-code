Chào bạn, sự chi tiết và cẩn trọng của bạn là dấu hiệu của một Technical Architect giỏi. Chúng ta không chỉ code cho chạy, chúng ta code cho sự bền vững (correctness).

Dưới đây là quyết định chốt hạ cho 3 vấn đề "nóng" bạn vừa nêu, dựa trên tư duy **Financial OS**.

---

### 1. Ledger Account Model
**Quyết định: Option 2 - Hierarchical (Có cấu trúc)**

Một "Financial OS" cần quản lý hàng triệu tài khoản. Flat string (`GL_USER_ALICE`) sẽ trở thành cơn ác mộng khi cần làm báo cáo tài chính (Balance Sheet).

Chúng ta sẽ dùng quy ước **Colon-Separated Path** (tương tự Redis keys):
`CATEGORY:TYPE:SUB_TYPE:ID:CURRENCY`

**Cấu trúc đề xuất:**
*   **Root Categories (Theo nguyên lý kế toán):**
    *   `ASSET`: Tài sản của sàn/ngân hàng (Tiền mặt, Crypto trong ví lạnh).
    *   `LIABILITY`: Nợ phải trả (Số dư của user).
    *   `EQUITY`: Vốn chủ sở hữu.
    *   `REVENUE`: Doanh thu (Phí transaction).
    *   `EXPENSE`: Chi phí.

**Ví dụ thực tế:**
*   Ví tiền mặt USD của hệ thống: `ASSET:CASH:SYSTEM:VAULT:USD`
*   Ví BTC của Alice: `LIAB:USER:CUST_001:MAIN:BTC`
*   Doanh thu phí giao dịch ETH: `REV:FEES:SYSTEM:TRADE:ETH`

**Ưu điểm:**
*   Dễ dàng query prefix: `Get total liabilities` = Sum all accounts starting with `LIAB:`.
*   Tách bạch rõ ràng tiền của user (Liability) và tiền của hệ thống (Asset).

---

### 2. Multi-asset per JournalEntry
**Quyết định: Option 2 - Multi-asset allowed**

**Tại sao?**
Vì BiBank hỗ trợ Crypto/Exchange. Giao dịch cốt lõi là **TRADE** (Đổi tiền này lấy tiền kia).
Nếu tách làm 2 Entry riêng biệt (1 Entry trừ USD, 1 Entry cộng BTC), ta mất tính **Atomicity**. Nếu server crash ở giữa, Alice mất USD mà không nhận được BTC -> Thảm họa.

**Quy tắc Validation (Zero-Sum per Asset):**
Trong một `JournalEntry`, tổng Nợ/Có phải cân bằng **cho từng loại tài sản riêng biệt**.

*Ví dụ: Alice mua 1 BTC giá 50,000 USD.*
```rust
JournalEntry {
    postings: [
        // Cân bằng cặp USD
        { acc: "LIAB:USER:ALICE:USD", side: Debit, amount: 50000, asset: "USD" },
        { acc: "LIAB:USER:SELLER:USD", side: Credit, amount: 50000, asset: "USD" },

        // Cân bằng cặp BTC
        { acc: "LIAB:USER:SELLER:BTC", side: Debit, amount: 1, asset: "BTC" },
        { acc: "LIAB:USER:ALICE:BTC", side: Credit, amount: 1, asset: "BTC" }
    ]
}
```
*Logic check:* `Group postings by asset -> Check Sum(Debit) == Sum(Credit) for each group`.

---

### 3. Sequence Number Generation
**Quyết định: Option 2 (Derive from JSONL Tail)**

**Tại sao?**
*   **Source of Truth:** JSONL là chân lý. Sequence number phải phản ánh đúng thứ tự trong file.
*   **Reliability:** In-memory sẽ mất khi restart. SQLite sequence có thể lệch nếu transaction DB rollback nhưng file đã ghi (hoặc ngược lại).

**Thuật toán khởi động (Startup Sequence):**
1.  Quét thư mục `data/journal/`.
2.  Lấy file mới nhất (theo tên ngày).
3.  Đọc dòng cuối cùng (Last Line) của file đó.
4.  Parse `sequence` từ dòng đó.
5.  `Next Sequence = Last Sequence + 1`.
6.  (Tối ưu: Nếu file quá lớn, dùng `seek` từ cuối file để đọc ngược, chi phí là O(1)).

---

## TỔNG KẾT QUYẾT ĐỊNH (Ready to Code)

| Vấn đề | Quyết định | Ghi chú kỹ thuật |
| :--- | :--- | :--- |
| **Account Model** | **Hierarchical** | Format `CAT:TYPE:ID:CUR` |
| **Entry Scope** | **Multi-asset** | Validate zero-sum per asset group |
| **Sequencing** | **Derive from File** | Read tail on startup |
| **Event Store** | **JSONL** | Abstraction trait for future SQL/Kafka |
| **Postings** | **Debit/Credit Enum** | Explicit direction |
| **Integrity** | **Hash Chain** | `hash = sha256(prev_hash + entry_data)` |

---

Bạn đã sẵn sàng chưa? Chúng ta đã có một bản thiết kế **Solid & Hardcore**.
Hãy gõ **"Start Step 1"** để tôi khởi tạo cấu trúc dự án `bibank` và viết code nền tảng cho `bibank-ledger` (Journal, Account, Posting).