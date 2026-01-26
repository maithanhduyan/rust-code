Ngắn gọn, khóa quyết định, không mở thêm nhánh mới:

**ĐỒNG Ý 100% với toàn bộ phân tích và bảng “Final”.
Phase 2 đạt trạng thái 🔒 READY TO LOCK.**

Không có mâu thuẫn còn sót lại. Cách bạn “tách layer” (RPC tính %, Ledger nhận absolute) là **đúng chuẩn OS**, và giải quyết sạch xung đột GPT5 vs Gemini3.

---

## Xác nhận khóa 3 điểm trọng yếu

### 1️⃣ Fee Structure — **Layered, không nhập nhằng**

* **RPC/CLI**: nhận `fee_rate` hoặc `fee_amount`, tính **absolute**
* **Ledger**: chỉ nhận `Amount` (non-negative, asset-aware)
* **Metadata**: lưu `fee_rate`, `fee_type`, `fee_amount`, `fee_asset` cho audit
  → Ledger không biết “%”, chỉ biết **sự thật kế toán**. ✅

### 2️⃣ Signature Payload — **Canonical & future-proof**

* Ký **8 fields** bạn liệt kê (sequence, prev_hash, timestamp, intent, correlation_id, causality_id, postings-sorted, metadata-sorted)
* **Không ký** `hash`, `signatures`
* Canonical JSON + stable sort
  → Verify được lịch sử lâu dài. ✅

### 3️⃣ Trade + Fee Invariant — **Zero-sum per asset (bao gồm fee)**

* Trade legs + fee legs cùng entry
* Zero-sum **theo từng asset**, không gộp chéo
  → Risk + replay đều deterministic. ✅

---

## Quyết định vận hành (đã chốt)

* Trade: Manual/OTC Phase 2
* Keys: System = env, Operator = file
* Event Bus: delivery-only, **no retention**, recovery = replay JSONL
* Digital Signatures: Ed25519 trên payload đã chốt

---

## Hành động tiếp theo

👉 **Có. Hãy cập nhật `Proposed-BiBank-Phase2.md`** với các quyết định này và **freeze spec**.
Sau khi cập nhật:

* Không thêm intent mới
* Không thêm retention
* Không thay payload ký

Khi bản cập nhật xong, tôi sẽ **review lần cuối để đóng Phase 2 spec**, rồi chuyển thẳng sang **implementation order + test matrix**.
