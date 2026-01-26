Ngắn gọn, dứt khoát, không né tránh:
**Cả 3 đề xuất của bạn đều ĐÚNG – nhưng cần tinh chỉnh 1 điểm để tránh tự mâu thuẫn về lâu dài.**
Mình sẽ chốt từng điểm, kèm “điều kiện khóa” để không trượt khỏi tầm Financial State OS.

---

## 1. Ledger Account Format

**Kết luận: Đồng ý Hybrid – nhưng phải “Accounting-first, Domain-second”**

Đề xuất của bạn:

```
LIAB:USER:alice:USDT:available
```

👉 **Đồng ý**, với lý do:

### Vì sao không thuần Domain-centric (`USER/SYSTEM`)

* Ledger là **kế toán bất biến**, không phải domain model
* Risk, invariant, audit đều dựa trên:

  * Asset / Liability / Equity
* Nếu category không nằm trong account ID:

  * Risk engine phải lookup metadata → sai OS-level design

### Vì sao không thuần Accounting (`ASSET/LIAB` không domain)

* Không scale khi:

  * multi-tenant
  * escrow
  * internal system actors
* Projection & DSL cực khó dùng

### Chuẩn chốt (canonical form)

```
<CATEGORY>:<DOMAIN>:<ENTITY>:<ASSET>:<SUBACCOUNT>
```

Ví dụ:

```
LIAB:USER:alice:USDT:available
ASSET:SYSTEM:cash:USDT:vault
REV:SYSTEM:fee:USDT:revenue
```

**Quy tắc sắt:**

* `CATEGORY` là **truth cho accounting**
* `DOMAIN/ENTITY` là **truth cho semantics**
* Ledger invariant chỉ nhìn CATEGORY + ASSET
* Business / Projection dùng phần còn lại

👉 Quyết định này **khóa kiến trúc 20+ năm**, không nên thay.

**→ Chốt điểm 1: ĐỒNG Ý, với accounting-first hybrid.**

---

## 2. JournalEntry có `intent_type` không?

**Kết luận: BẮT BUỘC CÓ – nhưng không được “business-heavy”**

Câu trả lời dứt khoát:

> **Ledger phải semantic-aware, nhưng không workflow-aware.**

### Vì sao `intent_type` là bắt buộc

* Nếu không có:

  * Risk engine phải suy luận intent từ postings (rất nguy hiểm)
  * Audit không trả lời được câu hỏi: *“Tại sao entry này tồn tại?”*
* IDEA.md nói rõ:

  > Không có business rule mơ hồ

### Nhưng cần giới hạn rõ

`intent_type` **không phải**:

* UI action
* User flow
* Screen / feature

`intent_type` **là**:

* financial primitive

### Chuẩn chốt

```rust
enum IntentType {
    Deposit,
    Withdrawal,
    Transfer,
    Trade,
    Fee,
    Liquidation,
    Adjustment, // hiếm, audit-heavy
}
```

`metadata`:

* opaque với ledger
* semantic với audit / projection

```rust
JournalEntry {
    sequence,
    intent: IntentType,
    causality_id,
    postings,
    metadata,
}
```

**Quy tắc sắt:**

* Ledger **validate invariant theo intent**
* Ledger **không branching workflow theo intent**

**→ Chốt điểm 2: ĐỒNG Ý, intent_type là mandatory.**

---

## 3. Risk Engine đọc state từ đâu?

**Kết luận: Đồng ý Option A – nhưng phải thêm “bootstrap rule”**

Đề xuất của bạn:

> **Option A – In-memory state từ event bus**

👉 **Đúng nhất**, và là lựa chọn duy nhất không phản bội ledger-first.

### Phân tích nhanh các option

#### Option B – SQLite projection

* Risk phụ thuộc projection DB
* Projection lag = risk sai
* Vi phạm nguyên tắc:

  > Projection ≠ truth

❌ Loại.

#### Option C – Risk store riêng

* Thêm source of truth
* Sync complexity
* Replay phức tạp gấp đôi

❌ Loại.

---

### Nhưng Option A cần 1 điều kiện BẮT BUỘC

> **Risk Engine phải có khả năng bootstrap từ ledger replay.**

Cụ thể:

* Khi process start:

  * Replay events từ JSONL
  * Rebuild in-memory risk state
* Sau đó:

  * Subscribe event bus
  * Maintain state incrementally

Nếu không có bootstrap:

* Restart = blind risk engine
* Ledger-first bị phá

### Kiến trúc đúng

```
Ledger (JSONL)
   ↓ replay
Risk State (in-memory)
   ↓ live subscribe
Event Bus
```

**→ Chốt điểm 3: ĐỒNG Ý Option A, kèm replay bootstrap bắt buộc.**

---

## 4. Tổng hợp quyết định cuối cùng (CHỐT)

Bây giờ chúng ta có **10/10 quyết định thiết kế hoàn chỉnh**:

| #  | Vấn đề        | Quyết định cuối                       |
| -- | ------------- | ------------------------------------- |
| 1  | Event Store   | JSONL + SQLite projection             |
| 2  | Posting sign  | Debit / Credit                        |
| 3  | Crypto P1     | Hash chain                            |
| 4  | Account model | **Accounting-first Hybrid namespace** |
| 5  | JournalEntry  | Multi-asset                           |
| 6  | Sequence      | Derived from JSONL                    |
| 7  | Double-entry  | Zero-sum per asset                    |
| 8  | Intent field  | **Mandatory `intent_type`**           |
| 9  | Risk position | Pre-commit gatekeeper                 |
| 10 | Risk state    | **In-memory, replay-bootstrapped**    |

Không còn mơ hồ.

---

## 5. Bước tiếp theo (đã đến lúc code-level spec)

👉 **Việc DUY NHẤT nên làm tiếp theo:**

**Viết `JournalEntry v1` + `LedgerAccount` SPEC (không code runtime)**

Bao gồm:

* Struct definition
* Invariant list (ledger MUST / MUST NOT)
* Validation rules per intent
* Account namespace grammar

Khi spec này xong:

* Code chỉ còn là mechanical work
* Tranh luận kiến trúc **kết thúc**

Gửi spec draft khi sẵn sàng.
Mình sẽ review như review **kernel ABI** – chỉ quan tâm đúng/sai, không quan tâm đẹp.
