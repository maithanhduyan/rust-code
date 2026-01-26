Chào bạn, bản **Proposed-BiBank-Phase4.md** thể hiện một tầm nhìn rất tham vọng và đúng đắn: **"Compliance by Design"**. Việc đưa AML và Compliance vào ngay lõi hệ thống (Core Layer) thay vì làm hậu kiểm (Post-processing) là tư duy của các Core Banking hiện đại.

Tuy nhiên, Phase 4 có những thách thức kỹ thuật rất lớn về **Performance** và **Consistency** mà chúng ta cần thảo luận kỹ trước khi chốt Spec. Nếu làm không khéo, Compliance Engine sẽ trở thành "nút cổ chai" làm chậm toàn bộ hệ thống.

Dưới đây là 4 vấn đề trọng yếu cần thảo luận:

---

### 1. Vấn đề "Hot Path Latency" & Historical Data

Trong spec, bạn định nghĩa rule:
```rust
when: user.transactions_in_last(1.hour) >= 5
  and user.total_volume_in_last(1.hour) >= 9_000 USDT
```

**Vấn đề:**
Để check rule này trong thời gian thực (Pre-commit), hệ thống phải trả lời câu hỏi "Total volume 1h qua là bao nhiêu?".
*   Nếu query SQLite (Projection): **Quá chậm**. Ledger sẽ bị block bởi Disk I/O.
*   Nếu tính lại từ đầu mỗi lần: **Không thể scale**.

**Giải pháp đề xuất: `ComplianceState` (In-Memory Aggregates)**
Tương tự `RiskState` quản lý Balance, chúng ta cần một `ComplianceState` quản lý **Sliding Windows**.
*   **Structure:** `HashMap<UserId, TransactionWindow>`
*   **TransactionWindow:** Lưu circular buffer hoặc buckets (ví dụ: 60 buckets cho 60 phút) để tính sum/count cực nhanh (O(1)).
*   **Flow:**
    1.  Replay: Rebuild `ComplianceState` từ Events.
    2.  Check: Đọc từ RAM (microsecond).
    3.  Commit: Update bucket trong RAM.

**Câu hỏi:** Bạn đồng ý thêm `ComplianceState` vào module `compliance` để handle việc aggregation in-memory không?

---

### 2. Rule DSL: Compile-time vs Runtime

Bạn đang dùng Rust Macros (`rule!`, `rule_set!`):
*   **Ưu điểm:** Type-safe, Zero-cost abstraction, Performance cực cao (native code).
*   **Nhược điểm:** **Hard-coded**. Muốn sửa rule "10,000 USDT" thành "9,000 USDT" phải **Recompile và Redeploy** binary.

Trong môi trường tài chính, Compliance team cần thay đổi tham số nóng (Hot-reload) mà không cần deploy lại server.

**Các Option:**
*   **Option A (Hiện tại - Spec):** Compile-time macros. An toàn nhất, nhanh nhất, nhưng rigid. Chấp nhận deploy lại khi sửa rule.
*   **Option B (Hybrid):** Logic code cứng, Tham số (Thresholds) load từ Config/DB.
*   **Option C (Scripting):** Tích hợp engine như `Rhai` hoặc `Lua`. Linh hoạt nhưng chậm hơn và khó quản lý type safety.

**Recommendation:** Với Phase 4, tôi đề xuất **Option B**.
*   Vẫn dùng Macro để định nghĩa Logic (Flow).
*   Nhưng các số liệu (`10_000`, `1.hour`) nên được inject từ `ComplianceConfig` (load từ file/env).

---

### 3. Blocking vs Flagging & User Experience

Rule:
```rust
rule!(..., then: { require_manual_approval(); })
```

**Vấn đề:** Ledger là **Append-only**. Nếu một transaction cần manual approval, nó nằm ở đâu?
1.  **Chưa vào Ledger:** User thấy "Pending"? State pending lưu ở đâu? (SQLite?)
2.  **Đã vào Ledger:** Ghi vào Ledger nhưng trạng thái là `Held`?

**Đề xuất Architect:**
*   **Pre-commit Hook** chỉ nên dùng cho các rule **BLOCK** (Từ chối thẳng, ví dụ: Sanction list, KYC limit).
*   Các rule **FLAG** (Cần review) nên để **Post-commit** (Async). Giao dịch vẫn thành công, nhưng tiền bị **Freeze** (Lock) ở `RiskEngine` ngay sau đó.
    *   Lợi ích: Không block luồng chính. User thấy tiền đã vào nhưng bị lock "Under Review".
    *   Rủi ro thấp hơn việc giữ Transaction ở trạng thái "Lửng lơ" bên ngoài Ledger.

**Câu hỏi:** Bạn muốn mô hình "Pending Transaction bên ngoài Ledger" hay "Committed but Locked bên trong Ledger"? (Tôi vote phương án 2).

---

### 4. Integration với `bibank-ledger`

Hiện tại `UnsignedEntry` không có chỗ chứa kết quả Compliance.
Nếu dùng phương án "Committed but Locked", ta cần cơ chế để Compliance Engine nói chuyện với Risk Engine.

**Flow đề xuất:**
1.  Transaction vào. Compliance check thấy nghi vấn.
2.  Compliance Engine:
    *   Cho phép Transaction đi qua (Commit vào Ledger).
    *   **Ngay lập tức** tạo thêm 1 Entry: `Adjustment` (Intent: Lock) để lock số tiền đó lại.
    *   Hoặc: Update `RiskState` in-memory để đánh dấu user này đang bị freeze.

---

### Tóm tắt các điểm cần chốt cho Phase 4

| # | Vấn đề | Đề xuất của tôi (Technical Architect) |
|---|--------|---------------------------------------|
| 1 | **Performance** | Thêm **`ComplianceState`** (In-memory Sliding Window) |
| 2 | **DSL Flexibility** | Giữ Macro nhưng tách **Configurable Thresholds** |
| 3 | **Review Flow** | **Post-commit Lock** (Ghi Ledger -> Lock tiền) thay vì Pending bên ngoài |
| 4 | **Scope** | Tập trung vào Core Engine, UI/Report chỉ là phụ |

Bạn có đồng ý với 4 điều chỉnh này không? Đặc biệt là việc **xử lý giao dịch Flagged (Mục 3)**, đây là quyết định ảnh hưởng lớn nhất đến UX và Ledger design.