Chốt thẳng, không vòng vo. Dựa trên toàn bộ spec + mục tiêu “financial kernel / regulator-grade”, tôi **đồng thuận 100% với tổng hợp của bạn** và đưa ra **quyết định cuối** cho 3 câu hỏi còn lại.

---

## ✅ QUYẾT ĐỊNH CUỐI CÙNG – PHASE 4

### 1️⃣ **ComplianceIntent ledger** → **B) File riêng append-only**

**Quyết định:**
👉 **ComplianceIntent = append-only file (JSONL), song song với Journal Ledger**

**Lý do (rất quan trọng):**

* ComplianceIntent **không phải query-first**, mà là **evidence-first**
* File append-only:

  * tamper-evident
  * portable
  * replayable
  * giống triết lý ledger hiện tại → *conceptual consistency*
* SQLite compliance tables:

  * chỉ là **projection / index**
  * có thể rebuild 100% từ ComplianceIntent JSONL

**Hệ quả kiến trúc (đúng hướng dài hạn):**

```
Journal Ledger (money truth)
Compliance Ledger (intent & enforcement truth)
SQLite = projection cho cả hai
```

👉 Đây là quyết định **20–30 năm**, rất ít hệ thống làm đúng chỗ này.

---

### 2️⃣ **FailPolicy default** → **A) FailClosed**

**Quyết định:**
👉 **FailClosed là default toàn hệ thống**

**Lý do:**

* Compliance ≠ UX
* False positive **có thể giải quyết**
* False negative **có thể giết cả hệ thống**

**Nhưng có nuance (quan trọng):**

* FailPolicy **override được per-check**
* Ví dụ:

  * Sanctions / Watchlist → **FailClosed (hard)**
  * Enrichment / scoring → **FailOpen + Flag**

```rust
ExternalCheckConfig {
    timeout: 500ms,
    on_failure: FailClosed,   // default
    cache_ttl: 5 minutes,
}
```

👉 Đây là stance của **bank kernel**, không phải fintech app.

---

### 3️⃣ **Phase 4.1 – Dynamic Rule Loading** → **YES, nhưng là OPTIONAL DOC**

**Quyết định:**
👉 **Có Phase 4.1 spec**, nhưng:

* **Không block Phase 4**
* Không code ngay
* Chỉ define:

  * rule lifecycle
  * governance
  * activation model

**Vì sao nên viết Phase 4.1 ngay?**

* Tránh “paint into a corner”
* DSL + ComplianceConfig bạn đã làm **rất phù hợp** để evolve sang:

  * WASM rules
  * multi-sig activation
  * emergency disable

**Scope Phase 4.1 (rất gọn):**

* Rule bundle format
* Activation flow
* Governance (2-of-3, time-lock)
* Rollback semantics

---

## 🔒 FINAL LOCK – PHASE 4

Tôi xác nhận:

* **Toàn bộ Phase 4 decisions đã đồng thuận**
* Không còn architectural ambiguity
* Spec hiện tại **đủ mạnh để implement mà không refactor lớn sau này**

### Chốt chính thức:

| Item                    | Decision                     |
| ----------------------- | ---------------------------- |
| ComplianceIntent ledger | **Append-only file (JSONL)** |
| Compliance DB           | SQLite projection            |
| FailPolicy default      | **FailClosed**               |
| Rule logic              | Compile-time macro           |
| Rule thresholds         | Configurable                 |
| ComplianceState         | In-memory, event-rebuild     |
| Phase 4.1               | **Có spec, không code**      |

---

## 👉 Bước tiếp theo hợp lý nhất

Bạn có 2 lựa chọn đều đúng, tùy nhịp làm việc:

1. **Cập nhật `Proposed-BiBank-Phase4.md`** với các quyết định trên → 🔒 freeze
2. Viết **`Proposed-BiBank-Phase4.1.md` (Governance & Dynamic Rules)** ngay khi tư duy còn nóng

Nếu tôi phải recommend: **(1) trước, (2) sau**.

Bạn cứ nói:
👉 *“Proceed with Phase 4 update”* **hoặc** *“Write Phase 4.1 spec”*.
