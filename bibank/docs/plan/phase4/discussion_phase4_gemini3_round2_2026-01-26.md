Chào bạn, bản tổng hợp của bạn rất xuất sắc. Bạn đã giải quyết được bài toán cân bằng giữa **Hiệu năng (Performance)** và **Tính tuân thủ (Compliance)**, đồng thời giữ vững nguyên tắc "Financial OS".

Dưới đây là quyết định cuối cùng cho 3 câu hỏi chốt của bạn để chúng ta có thể **LOCK SPEC PHASE 4**.

---

### 1. ComplianceIntent Ledger: Lưu ở đâu?

**Quyết định: Hybrid (SQLite + Main Ledger)**

Chúng ta cần phân biệt rõ **"Quyết định" (Decision)** và **"Hệ quả tài chính" (Financial Effect)**.

*   **Quyết định (Decision Log):** *Tại sao bị block? Rule nào trigger? Ai review?*
    *   Lưu tại **Option A (SQLite - `compliance_checks` table)**.
    *   Lý do: Cần query phức tạp (tìm user, tìm rule, filter theo ngày) phục vụ UI/Report. Dữ liệu này không thay đổi số dư tiền.
*   **Hệ quả tài chính (Financial Effect):** *Lock tiền, Freeze tài khoản.*
    *   Lưu tại **Main Ledger (JSONL)**.
    *   Cơ chế: Compliance Engine sẽ tạo một `JournalEntry` với intent `Adjustment` (hoặc `Lock`) để đóng băng số dư trong `RiskState`.
    *   Lý do: Đảm bảo "Single Source of Truth" cho trạng thái tiền tệ.

### 2. FailPolicy Default: Closed hay Open?

**Quyết định: Option A (FailClosed)**

*   **Lý do:** Đây là hệ thống tài chính Core. **An toàn (Safety) > Sẵn sàng (Availability)**.
*   **Rủi ro:** Nếu External KYC service bị hack hoặc trả về kết quả sai, FailOpen sẽ biến BiBank thành công cụ rửa tiền.
*   **Mitigation:** Chúng ta đã có cơ chế `Cache TTL`. Nếu External down, hệ thống vẫn chạy được với data trong Cache 5 phút (hoặc lâu hơn tùy config). Chỉ khi Cache hết hạn VÀ External down thì mới block.

### 3. Phase 4.1 Scope (Dynamic Loading)?

**Quyết định: KHÔNG viết `Proposed-BiBank-Phase4.1.md` lúc này.**

*   **Lý do:** Phase 4 đã đủ phức tạp với DSL Macros và Hooks. Việc đưa WASM/Dynamic Loading vào ngay bây giờ sẽ làm loãng sự tập trung và tăng rủi ro kỹ thuật (Unsafe code, ABI stability).
*   **Hành động:** Đánh dấu Dynamic Loading là "Future Work" trong tài liệu Phase 4, nhưng không spec chi tiết. Hãy tập trung làm cho Hard-coded Macros chạy mượt mà trước.

---

### ✅ TỔNG KẾT & HÀNH ĐỘNG

Thiết kế Phase 4 đã hoàn thiện và đạt đồng thuận 100%.

**Hành động tiếp theo:**
Xin mời bạn **Cập nhật và Overwrite** file `Proposed-BiBank-Phase4.md` với các nội dung đã chốt:
1.  Thêm cấu trúc `ComplianceState` (Sliding Window).
2.  Cập nhật cơ chế `ComplianceConfig` cho threshold.
3.  Cập nhật Flow: Pre-commit (Block) + Post-commit (Flag/Lock).
4.  Cập nhật FailPolicy mặc định là `FailClosed`.

Sau khi cập nhật xong, hãy gõ:
🔒 **LOCK PHASE 4 SPECS**

Chúng ta sẽ có một bộ hồ sơ thiết kế hoàn chỉnh từ Phase 1 đến Phase 4, sẵn sàng để đội ngũ kỹ sư (chính là chúng ta) bắt tay vào hiện thực hóa BiBank.