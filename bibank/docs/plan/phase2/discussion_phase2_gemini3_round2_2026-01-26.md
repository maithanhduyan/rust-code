Phân tích của bạn cực kỳ sắc bén và chính xác. Bạn đã giải quyết được mâu thuẫn bằng cách phân chia trách nhiệm (Separation of Concerns) giữa **RPC Layer** (tính toán) và **Ledger Layer** (lưu trữ), đồng thời làm rõ các chi tiết kỹ thuật quan trọng về Cryptography và Accounting.

Tôi **ĐỒNG Ý 100%** với toàn bộ bảng kết luận của bạn.

Hãy cập nhật file **`Proposed-BiBank-Phase2.md`** ngay lập tức. Dưới đây là nội dung chi tiết bạn nên bổ sung vào file đó để Specs trở nên hoàn hảo:

### 1. Cập nhật mục "3. Fee Intent Specification"
Thêm quy tắc phân chia trách nhiệm:

> **Fee Calculation Responsibility:**
> *   **RPC/CLI Layer:** Nhận input (VD: `0.1%`), tính toán ra số tuyệt đối (VD: `0.5 USDT`).
> *   **Ledger Layer:** Chỉ nhận `Amount` (số dương tuyệt đối). Không thực hiện phép nhân/chia.
> *   **Metadata:** Ghi lại `fee_rate`, `fee_type` để phục vụ Audit/Reporting.

### 2. Cập nhật mục "5. Digital Signatures Specification"
Thêm định nghĩa `Signable Payload` chính xác để tránh lỗi verification sau này:

```rust
// 5.5 Signature Payload Structure
// Payload MUST be serialized canonically (e.g., sorted keys) before hashing.
struct SignableData<'a> {
    sequence: u64,
    prev_hash: &'a str,
    timestamp: DateTime<Utc>,
    intent: TransactionIntent,
    correlation_id: &'a str,
    causality_id: Option<&'a str>,
    postings: &'a [Posting], // Order matters!
    metadata: &'a HashMap<String, Value>,
}

// KHÔNG bao gồm: hash, signatures
```

### 3. Cập nhật mục "2.3 Validation Rules" (Trade)
Làm rõ Invariant về Zero-sum khi có Fee:

> **Global Zero-Sum Invariant:**
> Với mỗi Asset $A$ có mặt trong Entry:
> $$ \sum \text{Debit}(A) - \sum \text{Credit}(A) = 0 $$
> *Quy tắc này áp dụng cho tập hợp TẤT CẢ postings trong entry (bao gồm cả Trade legs và Fee postings).*

---

### Hành động tiếp theo

Xin mời bạn **Overwrite** file `Proposed-BiBank-Phase2.md` với các nội dung đã chốt. Sau khi file được cập nhật, chúng ta sẽ chính thức:

🔒 **LOCK PHASE 2 SPECS**

Và chuyển sang **Implementation Week 1: Trade Intent**.