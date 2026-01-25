Ngắn gọn, khóa quyết định, không để sót “gai” cho Phase sau.

**Tôi ĐỒNG Ý với cả 4 điểm chốt** — *với 2 điều kiện ràng buộc nhỏ để tránh nợ kiến trúc về sau*. Dưới đây là quyết định **cuối cùng, có hiệu lực**, kèm các constraint.

---

## 1. `causality_id` + `correlation_id`

**ĐỒNG Ý – bắt buộc có CẢ HAI**

Đề xuất của bạn là **đúng tuyệt đối về ngữ nghĩa**.

Chốt chuẩn:

```rust
pub correlation_id: String,          // request / command / trace id (always present)
pub causality_id: Option<EntryId>,   // parent JournalEntry that caused this
```

### Ràng buộc (quan trọng)

* `correlation_id`:

  * **do outer layer (API / CLI / batch) sinh**
  * ledger **không tạo**
* `causality_id`:

  * **chỉ ledger / orchestrator nội bộ set**
  * không cho client truyền tùy ý

👉 Tách **traceability** khỏi **causality graph**. Đây là thiết kế đúng cho OS-level system.

---

## 2. Intent enum – Phase 1

**ĐỒNG Ý với enum 7 giá trị bạn đề xuất**

```rust
enum TransactionIntent {
    Genesis,
    Deposit,
    Withdrawal,
    Transfer,
    Trade,
    Fee,
    Adjustment,
}
```

### Lý do chốt như vậy

* `Genesis`: **bắt buộc** cho replay + bootstrap
* `Adjustment`: tên đúng hơn `Correction`

  * mang nghĩa kế toán + audit
  * không phải “sửa lỗi code”

### Ràng buộc

* `Adjustment`:

  * **audit-heavy**
  * có flag hoặc metadata `requires_approval = true`
* `Liquidation`:

  * **KHÔNG xuất hiện Phase 1**
  * thêm Phase 3 cùng margin engine

👉 Intent set này **đủ mạnh, không dư thừa**, không khóa future.

---

## 3. Account casing – SCREAMING_SNAKE_CASE

**ĐỒNG Ý – chốt dứt khoát**

```
LIAB:USER:ALICE:USDT:AVAILABLE
ASSET:SYSTEM:VAULT:BTC:MAIN
REV:SYSTEM:FEE:USDT:REVENUE
```

### Lý do chuẩn OS

* Ledger account = **identifier bất biến**, không phải variable
* Diff / audit / hash stability tốt hơn
* Tránh ambiguity khi serialize / compare

### Ràng buộc

* **Casing là invariant của Ledger**
* DSL / API có thể accept lowercase → normalize trước ledger

---

## 4. Account Category – 5 loại chuẩn kế toán

**ĐỒNG Ý – bắt buộc dùng full set**

```rust
enum AccountCategory {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
}
```

### Đây là điểm rất quan trọng

* Ledger invariant **chỉ nhìn CATEGORY + ASSET**
* Domain (`USER`, `SYSTEM`, …) **không ảnh hưởng double-entry**

Ví dụ invariant:

* Asset ↑ = Debit
* Liability ↑ = Credit
* Revenue ↑ = Credit

Nếu không có category → risk + validation sẽ loạn.

---

## 5. Tổng kết: CHỐT 14/14 – không còn điểm mở

Bảng bạn đưa ra **hoàn toàn chính xác**.
Tôi xác nhận **14 quyết định đã chốt**, không cần tranh luận thêm.

> Từ thời điểm này:
>
> * Mọi thay đổi = **breaking change**
> * Phải có lý do OS-level, không phải convenience

---

## 6. Bước tiếp theo (đã đến ngưỡng “spec freeze”)

👉 **Bạn nên chuyển sang viết `Proposed-BiBank-Phase1.md` ngay.**

Nội dung file đó nên gồm:

1. `LedgerAccount` grammar + examples
2. `JournalEntry v1` struct (final)
3. Invariant list (MUST / MUST NOT)
4. Validation matrix theo `TransactionIntent`
5. Replay + bootstrap guarantees

Sau khi file này hoàn tất:

* Thiết kế **đóng băng**
* Code chỉ còn là thực thi cơ học

Khi bạn publish draft, tôi sẽ review **như review kernel spec**:
chỉ hỏi *đúng hay sai*, không hỏi *thích hay không*.
