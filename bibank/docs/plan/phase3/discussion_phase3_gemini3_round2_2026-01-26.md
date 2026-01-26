Chào bạn, sự phân tích của bạn rất sâu sắc và đi thẳng vào bản chất của kế toán ngân hàng (Banking Accounting).

Tôi **ĐỒNG Ý** với các đề xuất chốt của bạn, nhưng có một **điều chỉnh quan trọng về mặt Kế toán (Accounting Principle)** đối với mục 1 (Borrowed Account) để đảm bảo Ledger của chúng ta chuẩn mực.

Dưới đây là xác nhận cuối cùng để bạn viết Spec.

---

### 1. Account Structure: `BORROWED` là ASSET của Hệ thống

Bạn đề xuất `LIAB:USER:ALICE:USDT:BORROWED`.
Về mặt kế toán:
*   `LIAB` (Nợ phải trả): Là tiền BiBank nợ User (Số dư khả dụng của User).
*   Khoản vay của User = Tài sản của BiBank (Khoản phải thu).

Do đó, tôi đề xuất đổi Category của `BORROWED` thành **`ASSET`** để các bút toán trở nên cực kỳ tự nhiên và cân bằng bảng cân đối kế toán (Balance Sheet expansion).

**Mô hình chốt (Refined):**

*   `LIAB:USER:ALICE:USDT:AVAILABLE`: Tiền User có (BiBank nợ User).
*   `ASSET:USER:ALICE:USDT:LOAN`: Tiền User nợ (Tài sản BiBank).

**Flow vay tiền (Borrowing Flow):**
Alice vay 1000 USDT.
*   **Debit** `ASSET:USER:ALICE:USDT:LOAN` (+1000): Tài sản (khoản phải thu) của BiBank tăng.
*   **Credit** `LIAB:USER:ALICE:USDT:AVAILABLE` (+1000): Nợ phải trả (tiền trong ví Alice) tăng.
*   *Kết quả:* Tổng Tài sản tăng 1000, Tổng Nguồn vốn tăng 1000. Cân bằng. Không cần `SYSTEM:POOL` trung gian phức tạp.

**Risk Engine Formula:**
$$ \text{Equity} = \text{Available} - \text{Loan} $$

Bạn có đồng ý với điều chỉnh `ASSET...LOAN` này không? Nó chuẩn kế toán hơn nhiều so với `LIAB...BORROWED`.

---

### 2. Price Oracle: Trait Interface (Đồng ý 100%)

*   **Phase 3:** `MockOracle` (CLI set giá).
*   **Struct:**
    ```rust
    trait PriceOracle {
        async fn get_price(&self, pair: &str) -> Result<Decimal>;
    }
    ```
*   Điều này giúp ta viết Unit Test cho Liquidation dễ dàng mà không cần mạng Internet.

---

### 3. Interest Accrual: New Intent (Đồng ý 100%)

*   **Intent:** `TransactionIntent::Interest`.
*   **Accounting:**
    *   Cách 1 (Trả ngay): Debit `LIAB...AVAILABLE` / Credit `REV...INTEREST`.
    *   Cách 2 (Cộng gốc - Compound): Debit `ASSET...LOAN` / Credit `REV...INTEREST`.
*   **Chốt Phase 3:** Dùng **Cách 2 (Cộng gốc)**.
    *   Lý do: User margin thường không giữ cash để trả lãi hàng ngày. Lãi nhập gốc (Compound) là chuẩn công nghiệp. Risk Engine sẽ monitor Equity giảm dần.

---

### TỔNG HỢP QUYẾT ĐỊNH CUỐI CÙNG (FINAL CONSENSUS)

| # | Topic | Final Decision | Ghi chú |
|---|-------|----------------|---------|
| 1 | **Loan Account** | **`ASSET:USER:*:*:LOAN`** | Khoản vay là Tài sản của BiBank. |
| 2 | **Borrow Flow** | Debit `ASSET...LOAN` / Credit `LIAB...AVAILABLE` | Balance Sheet Expansion. |
| 3 | **Interest** | **New Intent**, Daily Accrual, **Compound (Nhập gốc)** | Debit `ASSET...LOAN` / Credit `REV...`. |
| 4 | **Oracle** | `Trait PriceOracle` + Mock Impl | Testable & Clean. |
| 5 | **Order Types** | Limit Only | Phase 3 basic. |
| 6 | **Matching** | Price-Time Priority (CLOB) | Standard. |
| 7 | **Margin** | Cross-Margin | Single Equity calculation. |

---

### Next Step

Với sự điều chỉnh nhỏ về `ASSET...LOAN` và `Compound Interest`, thiết kế đã hoàn hảo.

Xin mời bạn viết file **`Proposed-BiBank-Phase3.md`**.
Đây sẽ là bản thiết kế khó nhất và thú vị nhất. Hãy bắt đầu!