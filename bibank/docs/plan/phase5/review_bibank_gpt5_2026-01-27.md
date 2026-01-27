Mình trả lời **thẳng vào kiến trúc**, không nói kiểu “kết nối API ví” chung chung.

---

## 1. Nguyên tắc sống còn (đúng với BiBank-class)

**Trust Wallet / MetaMask KHÔNG “kết nối trực tiếp” với hệ core của bạn.**
Chúng chỉ:

- ký giao dịch
- gửi giao dịch lên blockchain

👉 **Blockchain mới là điểm giao tiếp**, không phải ví.

Vì vậy:

- Ví = client-side signer
- Blockchain = settlement layer
- BiBank = **custodial state machine**

Nếu bạn “connect wallet rồi cộng trừ balance” → sai kiến trúc, sớm chết.

---

## 2. Mô hình chuẩn: 3-layer Deposit / Withdraw

```
Trust Wallet
   │
   │  (signed tx)
   ▼
Blockchain (ETH / BSC / Tron)
   │
   │  (on-chain event)
   ▼
Chain Watcher  ──▶  Ledger Commit  ──▶  Event Bus  ──▶  Projection
```

**Không có đường tắt.**

---

## 3. Deposit – cách làm đúng (high-reward)

### 3.1. Mỗi user = 1 deposit address (per-chain)

Không dùng memo nếu có thể.

```text
user_id → address (ETH / BSC / TRON)
```

Lưu mapping này trong DB riêng (không phải ledger).

---

### 3.2. Chain Watcher (module bắt buộc)

Bạn cần **1 service per chain**:

- ETH / BSC → listen ERC20 `Transfer`
- TRON → listen TRC20
- BTC → UTXO watcher

Watcher chỉ làm 1 việc:

> **Phát hiện sự kiện on-chain đã FINAL**

Ví dụ rule:

- ETH: ≥ 12 confirmations
- BSC: ≥ 20
- TRON: confirmed block

---

### 3.3. Khi phát hiện deposit hợp lệ

Chain Watcher **KHÔNG update balance**
Nó chỉ **emit intent**:

```json
DepositDetected {
  tx_hash,
  chain,
  address,
  asset,
  amount,
  block_height
}
```

---

### 3.4. Ledger ghi nhận deposit (double-entry)

Ledger sinh **Financial Event**:

```text
UserAssetAccount(user)     +100 USDT
SystemCustodyAccount       -100 USDT
```

✔ atomic
✔ deterministic
✔ replayable

👉 Deposit lúc này mới “có thật” trong BiBank.

---

## 4. Withdraw – nơi dễ chết nhất

### 4.1. Withdraw ≠ gửi tiền ngay

Flow chuẩn:

```
User Request
   ↓
Risk Engine (balance, AML, velocity)
   ↓
Approval (nếu lớn)
   ↓
Ledger: FundsLocked
   ↓
Blockchain Executor
   ↓
On-chain Tx Sent
   ↓
Chain Confirmed
   ↓
Ledger: FundsDebited
```

---

### 4.2. Ledger phải có trạng thái LOCK

Ví dụ:

```text
Available: 1,000 USDT
Locked:        200 USDT (withdraw pending)
```

Không có lock = double-spend nội bộ.

---

### 4.3. Blockchain Executor (hot wallet layer)

Executor:

- giữ private key (HSM / MPC sau này)
- đọc queue “ApprovedWithdraw”
- gửi tx

👉 **Executor KHÔNG có quyền sửa ledger**

Nó chỉ báo ngược lại:

```json
WithdrawBroadcasted { tx_hash }
WithdrawConfirmed { tx_hash }
```

---

## 5. Trust Wallet “kết nối” ở đâu?

### 5.1. Deposit

- User mở Trust Wallet
- Gửi token đến address hệ thống cấp
- Không cần SDK
- Không cần OAuth
- Không cần approve

✔ trust-minimized
✔ ví nào cũng dùng được

---

### 5.2. Withdraw

Trust Wallet **KHÔNG tham gia**.

Withdraw là:

- custodial
- do hệ thống ký
- user chỉ là người yêu cầu

---

## 6. Nếu muốn “Connect Wallet” (Web3-style)

Chỉ dùng cho:

- **KYC-less mode**
- proof of address ownership
- future non-custodial extension

Ví dụ:

- User ký message:

  ```
  "I own address 0xABC at time T"
  ```

- Map address ↔ user

👉 **KHÔNG dùng chữ ký này để chuyển tiền nội bộ.**

---

## 7. Đi xa 5–10–20 năm (đột phá)

### Ý tưởng high-risk / high-reward

1. **Internal Ledger = L2 off-chain**
   - Deposit = bridge in
   - Withdraw = bridge out

2. **Merkle proof cho user**
   - User có thể verify:

     > “Balance của tôi nằm trong root hash hôm nay”

3. **Proof-of-liability public**
   - Công khai:
     - total user balances
     - custody addresses

   - Không cần audit PDF

4. **Future MPC**
   - Executor không có private key hoàn chỉnh
   - 3/5 signer tự động + approval engine

---

## 8. Tóm gọn cực ngắn

- ❌ Không “connect Trust Wallet” theo nghĩa API
- ✅ Dùng blockchain làm giao diện
- ✅ Chain watcher → Ledger → Event Bus
- ✅ Deposit = on-chain truth → internal truth
- ✅ Withdraw = internal truth → on-chain execution

Nếu bạn muốn, mình có thể:

- vẽ **sequence diagram chi tiết**
- đề xuất **module Rust cho chain watcher**
- hoặc chỉ ra **điểm cần viết thêm trong repo hiện tại** (rất rõ chỗ nào còn thiếu)

Chọn hướng nào thì đi tiếp.
