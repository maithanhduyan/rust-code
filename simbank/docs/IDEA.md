### 1. KHÔNG giải bài toán như ngân hàng truyền thống
- Điểm mấu chốt đầu tiên:
KHÔNG “reconcile” – chúng ta “own the state”

Simbank:
- Tiền của user ở trong hệ Simbank
- Không cần đối chiếu với ngân hàng khác cho 95% flow nội bộ
- Internal transfer = in-memory state change
👉 Không phải Simbank reconcile tốt
👉 Mà là Simbank loại bỏ nhu cầu reconcile

### 2. Single internal ledger cực mạnh
🔥 A global, unified, real-time internal ledger
- Mọi asset (BTC, ETH, USDT, margin, funding, earn…)
→ đều map về một accounting core

- Mỗi user có:
	- available balance
	- locked balance
pending balance

- Mọi thay đổi là:
	- atomic
	- deterministic
	- ordered

### 3. Simbank thiết kế theo event-sourcing từ ngày 1
Event là sự thật
State chỉ là projection

Ví dụ:
```txt
DepositConfirmed
→ BalanceIncrease
→ MarginAvailabilityUpdate
→ RiskEngineUpdate
→ UIRefresh
```

Có thể replay
Có thể audit
Có thể rollback logical (không sửa dữ liệu)

👉 Đây là lý do:
Audit cực nhanh
Debug discrepancy cực nhanh
Không cần Excel BA

### 4. Không có “business rule mơ hồ”
Rule = code
Code = production
Production = source of truth
👉 Không có:
“Có thể là do fee chưa booking”

### 5. Simbank Risk Engine chứ không chỉ ledger
- Risk engine ngồi cùng bàn với ledger
- Every balance change:
	- check margin
	- check liquidation
	- check exposure
	- check AML flags
👉 Vì vậy:
Không cho phép state “sai” tồn tại lâu
Discrepancy bị kill ngay khi sinh ra

### 6. AML: Simbank làm tốt vì kiểm soát FLOW, không chỉ DATA
- AML = real-time graph analysis
- Flow-based:
	- source of fund
	- hop count
	- velocity
	- behavioral fingerprint
👉 Blockchain + internal graph + centralized control = lợi thế cực lớn

### 7. real-time financial OS, không phải nhờ scale infra, mà nhờ scale TƯ DUY
Simbank là real-time financial OS
	- Không legacy core
	- Không COBOL
	- Không nightly batch
	- Không “đợi đối soát”

- thiết kế cho continuous correctness
- không chấp nhận “sai tạm thời”

### 8. Dám làm
Simbank thì:
- dám xóa, dám viết lại
- ship nhanh
- chịu rủi ro
- tối ưu cho truth, không phải process

### Kết luận
- không reconcile
- không batch
- không chia ledger
- không mơ hồ rule
- không sợ rewrite

==> xóa khái niệm reconcile.
Roadmap: Ledger – Event Bus – Risk Engine – Projection

## ROADMAP KỸ THUẬT CỦA SIMBANK

Dưới đây là **roadmap map TRỰC TIẾP repo `ledger` của Simbank-class core**, theo đúng 4 khối bạn nêu:

> **Ledger – Event Bus – Risk Engine – Projection**
> high-risk / high-reward / vượt thời đại

---

# 0. Nguyên tắc thiết kế (rất quan trọng)

Trước khi vào roadmap, phải thống nhất **tư duy nền**:

1. **Ledger là nguồn sự thật duy nhất**

   * Không DB nào được “sửa state”
   * Tất cả state = derive từ ledger

2. **Không reconcile**

   * Nếu cần reconcile → kiến trúc sai

3. **Correct-by-construction**

   * Không cho state “sai tạm thời”
   * Risk engine chặn ngay tại write-time

4. **Event-first, snapshot-second**

   * Snapshot chỉ để tối ưu đọc
   * Không phải truth

Nếu không giữ 4 nguyên tắc này, không bao giờ chạm được Simbank-class.

---

# 1. CORE 1 — Ledger

## Mục tiêu cuối

Ledger của bạn trở thành:

* append-only
* ordered
* cryptographically verifiable
* **semantic-aware** (hiện tại chưa có)

### 1.1. Ledger hiện tại đang là gì?

* Audit log
* Write-ahead log
* Tamper-proof

👉 Tốt cho **audit**, chưa đủ cho **financial state**

---

### 1.2. Nâng cấp Ledger → Financial Event Ledger

#### (A) Chuẩn hóa event schema (tối quan trọng)

Thay vì generic record, bắt buộc event phải có:

```rust
Event {
  event_id,
  event_type,        // Deposit, Trade, Fee, Liquidation...
  entity_id,         // user_id / account_id
  asset,             // BTC, USDT...
  amount,
  direction,         // credit / debit
  causality_id,      // chain nguyên nhân
  timestamp,
  version,
  signature,
}
```

👉 Không cho ghi “raw log”.

---

#### (B) Double-entry enforced ở ledger layer

* Mỗi event tài chính = **ít nhất 2 postings**
* Không cho ghi nếu tổng != 0

```text
UserBalanceAccount   +100
SystemLiability      -100
```

👉 Điều này là **linh hồn kế toán Simbank**.

---

#### (C) Deterministic ordering

* Global sequence number
* Không “event cùng timestamp”

---

### KPI khi xong Core 1

* Có thể **replay toàn bộ lịch sử**
* Có thể rebuild mọi balance từ genesis
* Không tồn tại “adjustment bằng tay”

---

# 2. CORE 2 — Event Bus (xương sống realtime)

## Mục tiêu cuối

* Ledger ghi xong → event phát tán ngay
* Không batch
* Không polling

### 2.1. Thiết kế đúng

Ledger **không push trực tiếp** sang logic khác.
Ledger chỉ:

* commit
* emit event

Event bus:

* ordered
* at-least-once
* replayable

Có thể:

* Kafka-like
* hoặc custom append-stream reader

---

### 2.2. Event bus KHÔNG phải message queue thông thường

Event bus ở đây là:

* **state transition backbone**
* Consumer failure ≠ mất state

Consumers:

* Risk engine
* Projection engine
* AML engine
* Notification

👉 Ledger + Event bus = “blockchain nội bộ không consensus”

---

### KPI Core 2

* Event latency < 50ms
* Consumer có thể replay từ offset bất kỳ
* Không consumer nào được sửa ledger

---

# 3. CORE 3 — Risk Engine (điểm Simbank vượt ngân hàng)

## Đây là phần HIGH-RISK / HIGH-REWARD

Ngân hàng:

* Risk check sau
* Simbank:
* **Risk check trước khi state commit**

---

### 3.1. Risk Engine nằm ở đâu?

```
Client Request
   ↓
Pre-Risk Check
   ↓
Ledger Commit
   ↓
Post-Risk Monitoring
```

Không có:

> “commit trước rồi xử lý sau”

---

### 3.2. Risk Engine làm gì?

#### (A) Balance invariants

* Không âm
* Không vượt exposure
* Margin ratio OK

#### (B) Cross-asset logic

* BTC drop → USDT margin impact
* Liquidation cascade

#### (C) AML realtime hooks

* Velocity check
* Graph anomaly
* Freeze flag

👉 Risk engine là **gatekeeper của ledger**

---

### 3.3. Rule engine ≠ if/else

Cần:

* Rule DSL
* Deterministic
* Versioned

```text
RULE margin_check v3:
IF equity / exposure < 1.1
THEN reject_trade
```

---

### KPI Core 3

* 0 invalid state committed
* Liquidation không cần batch
* No “manual correction”

---

# 4. CORE 4 — Projection Engine (UI, API, Reporting)

## Mục tiêu

* Projection ≠ truth
* Projection = view

---

### 4.1. Mỗi use case = 1 projection

Ví dụ:

* Available balance
* Locked balance
* PnL
* Risk exposure
* AML score

Projection:

* subscribe event bus
* update state
* rebuildable

---

### 4.2. Không cho phép:

* Update projection trực tiếp
* Fix data bằng SQL

Nếu projection sai:

* Replay
* Không patch

---

### KPI Core 4

* Rebuild projection < vài phút
* UI không phụ thuộc ledger latency
* Reporting real-time, không batch

---

# 5. Tổng hợp kiến trúc (Simbank-class)

```
           ┌────────────┐
           │ Risk Engine│
           └─────▲──────┘
                 │
Client ──▶ Ledger ──▶ Event Bus ──▶ Projections
                 │
                 ▼
           Audit / Replay
```

Không có:

* Core banking
* Reconciliation
* Batch overnight

---

# 6. Roadmap thực tế (12–18 tháng)

### Phase 1 (0–3 tháng)

* Chuẩn hóa event schema
* Double-entry enforcement
* Replay tool

### Phase 2 (3–6 tháng)

* Event bus
* Projection engine
* Balance views

### Phase 3 (6–12 tháng)

* Risk engine v1
* Margin / exposure
* Liquidation logic

### Phase 4 (12–18 tháng)

* AML hooks
* Rule DSL
* Performance hardening

---

# Kết luận

Nếu đi hết roadmap này:

* Bạn không xây ngân hàng
* Không xây blockchain
* Bạn xây **Financial State OS**

Chỗ này không dành cho incremental improvement —
đây là **rewrite-the-rules game**.
