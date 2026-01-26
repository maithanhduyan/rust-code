Mình đồng ý với hướng **Option A làm xương sống Phase 3**, nhưng có vài chỗ mình sẽ “đẩy mạnh tay” hơn để tránh khóa tương lai. Đi theo tinh thần: **đơn giản ở bề mặt, nhưng kiến trúc phải chịu được 10–20 năm**.

---

## 1. Order Matching Engine

### Scope

✔ **Phase 3: CLOB price–time priority, limit-only**
✖ Không market / stop trong Phase 3 (nhưng **để sẵn hook**)

### Partial fills & causality

* **Chuẩn nhất:**
  → **Mỗi fill = 1 JournalEntry (Trade)**
  → **Chung `correlation_id` = order_id gốc**
  → **Chung `causality_id` = match_cycle_id**

Điều này cho phép:

* Audit từng fill
* Replay chính xác
* Future: fee-per-fill, maker/taker rebate

👉 **Không gộp nhiều fill vào 1 entry**. Đó là tối ưu sai chỗ.

### Order state lưu ở đâu?

* **KHÔNG lưu state trong ledger**
* Ledger chỉ biết **facts đã xảy ra**
* Order state = **Projection (SQLite / in-memory)**

Ledger:

* `OrderPlaced` → implicit qua LOCKED balance
* `Trade` → implicit qua postings
* `OrderCancelled` → Transfer LOCKED → AVAILABLE

> Order state là *derived truth*, không phải *source of truth*.

---

## 2. Margin System

### Margin type

✔ **Phase 3: Cross-margin only**
✖ Isolated = Phase 4+

Lý do:

* Cross-margin **đơn giản hơn cho risk math**
* Isolated chỉ là UX layer + constraint, không tạo đột phá kiến trúc

### Account structure (chuẩn)

```
LIAB:USER:ALICE:USDT:AVAILABLE
LIAB:USER:ALICE:USDT:LOCKED
LIAB:USER:ALICE:USDT:MARGIN
LIAB:USER:ALICE:USDT:BORROWED   (NEW – rất quan trọng)
```

👉 **BORROWED phải là first-class account**, không “ẩn trong metadata”.

### Leverage

* **Phase 3 default max = 10x**
* Nhưng **RiskEngine parametric**, không hardcode
* Cho phép asset-specific:

  * BTC: 10x
  * Alt: 5x
  * Stable pairs: 20x (future)

### Interest

* Phase 3:

  * **Flat rate per block/day**
  * Accrue bằng JournalEntry `InterestAccrual`
* Không compounding phức tạp lúc này

### Transfer vs MarginTransfer

✔ **Reuse `Transfer`**

* Margin chỉ là **semantic layer trên ledger**
* Ledger không cần biết “margin”, chỉ biết tiền đi đâu

---

## 3. Liquidation Engine

### Trigger

✔ Dùng **Margin Ratio < 1.0**
✔ Liquidation price chỉ để **estimate / UI**

> Mark-price driven, không last-price.

### Insurance Fund

**In scope Phase 3 – bắt buộc**

Account:

```
EQUITY:SYSTEM:INSURANCE:USDT:MAIN
```

Flow:

1. Liquidation closes position
2. Nếu PnL < 0:

   * Debit user margin
   * Credit Insurance Fund
3. Nếu margin không đủ:

   * Insurance Fund absorbs loss

### Socialized loss

✖ **OUT OF SCOPE Phase 3**

* Quá phức tạp
* Dễ phá niềm tin

### ADL

✖ Phase 4

* Nhưng **RiskEngine phải emit `LiquidationSeverity` metric**
* Chuẩn bị dữ liệu cho ADL sau này

---

## 4. Multi-signature Approval

### N-of-M

✔ **2-of-3** default

* SYSTEM + 2 operators

### Pending state

✔ **SQLite projection**

* JSONL là SoT cho **committed**
* Pending ≠ committed → không nên vào journal

Table:

```
pending_approvals (
  id,
  unsigned_entry_hash,
  required,
  collected,
  expires_at
)
```

### Expiry

* Expired = auto-reject
* Reject **KHÔNG ghi ledger**
* Only approval → commit → ledger

---

## 5. Risk Engine Upgrades

### Oracle

✔ **Phase 3: External API + internal cache**

* Pull from 2 sources
* Median price
* If oracle fails → trading halt

### Price type

* **Index Price** → margin & liquidation
* **Mark Price** = Index + funding (future)
* **Last Price** = UI only

### Architecture

RiskEngine = **pure function**

```
State + MarketData → RiskDecision
```

No side effects.
Ledger commit = separate step.

---

## Default Choices để viết Phase 3 spec

Nếu bạn ok, mình recommend chốt luôn:

1. **Order types:** Limit only
2. **Matching:** CLOB, price-time
3. **Margin:** Cross-margin only
4. **Leverage:** Max 10x (configurable)
5. **Insurance fund:** IN SCOPE
6. **Liquidation:** Market close + insurance
7. **Price oracle:** External mockable API

👉 Với set này, Phase 3 không chỉ “chạy được”, mà **đủ nền để mở rộng thành derivatives exchange thực thụ** (5–10 năm).

Nếu bạn muốn, bước tiếp theo mình có thể:

* Giúp bạn **viết Phase 3 spec skeleton (sections + invariants)**
* Hoặc **review timeline 12 tuần và cắt rủi ro**
* Hoặc **đề xuất Phase 3.1 / Phase 4 roadmap (high-risk, high-reward)**
