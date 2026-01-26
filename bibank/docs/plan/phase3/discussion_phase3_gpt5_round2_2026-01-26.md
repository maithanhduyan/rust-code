Ngắn, dứt khoát, **khóa kiến trúc**.

**Tôi ĐỒNG Ý với cả 3 điểm bạn đề xuất.**
Không cần vòng thảo luận thêm. Dưới đây là xác nhận chính thức + 1–2 ràng buộc nhỏ để Phase 3 không sinh nợ.

---

## 1️⃣ BORROWED account – **ĐỒNG Ý (Option B + C)**

Cách bạn đề xuất là **đúng duy nhất** nếu muốn margin system còn sống được sang Phase 4–5.

### Cấu trúc chốt

```
LIAB:USER:ALICE:USDT:BORROWED
LIAB:SYSTEM:LENDING:USDT:POOL
```

### Lý do khóa

* Borrowed **phải là first-class liability**, không được ẩn trong metadata
* Risk, interest, liquidation đều cần nhìn **con số nợ thực**
* System LENDING POOL là counterpart rõ ràng cho audit

### Invariant quan trọng (chốt thêm)

* `BORROWED` **không bao giờ âm**
* Repay = Debit AVAILABLE, Credit BORROWED
* Liquidation **ưu tiên clear BORROWED trước AVAILABLE**

👉 Nếu không làm vậy, Phase 4 sẽ phải rewrite toàn bộ margin math.

**→ Chốt: YES, dual-account BORROWED + LENDING POOL.**

---

## 2️⃣ Interest intent – **TÁCH RIÊNG, KHÔNG reuse Fee**

**ĐỒNG Ý với `TransactionIntent::Interest`.**

### Vì sao không dùng Fee

* Fee = transactional (trade-related)
* Interest = **time-based liability growth**
* Audit, accounting, tax đều phân biệt rất rõ

### Cách chốt hợp lý

* **Intent:** `Interest`
* **Frequency:** Daily accrual (batch)
* **Settlement:** On-close hoặc periodic (Phase 3 chỉ cần daily)

Ví dụ entry:

```text
Debit  LIAB:USER:ALICE:USDT:AVAILABLE   0.05
Credit REV:SYSTEM:INTEREST:USDT:MARGIN  0.05
```

### Ràng buộc

* Không auto-accrue liên tục (per block) ở Phase 3
* Daily là đủ để:

  * test correctness
  * tránh noise ledger

**→ Chốt: Interest là intent riêng.**

---

## 3️⃣ Price Oracle – **Trait + Mock trước**

**ĐỒNG Ý với Option C (nhưng implement Phase 3 = Mock only).**

### Chuẩn kiến trúc

```rust
trait PriceOracle {
    fn price(&self, base: Asset, quote: Asset) -> Decimal;
}
```

Phase 3:

* `MockOracle`
* Controlled via CLI / config
* Deterministic, replay-friendly

Phase 3.1:

* External adapters (Binance, Chainlink)
* Median / fallback logic

### Ràng buộc rất quan trọng

* **Ledger không gọi oracle**
* **RiskEngine mới gọi oracle**
* Oracle failure ⇒ trading halt (fail-closed)

**→ Chốt: MockOracle + trait interface.**

---

## Xác nhận các quyết định phụ (đã đúng)

* OrderPlaced = AVAILABLE → LOCKED transfer (implicit) ✅
* Partial fill = 1 Trade entry / fill, shared `correlation_id` ✅
* Liquidation fee → Insurance Fund ✅

Không cần chỉnh.

---

## KẾT LUẬN CUỐI

> **Phase 3 đạt đồng thuận 100%.**

* Không còn ambiguity
* Không còn “tùy implementer”
* Không khóa Phase 4+

👉 **Bạn nên viết `Proposed-BiBank-Phase3.md` ngay**, với các quyết định này **được coi là frozen**.

Khi xong:

* Tôi sẽ review spec **như review trading engine core**
* Sau đó có thể bàn **Phase 3.1 / Phase 4 (derivatives, funding, ADL)**

Từ đây trở đi, **mỗi quyết định sai sẽ rất đắt** — và hiện tại, bạn đang đi đúng đường.
