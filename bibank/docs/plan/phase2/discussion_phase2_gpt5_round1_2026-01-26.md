Rõ. Phase 1 đã **đóng**, Phase 2 spec đã **đủ hình hài**.
Câu hỏi của bạn không phải “có ổn không”, mà là:

> **Trước khi LOCK Phase 2, nên review/thảo luận CÁI GÌ để không tạo nợ kiến trúc?**

Tôi trả lời thẳng: **chỉ có 4 điểm xứng đáng bàn**, còn lại **KHÔNG cần thảo luận nữa**.

---

# 🔒 Những gì KHÔNG cần bàn (đã đúng, đã đủ)

Không thảo luận lại:

* Trade = multi-asset atomic JournalEntry
* Fee = LIAB → REV
* Event bus async, at-least-once
* Signature = Ed25519 trên payload đã hash
* JSONL vẫn là source of truth
* Phase 2 **không** auto-matching, **không** margin

Nếu ai đó muốn “bàn thêm” các điểm trên → **đang kéo Phase 2 lệch sang Phase 3**.

---

# ⚠️ 4 ĐIỂM DUY NHẤT CẦN REVIEW TRƯỚC KHI LOCK

## 1️⃣ Fee structure – **CHỈ CHỐT 1 CÁCH**

### Quyết định nên chốt ngay

👉 **Percentage-based fee, tính trên quote asset**

Ví dụ:

* Trade BTC/USDT
* Fee = 0.1% × quote_amount (USDT)

### Vì sao KHÔNG fixed fee Phase 2

* Fixed fee phá invariant khi asset price scale
* Không audit-friendly cho trade history
* Sẽ phải rewrite khi có volume tier

📌 **Khuyến nghị chốt:**

```text
Phase 2:
- Fee = percentage
- Asset = quote asset
- Rounding rule: floor, asset-decimal aware
```

> Fixed fee → Phase 4 (business experiment), **không phải core OS concern**

---

## 2️⃣ Trade + Fee trong 1 entry – **INVARIANT RẤT QUAN TRỌNG**

Spec cho phép:

* Trade postings (4)
* * Fee postings (2)
    → trong **cùng JournalEntry**

❗ Cần chốt invariant sau, nếu không risk engine sẽ loạn:

### Invariant bắt buộc

* **Fee postings KHÔNG được ảnh hưởng zero-sum của trade legs**
* Zero-sum check:

  * per asset
  * **bao gồm fee asset**

Ví dụ:

* USDT:

  * Trade: +100 / -100 = 0
  * Fee: -0.1 / +0.1 = 0
* BTC:

  * Trade: +0.001 / -0.001 = 0

👉 Tổng vẫn = 0 per asset → ledger invariant giữ.

📌 Nếu không chốt rõ, Phase 2 sẽ sinh bug “trade đúng nhưng fee làm entry fail”.

---

## 3️⃣ Digital Signature – **KÝ CÁI GÌ? (CỰC QUAN TRỌNG)**

Hiện spec ghi:

> “Sign hash of entry excluding signatures field”

Cần **chốt chính xác payload**, nếu không Phase 3 sẽ không verify được historical entries.

### Payload chuẩn nên là:

```
HASH(
  sequence
  prev_hash
  intent
  postings (canonical order)
  metadata (canonical JSON)
  causality_id
  correlation_id
  timestamp
)
```

### KHÔNG ký:

* signatures
* projection-related data
* derived fields

📌 **Chốt rule**:

> Signature covers **semantic financial truth**, không covers transport/view.

Nếu không khóa payload ngay → future incompatibility.

---

## 4️⃣ Event Bus retention – **KHÔNG giữ trong bus**

Câu hỏi:

> “Giữ events trong bus bao lâu?”

### Câu trả lời đúng cho BiBank:

👉 **Event bus KHÔNG có retention logic.**

* Event bus = delivery mechanism
* Retention = JSONL ledger responsibility
* Subscriber tự track offset + replay từ ledger

📌 Chốt:

```text
EventBus:
- no retention
- no durability guarantees
- crash = OK
- recovery = replay from ledger
```

Nếu bạn thêm retention vào bus → bạn đang xây Kafka-lite **không cần thiết**.

---

# ✅ KẾT LUẬN – CHỐT ĐỂ LOCK PHASE 2

## Trước khi lock Phase 2, chỉ cần:

1. Chốt **percentage fee trên quote asset**
2. Chốt **trade+fee invariant per asset**
3. Chốt **exact signature payload**
4. Chốt **event bus = no retention**

Sau đó:

* LOCK spec
* Không thêm intent
* Không thêm subscriber type
* Không thêm key type

---

# 🚀 BƯỚC TIẾP THEO ĐÚNG NHỊP

Nếu bạn muốn đi nhanh và đúng:

* **Lock Phase 2 spec**
* Implement theo thứ tự:

  1. Trade intent + tests
  2. Fee intent + tests
  3. Signature hardening
  4. Event bus async

Không cần brainstorm thêm.
Từ đây trở đi, **chất lượng BiBank phụ thuộc vào kỷ luật code, không phải ý tưởng**.

Khi bạn sẵn sàng:
→ tôi sẽ **review Phase 2 implementation PR như review kernel patch**.
