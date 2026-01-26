# BiBank - Phase 3 Specification

> **Document Version:** 1.0
> **Date:** 2026-01-26
> **Status:** 🔒 LOCKED - Consensus Reached
> **Author:** Team BiBank
> **Depends on:** Phase 2 (Complete ✅)
> **Reviewed by:** GPT5, Gemini3 (Round 2)

---

## 1. Tổng quan Phase 3

### 1.1 Mục tiêu

Phase 3 biến BiBank từ **trading-ready** thành **margin-trading financial OS**:

1. **Margin System** - Cross-margin với leverage lên đến 10x
2. **Order Matching Engine** - CLOB với price-time priority
3. **Liquidation Engine** - Tự động liquidate khi margin ratio < 1.0
4. **Multi-signature Approval** - 2-of-3 cho các giao dịch quan trọng

### 1.2 Phase 2 Recap

| Component | Status |
|-----------|--------|
| Trade Intent (multi-asset swap) | ✅ |
| Fee Intent | ✅ |
| Digital Signatures (Ed25519) | ✅ |
| Async Event Bus | ✅ |
| Trade History Projection | ✅ |
| Currency enum | ✅ |
| 63 tests passing | ✅ |

### 1.3 Triết lý Phase 3

> **"Margin-First, Trading-Second"**

Phase 3 tập trung vào **Margin Logic** - phần khó nhất của accounting:
- Order matching chỉ là CLOB đơn giản
- Không có Market/Stop/OCO orders (Phase 4+)
- Không có Order Book UI (Phase 4+)

---

## 2. Account Structure (Mở rộng)

### 2.1 New Account Types

Phase 3 giới thiệu **Loan Account** - theo chuẩn kế toán ngân hàng:

```
ASSET:USER:ALICE:USDT:LOAN      # Khoản vay của User = Tài sản của BiBank
LIAB:USER:ALICE:USDT:AVAILABLE  # Số dư khả dụng (existing)
LIAB:USER:ALICE:USDT:LOCKED     # Số dư bị khóa trong orders (existing)

EQUITY:SYSTEM:INSURANCE:USDT:MAIN  # Insurance Fund
REV:SYSTEM:INTEREST:USDT:MARGIN    # Revenue từ lãi margin
```

### 2.2 Accounting Rationale

| Account | Category | Normal Balance | Ý nghĩa |
|---------|----------|----------------|---------|
| `ASSET:USER:*:*:LOAN` | Asset | Debit | Khoản phải thu từ User (BiBank's receivable) |
| `LIAB:USER:*:*:AVAILABLE` | Liability | Credit | Tiền BiBank nợ User |
| `EQUITY:SYSTEM:INSURANCE:*:*` | Equity | Credit | Quỹ bảo hiểm |
| `REV:SYSTEM:INTEREST:*:*` | Revenue | Credit | Thu nhập lãi |

### 2.3 User Equity Formula

$$\text{Equity} = \text{Available} + \text{Unrealized PnL} - \text{Loan}$$

$$\text{Margin Ratio} = \frac{\text{Equity}}{\text{Maintenance Margin}}$$

**Liquidation Trigger:** Margin Ratio < 1.0 (hoặc < 105% để có buffer)

---

## 3. Margin System Specification

### 3.1 Margin Type

**Decision: Cross-Margin Only (Phase 3)**

- Toàn bộ `AVAILABLE` balance là collateral
- Shared across all positions
- Isolated margin = Phase 4+

### 3.2 Leverage

| Asset Type | Max Leverage | Maintenance Margin |
|------------|--------------|-------------------|
| BTC, ETH | 10x | 5% |
| Major Alts | 5x | 10% |
| Stablecoins | 20x | 2.5% |

**Phase 3 Default:** 10x cho tất cả (configurable via RiskEngine)

### 3.3 Borrowing Flow

**Scenario:** Alice muốn mua 1 BTC (giá 100,000 USDT) với 10,000 USDT collateral (10x leverage)

```rust
// Step 1: Borrow 90,000 USDT
JournalEntry {
    intent: TransactionIntent::Borrow,
    postings: [
        // BiBank's receivable increases (Alice owes BiBank)
        Posting { account: "ASSET:USER:ALICE:USDT:LOAN", amount: 90000, side: Debit },
        // Alice's available balance increases
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 90000, side: Credit },
    ],
    metadata: {
        "leverage": "10",
        "collateral": "10000",
        "borrowed": "90000",
    }
}

// Step 2: Trade (existing Phase 2 flow)
JournalEntry {
    intent: TransactionIntent::Trade,
    postings: [
        // Alice pays 100,000 USDT
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 100000, side: Debit },
        // Counterparty receives USDT
        Posting { account: "LIAB:USER:BOB:USDT:AVAILABLE", amount: 100000, side: Credit },
        // Counterparty pays 1 BTC
        Posting { account: "LIAB:USER:BOB:BTC:AVAILABLE", amount: 1, side: Debit },
        // Alice receives 1 BTC
        Posting { account: "LIAB:USER:ALICE:BTC:AVAILABLE", amount: 1, side: Credit },
    ],
}
```

### 3.4 Repayment Flow

```rust
// Repay 90,000 USDT loan
JournalEntry {
    intent: TransactionIntent::Repay,
    postings: [
        // Alice pays from available
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 90000, side: Debit },
        // BiBank's receivable decreases
        Posting { account: "ASSET:USER:ALICE:USDT:LOAN", amount: 90000, side: Credit },
    ],
}
```

### 3.5 New Transaction Intents

```rust
pub enum TransactionIntent {
    // ... existing ...

    /// Borrow funds for margin trading
    Borrow,

    /// Repay borrowed funds
    Repay,

    /// Interest accrual on borrowed funds
    Interest,

    /// Forced liquidation
    Liquidation,

    /// Order placement (lock funds)
    OrderPlace,

    /// Order cancellation (unlock funds)
    OrderCancel,
}
```

---

## 4. Interest Accrual Specification

### 4.1 Interest Model

**Decision: Compound Interest (Nhập gốc)**

- Frequency: Daily accrual
- Method: Add to loan principal
- Rate: Configurable per asset (default 0.05% daily = ~18% APY)

### 4.2 Interest Entry

```rust
// Daily interest accrual for Alice's 90,000 USDT loan (0.05% = 45 USDT)
JournalEntry {
    intent: TransactionIntent::Interest,
    postings: [
        // Increase loan principal (compound)
        Posting { account: "ASSET:USER:ALICE:USDT:LOAN", amount: 45, side: Debit },
        // Revenue for BiBank
        Posting { account: "REV:SYSTEM:INTEREST:USDT:MARGIN", amount: 45, side: Credit },
    ],
    metadata: {
        "rate": "0.0005",
        "principal": "90000",
        "accrued": "45",
    }
}
```

### 4.3 Interest Invariants

| Rule | Description |
|------|-------------|
| No negative interest | Rate >= 0 |
| Daily batch | Accrue once per day, not per block |
| Compound | Interest adds to principal |
| On-close settlement | Full interest paid on position close |

---

## 5. Price Oracle Specification

### 5.1 Architecture

```rust
#[async_trait]
pub trait PriceOracle: Send + Sync {
    /// Get current price for a trading pair
    async fn get_price(&self, base: &str, quote: &str) -> Result<Decimal, OracleError>;

    /// Get mark price (for margin calculation)
    async fn get_mark_price(&self, base: &str, quote: &str) -> Result<Decimal, OracleError>;
}
```

### 5.2 Phase 3 Implementation

**MockOracle** - Controlled via CLI:

```bash
# Set BTC price to 100,000 USDT
bibank oracle set BTC USDT 100000

# Get current price
bibank oracle get BTC USDT
```

```rust
pub struct MockOracle {
    prices: RwLock<HashMap<(String, String), Decimal>>,
}

impl PriceOracle for MockOracle {
    async fn get_price(&self, base: &str, quote: &str) -> Result<Decimal, OracleError> {
        self.prices.read().await
            .get(&(base.to_string(), quote.to_string()))
            .copied()
            .ok_or(OracleError::PriceNotFound)
    }
}
```

### 5.3 Oracle Invariants

| Rule | Description |
|------|-------------|
| **Ledger không gọi Oracle** | Chỉ RiskEngine gọi |
| **Fail-closed** | Oracle failure → trading halt |
| **Deterministic** | Mock prices fixed until changed |
| **Replay-friendly** | Prices stored in metadata |

### 5.4 Price Types

| Type | Usage | Source |
|------|-------|--------|
| **Last Price** | UI display | Last trade |
| **Mark Price** | Margin calculation | Oracle |
| **Index Price** | Liquidation trigger | Oracle (Phase 3.1: median of multiple sources) |

---

## 6. Order Matching Engine Specification

### 6.1 Order Book Structure

```rust
pub struct OrderBook {
    pub pair: TradingPair,
    pub bids: BTreeMap<Price, VecDeque<Order>>,  // Price descending
    pub asks: BTreeMap<Price, VecDeque<Order>>,  // Price ascending
}

pub struct Order {
    pub id: OrderId,
    pub user_id: String,
    pub side: OrderSide,
    pub price: Decimal,
    pub quantity: Decimal,
    pub filled: Decimal,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
}

pub enum OrderSide {
    Buy,
    Sell,
}

pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
}
```

### 6.2 Matching Algorithm

**CLOB with Price-Time Priority:**

1. New order arrives
2. Match against opposite side (best price first, then time)
3. For each match:
   - Create Trade JournalEntry
   - Update order quantities
4. Remaining quantity stays in book (or reject if IOC)

### 6.3 Order Types (Phase 3)

| Type | Description | Phase |
|------|-------------|-------|
| **Limit GTC** | Good Till Cancelled | ✅ Phase 3 |
| Limit IOC | Immediate or Cancel | Phase 3.1 |
| Market | Execute at best price | Phase 3.1 |
| Stop-Limit | Trigger on price | Phase 4 |

### 6.4 Order Placement Flow

```rust
// Step 1: Lock funds (OrderPlace intent)
JournalEntry {
    intent: TransactionIntent::OrderPlace,
    postings: [
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 10000, side: Debit },
        Posting { account: "LIAB:USER:ALICE:USDT:LOCKED", amount: 10000, side: Credit },
    ],
    metadata: {
        "order_id": "ORD-001",
        "side": "buy",
        "price": "100000",
        "quantity": "0.1",
    }
}
```

### 6.5 Partial Fill Flow

**Key Decision:** Mỗi fill = 1 Trade JournalEntry

```rust
// Order: Buy 1 BTC @ 100,000 USDT
// Fill 1: 0.3 BTC matched
JournalEntry {
    intent: TransactionIntent::Trade,
    correlation_id: "ORD-001",  // Link to original order
    postings: [
        // USDT leg
        Posting { account: "LIAB:USER:ALICE:USDT:LOCKED", amount: 30000, side: Debit },
        Posting { account: "LIAB:USER:BOB:USDT:AVAILABLE", amount: 30000, side: Credit },
        // BTC leg
        Posting { account: "LIAB:USER:BOB:BTC:AVAILABLE", amount: 0.3, side: Debit },
        Posting { account: "LIAB:USER:ALICE:BTC:AVAILABLE", amount: 0.3, side: Credit },
    ],
    metadata: {
        "order_id": "ORD-001",
        "fill_id": "FILL-001",
        "fill_quantity": "0.3",
        "remaining": "0.7",
    }
}

// Fill 2: 0.7 BTC matched (order complete)
JournalEntry {
    intent: TransactionIntent::Trade,
    correlation_id: "ORD-001",
    // ... similar structure ...
}
```

### 6.6 Order Cancellation

```rust
// Cancel unfilled portion
JournalEntry {
    intent: TransactionIntent::OrderCancel,
    postings: [
        // Unlock remaining funds
        Posting { account: "LIAB:USER:ALICE:USDT:LOCKED", amount: 70000, side: Debit },
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 70000, side: Credit },
    ],
    metadata: {
        "order_id": "ORD-001",
        "cancelled_quantity": "0.7",
    }
}
```

### 6.7 Order State

**Decision: Order state lives in Projection, NOT Ledger**

```sql
-- orders projection table
CREATE TABLE orders (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    pair TEXT NOT NULL,
    side TEXT NOT NULL,
    price TEXT NOT NULL,
    quantity TEXT NOT NULL,
    filled TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Ledger chỉ biết:
- `OrderPlace` → funds locked
- `Trade` → fill occurred
- `OrderCancel` → funds unlocked

---

## 7. Liquidation Engine Specification

### 7.1 Trigger Conditions

```rust
pub struct LiquidationCheck {
    pub user_id: String,
    pub equity: Decimal,
    pub maintenance_margin: Decimal,
    pub margin_ratio: Decimal,
    pub should_liquidate: bool,
}

impl RiskEngine {
    pub fn check_liquidation(&self, user_id: &str, oracle: &dyn PriceOracle) -> LiquidationCheck {
        let available = self.get_available(user_id);
        let loan = self.get_loan(user_id);
        let unrealized_pnl = self.calculate_pnl(user_id, oracle);

        let equity = available + unrealized_pnl - loan;
        let maintenance_margin = loan * MAINTENANCE_MARGIN_RATE;  // 5%
        let margin_ratio = equity / maintenance_margin;

        LiquidationCheck {
            user_id: user_id.to_string(),
            equity,
            maintenance_margin,
            margin_ratio,
            should_liquidate: margin_ratio < Decimal::ONE,
        }
    }
}
```

### 7.2 Liquidation Flow

```
1. RiskEngine detects margin_ratio < 1.0
2. Cancel all open orders (unlock funds)
3. Create Liquidation Trade (market close position)
4. Calculate PnL
5. If negative PnL > equity:
   a. Debit remaining equity from user
   b. Debit shortfall from Insurance Fund
6. Settle loan
7. Charge liquidation fee (5% of position)
```

### 7.3 Liquidation Entry

```rust
// Liquidation: Alice's 1 BTC position force-closed at 95,000 USDT
// Original: Bought at 100,000 with 10,000 collateral, 90,000 borrowed
// PnL: -5,000 USDT

JournalEntry {
    intent: TransactionIntent::Liquidation,
    postings: [
        // Close position: Sell 1 BTC
        Posting { account: "LIAB:USER:ALICE:BTC:AVAILABLE", amount: 1, side: Debit },
        Posting { account: "LIAB:SYSTEM:LIQUIDATOR:BTC:MAIN", amount: 1, side: Credit },

        // Receive 95,000 USDT
        Posting { account: "LIAB:SYSTEM:LIQUIDATOR:USDT:MAIN", amount: 95000, side: Debit },
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 95000, side: Credit },
    ],
    metadata: {
        "liquidation_id": "LIQ-001",
        "position_size": "1",
        "liquidation_price": "95000",
        "pnl": "-5000",
    }
}

// Repay loan + settle loss
JournalEntry {
    intent: TransactionIntent::Repay,
    postings: [
        // Pay back 90,000 loan
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 90000, side: Debit },
        Posting { account: "ASSET:USER:ALICE:USDT:LOAN", amount: 90000, side: Credit },
    ],
}

// Alice's remaining: 95,000 - 90,000 = 5,000 USDT (original 10,000 - 5,000 loss)

// Liquidation fee (5% of position value = 4,750 USDT)
JournalEntry {
    intent: TransactionIntent::Fee,
    postings: [
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 4750, side: Debit },
        Posting { account: "EQUITY:SYSTEM:INSURANCE:USDT:MAIN", amount: 4750, side: Credit },
    ],
    metadata: {
        "fee_type": "liquidation",
        "related_liquidation": "LIQ-001",
    }
}
```

### 7.4 Insurance Fund Usage

```rust
// Scenario: Alice's position liquidated with negative equity
// Liquidation price: 85,000 (worse than expected)
// Shortfall: 5,000 USDT

JournalEntry {
    intent: TransactionIntent::Adjustment,
    postings: [
        // Insurance Fund covers shortfall
        Posting { account: "EQUITY:SYSTEM:INSURANCE:USDT:MAIN", amount: 5000, side: Debit },
        // Clear Alice's remaining loan
        Posting { account: "ASSET:USER:ALICE:USDT:LOAN", amount: 5000, side: Credit },
    ],
    metadata: {
        "reason": "insurance_fund_coverage",
        "liquidation_id": "LIQ-001",
        "shortfall": "5000",
    }
}
```

### 7.5 Liquidation Invariants

| Rule | Description |
|------|-------------|
| **Cancel orders first** | Free up locked funds before liquidation |
| **Market close** | Liquidate at current mark price |
| **Insurance priority** | Insurance Fund absorbs losses after user equity exhausted |
| **No socialized loss** | Phase 3: No ADL, no clawback from other users |
| **Liquidation fee** | 5% of position value → Insurance Fund |

---

## 8. Multi-Signature Approval Specification

### 8.1 Scope

Operations requiring multi-sig:
- `Adjustment` intent
- Withdrawals > threshold (e.g., 100,000 USDT)
- System parameter changes

### 8.2 Approval Flow

```
1. Operator creates UnsignedEntry
2. Entry stored in pending_approvals (SQLite)
3. Other operators sign
4. When N signatures collected (N-of-M):
   a. Assemble full JournalEntry with all signatures
   b. Submit to Ledger
5. If expired → auto-reject (no ledger entry)
```

### 8.3 Data Structures

```rust
pub struct PendingApproval {
    pub id: String,
    pub unsigned_entry: UnsignedEntry,
    pub unsigned_entry_hash: String,
    pub required_signatures: u8,
    pub collected_signatures: Vec<EntrySignature>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: ApprovalStatus,
}

pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}
```

### 8.4 SQLite Schema

```sql
CREATE TABLE pending_approvals (
    id TEXT PRIMARY KEY,
    unsigned_entry_json TEXT NOT NULL,
    unsigned_entry_hash TEXT NOT NULL,
    required_signatures INTEGER NOT NULL,
    collected_signatures_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    status TEXT NOT NULL
);
```

### 8.5 CLI Commands

```bash
# Create pending approval
bibank approve create --entry-file adjustment.json --required 2

# List pending approvals
bibank approve list

# Sign pending approval
bibank approve sign --id APPR-001 --key-file operator.key

# Check status
bibank approve status --id APPR-001
```

### 8.6 Multi-sig Invariants

| Rule | Description |
|------|-------------|
| **2-of-3 default** | SYSTEM + 2 operators |
| **Pending ≠ Ledger** | Pending state in SQLite only |
| **Expiry = 24h** | Auto-reject after timeout |
| **Reject silent** | Rejected approvals don't create ledger entries |
| **Signatures immutable** | Cannot remove signature once added |

---

## 9. Risk Engine Upgrades

### 9.1 New Checks

```rust
impl RiskEngine {
    /// Pre-trade margin check
    pub fn check_margin_order(&self, order: &Order, oracle: &dyn PriceOracle) -> Result<(), RiskError> {
        let required_margin = order.notional_value() / order.leverage;
        let available = self.get_available(&order.user_id);

        if available < required_margin {
            return Err(RiskError::InsufficientMargin);
        }
        Ok(())
    }

    /// Real-time PnL calculation
    pub fn calculate_pnl(&self, user_id: &str, oracle: &dyn PriceOracle) -> Decimal {
        let positions = self.get_positions(user_id);
        positions.iter().map(|p| {
            let current_price = oracle.get_mark_price(&p.asset, "USDT").unwrap();
            (current_price - p.entry_price) * p.quantity
        }).sum()
    }

    /// Liquidation monitor
    pub fn check_all_liquidations(&self, oracle: &dyn PriceOracle) -> Vec<LiquidationCheck> {
        self.all_users_with_loans()
            .map(|user_id| self.check_liquidation(&user_id, oracle))
            .filter(|check| check.should_liquidate)
            .collect()
    }
}
```

### 9.2 Risk Parameters

```rust
pub struct RiskParameters {
    /// Maximum leverage per asset
    pub max_leverage: HashMap<String, Decimal>,

    /// Maintenance margin rate (default 5%)
    pub maintenance_margin_rate: Decimal,

    /// Liquidation buffer (trigger at 105% instead of 100%)
    pub liquidation_buffer: Decimal,

    /// Liquidation fee rate (5%)
    pub liquidation_fee_rate: Decimal,

    /// Daily interest rate (0.05%)
    pub daily_interest_rate: Decimal,

    /// Max position size per user
    pub max_position_size: HashMap<String, Decimal>,
}
```

---

## 10. Updated Workspace Structure

```
bibank/
├── crates/
│   ├── core/
│   │   ├── amount.rs
│   │   └── currency.rs
│   │
│   ├── ledger/
│   │   ├── account.rs
│   │   ├── entry.rs
│   │   ├── hash.rs
│   │   ├── signature.rs
│   │   ├── validation.rs
│   │   └── intent/                 # NEW: Intent-specific validation
│   │       ├── mod.rs
│   │       ├── borrow.rs
│   │       ├── repay.rs
│   │       ├── interest.rs
│   │       ├── liquidation.rs
│   │       └── order.rs
│   │
│   ├── risk/
│   │   ├── engine.rs
│   │   ├── state.rs
│   │   ├── margin.rs              # NEW: Margin calculations
│   │   ├── liquidation.rs         # NEW: Liquidation logic
│   │   └── parameters.rs          # NEW: Risk parameters
│   │
│   ├── oracle/                    # NEW CRATE
│   │   ├── lib.rs
│   │   ├── trait.rs               # PriceOracle trait
│   │   ├── mock.rs                # MockOracle implementation
│   │   └── error.rs
│   │
│   ├── matching/                  # NEW CRATE
│   │   ├── lib.rs
│   │   ├── orderbook.rs           # OrderBook structure
│   │   ├── engine.rs              # Matching engine
│   │   ├── order.rs               # Order types
│   │   └── fill.rs                # Fill generation
│   │
│   ├── approval/                  # NEW CRATE
│   │   ├── lib.rs
│   │   ├── pending.rs             # PendingApproval
│   │   ├── store.rs               # SQLite storage
│   │   └── workflow.rs            # Approval workflow
│   │
│   ├── events/
│   ├── bus/
│   │
│   ├── projection/
│   │   ├── balance.rs
│   │   ├── trade.rs
│   │   ├── order.rs               # NEW: Order projection
│   │   └── position.rs            # NEW: Position projection
│   │
│   ├── rpc/
│   │   ├── commands.rs            # UPDATED: margin commands
│   │   └── main.rs
│   │
│   └── dsl/
│
└── data/
    ├── journal/
    ├── projection.db
    └── keys/
```

---

## 11. New Dependencies

```toml
[workspace.dependencies]
# Existing...

# Phase 3 additions (nếu cần)
# Không thêm dependency mới nếu có thể
# MockOracle không cần external API client
```

---

## 12. CLI Commands (Phase 3)

```bash
# === Margin Trading ===
bibank margin borrow ALICE 90000 USDT --leverage 10
bibank margin repay ALICE 90000 USDT
bibank margin status ALICE

# === Orders ===
bibank order place ALICE buy BTC/USDT --price 100000 --quantity 0.1
bibank order cancel ALICE ORD-001
bibank order list ALICE
bibank orderbook BTC/USDT

# === Oracle ===
bibank oracle set BTC USDT 100000
bibank oracle get BTC USDT

# === Liquidation ===
bibank liquidation check ALICE
bibank liquidation list
bibank liquidation execute LIQ-001  # Manual trigger (for testing)

# === Multi-sig Approval ===
bibank approve create --entry-file adjustment.json --required 2
bibank approve list
bibank approve sign --id APPR-001 --key-file operator.key
bibank approve status APPR-001

# === Interest ===
bibank interest accrue  # Daily batch (cron job)
bibank interest status ALICE

# === Existing (unchanged) ===
bibank init
bibank deposit ALICE 10000 USDT
bibank trade ALICE BOB --sell 100 --sell-asset USDT --buy 0.001 --buy-asset BTC
bibank balance ALICE
bibank trades
bibank audit --verify-signatures
bibank replay --reset
```

---

## 13. Validation Matrix (Updated)

| Intent | Min Postings | Allowed Categories | Special Rules |
|--------|-------------|-------------------|---------------|
| `Genesis` | 2 | ASSET, EQUITY | sequence = 1 |
| `Deposit` | 2 | ASSET ↑, LIAB ↑ | - |
| `Withdrawal` | 2 | ASSET ↓, LIAB ↓ | Risk check |
| `Transfer` | 2 | LIAB only | Risk check |
| `Trade` | 4+ | LIAB only | 2 assets, risk check |
| `Fee` | 2 | LIAB ↓, REV/EQUITY ↑ | Risk check |
| **`Borrow`** | 2 | ASSET:LOAN ↑, LIAB:AVAIL ↑ | Margin check |
| **`Repay`** | 2 | LIAB:AVAIL ↓, ASSET:LOAN ↓ | Balance check |
| **`Interest`** | 2 | ASSET:LOAN ↑, REV:INTEREST ↑ | Daily batch |
| **`Liquidation`** | 4+ | Multiple | Margin ratio < 1.0 |
| **`OrderPlace`** | 2 | LIAB:AVAIL ↓, LIAB:LOCKED ↑ | Margin check |
| **`OrderCancel`** | 2 | LIAB:LOCKED ↓, LIAB:AVAIL ↑ | - |
| `Adjustment` | 2 | Any | `requires_approval = true` |

---

## 14. Phase 3 Deliverables

### 14.1 In Scope

| Component | Deliverable |
|-----------|-------------|
| Margin System | Cross-margin, 10x leverage, borrow/repay |
| Interest | Daily accrual, compound |
| Price Oracle | MockOracle with trait interface |
| Order Matching | CLOB, limit orders, partial fills |
| Liquidation | Auto-liquidate, insurance fund |
| Multi-sig | 2-of-3 approval workflow |
| Projections | Order, Position projections |
| CLI | margin, order, oracle, liquidation, approve commands |
| Tests | Margin tests, matching tests, liquidation tests |

### 14.2 Out of Scope (Phase 4+)

| Component | Phase |
|-----------|-------|
| Isolated margin | Phase 4 |
| Market/Stop orders | Phase 3.1 |
| External oracle | Phase 3.1 |
| ADL (Auto-Deleverage) | Phase 4 |
| Socialized loss | Phase 4 |
| Funding rate | Phase 4 |
| Perpetual contracts | Phase 4 |
| Cross-margin with multiple assets | Phase 4 |

### 14.3 Success Criteria

- [ ] Borrow/Repay flow works correctly
- [ ] Interest accrues daily and compounds
- [ ] Order matching produces correct Trade entries
- [ ] Partial fills link to original order via correlation_id
- [ ] Liquidation triggers at margin ratio < 1.0
- [ ] Insurance Fund absorbs losses beyond user equity
- [ ] Multi-sig 2-of-3 works for Adjustment
- [ ] MockOracle provides deterministic prices
- [ ] All existing tests still pass
- [ ] 80+ tests passing

---

## 15. Timeline

| Week | Deliverable |
|------|-------------|
| 1-2 | Account structure (LOAN) + Borrow/Repay intents |
| 3-4 | Interest accrual + PriceOracle trait + MockOracle |
| 5-6 | RiskEngine margin checks + pre-trade validation |
| 7-8 | OrderBook + Matching engine + partial fills |
| 9-10 | Liquidation engine + Insurance Fund |
| 11-12 | Multi-sig approval module |

**Total: ~12 weeks**

---

## 16. Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Margin math errors | Critical | Extensive unit tests, fuzzing |
| Liquidation cascade | High | Insurance Fund, position limits |
| Oracle manipulation | High | Multiple sources (Phase 3.1), circuit breakers |
| Order matching bugs | Medium | Deterministic matching, replay tests |
| Interest calculation drift | Low | Daily batch with audit trail |

---

## 17. Design Decisions (Consensus)

| # | Question | Decision | Rationale |
|---|----------|----------|----------|
| 1 | Loan account category | **ASSET:USER:*:*:LOAN** | Khoản vay = Tài sản của BiBank (kế toán chuẩn) |
| 2 | Interest method | **Compound (nhập gốc)** | Industry standard cho margin trading |
| 3 | Oracle implementation | **Trait + MockOracle** | Testable, no external deps Phase 3 |
| 4 | Order types | **Limit GTC only** | Đơn giản, mở rộng sau |
| 5 | Margin type | **Cross-margin only** | Single equity calculation |
| 6 | Partial fill tracking | **1 Trade entry per fill** | Audit-friendly, replay-correct |
| 7 | Order state storage | **Projection (SQLite)** | Ledger = facts, not state |
| 8 | Multi-sig pending | **SQLite, not JSONL** | Pending ≠ committed |
| 9 | Liquidation fee | **5% → Insurance Fund** | Standard practice |
| 10 | Socialized loss | **Out of scope** | Phase 4, avoid complexity |

> ✅ **Consensus reached:** GPT5 ✅ | Gemini3 ✅ | Author ✅ (2026-01-26)

---

> **Document Status:** 🔒 LOCKED
> **Consensus:** GPT5 ✅ | Gemini3 ✅ | Author ✅
> **Next Step:** Implementation
> **Estimated Duration:** 12 weeks
