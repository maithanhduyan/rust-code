# BiBank - Phase 1 Specification

> **Document Version:** 1.0
> **Date:** 2026-01-25
> **Status:** ✅ DESIGN LOCKED - Ready for Implementation
> **Author:** Team BiBank
> **Reviewed by:** GPT5, Gemini3

---

## 1. Tổng quan

### 1.1 Tầm nhìn

**BiBank** là **Financial State OS** - không phải app ngân hàng truyền thống.

Nguyên tắc nền tảng:
- **Ledger là nguồn sự thật duy nhất** - Không DB nào được "sửa state"
- **Không reconcile** - Nếu cần reconcile → kiến trúc sai
- **Correct-by-construction** - Risk engine chặn ngay tại write-time
- **Event-first, snapshot-second** - Snapshot chỉ để tối ưu đọc

### 1.2 Kiến trúc tổng quan

```
           ┌────────────┐
           │ Risk Engine│ (Pre-commit Gatekeeper)
           └─────▲──────┘
                 │
Client ──▶ Ledger ──▶ Event Bus ──▶ Projections
                 │
                 ▼
           Audit / Replay
```

---

## 2. Quyết định thiết kế đã chốt (14 Pillars)

| # | Hạng mục | Quyết định kỹ thuật |
|---|----------|---------------------|
| 1 | **Event Store** | JSONL (Source of Truth) + SQLite (Disposable Read Model) |
| 2 | **Accounting** | Double-Entry (Zero-sum per asset group) |
| 3 | **Direction** | `Side::Debit` / `Side::Credit` (Explicit Enum) |
| 4 | **Structure** | Multi-asset per Entry (Atomic Trade) |
| 5 | **Security** | Hash Chain (SHA256 linking prev_hash) |
| 6 | **Ordering** | Derived Sequence (Bootstrapped from file tail) |
| 7 | **Validation** | Risk Engine (Pre-commit Gatekeeper) |
| 8 | **Account Model** | Hierarchical 5-part (`CAT:SEGMENT:ID:ASSET:TYPE`) |
| 9 | **Naming** | SCREAMING_SNAKE_CASE (`LIAB:USER:ALICE:USD:AVAILABLE`) |
| 10 | **Categories** | 5 Standard Types (`Asset`, `Liability`, `Equity`, `Revenue`, `Expense`) |
| 11 | **Intent** | 7 Values (`Genesis`, `Deposit`, `Withdrawal`, `Transfer`, `Trade`, `Fee`, `Adjustment`) |
| 12 | **Tracing** | Dual IDs: `causality_id` (Logic chain) + `correlation_id` (Request trace) |
| 13 | **Risk State** | In-Memory (Rebuilt via Event Replay on startup) |
| 14 | **Workspace** | 8 Crates (`core`, `ledger`, `risk`, `events`, `bus`, `projection`, `rpc`, `dsl`) |

---

## 3. Ledger Account Specification

### 3.1 Account Key Grammar

```
AccountKey ::= CATEGORY ":" SEGMENT ":" ID ":" ASSET ":" SUB_ACCOUNT

CATEGORY    ::= "ASSET" | "LIAB" | "EQUITY" | "REV" | "EXP"
SEGMENT     ::= "USER" | "SYSTEM"
ID          ::= SCREAMING_SNAKE_CASE_STRING
ASSET       ::= CURRENCY_CODE
SUB_ACCOUNT ::= "AVAILABLE" | "LOCKED" | "MAIN" | "VAULT" | "REVENUE" | ...
```

### 3.2 Account Category

```rust
pub enum AccountCategory {
    Asset,      // Tài sản của hệ thống (Cash, Crypto vault)
    Liability,  // Nợ phải trả (User balances - tiền user gửi)
    Equity,     // Vốn chủ sở hữu
    Revenue,    // Doanh thu (Fees collected)
    Expense,    // Chi phí
}

impl AccountCategory {
    /// Normal balance side (Tăng bên nào?)
    pub fn normal_balance(&self) -> Side {
        match self {
            Asset | Expense => Side::Debit,
            Liability | Equity | Revenue => Side::Credit,
        }
    }
}
```

### 3.3 Account Examples

| Account Key | Mô tả |
|-------------|-------|
| `ASSET:SYSTEM:VAULT:USDT:MAIN` | Kho USDT của hệ thống |
| `ASSET:SYSTEM:VAULT:BTC:MAIN` | Kho BTC của hệ thống |
| `LIAB:USER:ALICE:USDT:AVAILABLE` | Số dư USDT khả dụng của Alice |
| `LIAB:USER:ALICE:USDT:LOCKED` | Số dư USDT bị khóa của Alice |
| `LIAB:USER:BOB:BTC:AVAILABLE` | Số dư BTC khả dụng của Bob |
| `REV:SYSTEM:FEE:USDT:REVENUE` | Doanh thu phí giao dịch USDT |
| `REV:SYSTEM:FEE:BTC:REVENUE` | Doanh thu phí giao dịch BTC |
| `EXP:SYSTEM:SALARY:USD:MAIN` | Chi phí lương nhân viên |

### 3.4 Account Key Struct

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountKey {
    pub category: AccountCategory,
    pub segment: String,      // "USER", "SYSTEM"
    pub id: String,           // "ALICE", "VAULT"
    pub asset: String,        // "USDT", "BTC"
    pub sub_account: String,  // "AVAILABLE", "LOCKED"
}

impl Display for AccountKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}:{}:{}",
            self.category, self.segment, self.id, self.asset, self.sub_account)
    }
}

impl FromStr for AccountKey {
    type Err = LedgerError;
    // Parse "LIAB:USER:ALICE:USDT:AVAILABLE" → AccountKey
}
```

---

## 4. Journal Entry Specification

### 4.1 Core Structs

```rust
/// Transaction Intent - Financial primitive (NOT workflow)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionIntent {
    Genesis,      // System initialization
    Deposit,      // External money entering system
    Withdrawal,   // External money leaving system
    Transfer,     // Internal transfer between accounts
    Trade,        // Exchange between assets
    Fee,          // Fee collection
    Adjustment,   // Manual adjustment (audit-heavy, requires approval)
}

/// Posting Side - Explicit Debit/Credit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Debit,
    Credit,
}

/// Amount - Non-negative wrapper around Decimal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Amount(Decimal);  // Validated non-negative

/// Single Posting in a Journal Entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    pub account: AccountKey,
    pub amount: Amount,
    pub side: Side,
}

/// Journal Entry - The atomic unit of financial state change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    // === Ordering & Integrity ===
    pub sequence: u64,
    pub prev_hash: String,
    pub hash: String,
    pub timestamp: DateTime<Utc>,

    // === Semantics ===
    pub intent: TransactionIntent,

    // === Tracing ===
    pub correlation_id: String,         // Request UUID (from API/CLI)
    pub causality_id: Option<String>,   // Parent entry that caused this

    // === Financial Data ===
    pub postings: Vec<Posting>,

    // === Metadata ===
    pub metadata: HashMap<String, Value>,
}
```

### 4.2 Validation Rules

```rust
impl JournalEntry {
    /// MUST: Entry has at least 2 postings (double-entry)
    /// MUST: Zero-sum per asset group
    /// MUST: All amounts are positive
    /// MUST: Valid account keys
    pub fn validate(&self) -> Result<(), LedgerError> {
        // Rule 1: At least 2 postings
        if self.postings.len() < 2 {
            return Err(LedgerError::InsufficientPostings);
        }

        // Rule 2: Zero-sum per asset
        let mut sums: HashMap<String, Decimal> = HashMap::new();
        for posting in &self.postings {
            let asset = &posting.account.asset;
            let delta = match posting.side {
                Side::Debit => posting.amount.value(),
                Side::Credit => -posting.amount.value(),
            };
            *sums.entry(asset.clone()).or_default() += delta;
        }

        for (asset, sum) in sums {
            if !sum.is_zero() {
                return Err(LedgerError::UnbalancedEntry { asset, imbalance: sum });
            }
        }

        Ok(())
    }
}
```

---

## 5. Invariant List

### 5.1 Ledger MUST

| # | Invariant | Enforcement |
|---|-----------|-------------|
| 1 | Every entry has `sequence` strictly increasing | Derived from JSONL line count |
| 2 | Every entry has valid `prev_hash` | SHA256 of previous entry |
| 3 | Every entry is double-entry balanced per asset | `validate()` before append |
| 4 | Every entry has non-empty `correlation_id` | Constructor enforced |
| 5 | Every amount is non-negative | `Amount` type enforced |
| 6 | Genesis entry has sequence = 1 and prev_hash = "GENESIS" | Bootstrap check |
| 7 | Ledger is append-only | No update/delete API |

### 5.2 Ledger MUST NOT

| # | Forbidden | Reason |
|---|-----------|--------|
| 1 | Edit or delete existing entries | Append-only principle |
| 2 | Allow unbalanced entries to commit | Double-entry integrity |
| 3 | Generate `correlation_id` internally | Separation of concerns |
| 4 | Skip sequence numbers | Deterministic ordering |
| 5 | Accept negative amounts | Amount type constraint |
| 6 | Have multiple entries with same sequence | Uniqueness |

### 5.3 Risk Engine MUST

| # | Invariant | Description |
|---|-----------|-------------|
| 1 | Check BEFORE ledger commit | Pre-commit gatekeeper |
| 2 | NOT read from SQLite projection | Only in-memory state |
| 3 | Bootstrap from ledger replay | Rebuild state on startup |
| 4 | Reject insufficient balance | Balance >= 0 after posting |
| 5 | Be deterministic | Same input → same output |

---

## 6. Transaction Intent Validation Matrix

| Intent | Min Postings | Allowed Categories | Special Rules |
|--------|-------------|-------------------|---------------|
| `Genesis` | 2 | ASSET, LIAB, EQUITY | sequence = 1, prev_hash = "GENESIS" |
| `Deposit` | 2 | ASSET ↑, LIAB ↑ | External source |
| `Withdrawal` | 2 | ASSET ↓, LIAB ↓ | Risk check: balance >= amount |
| `Transfer` | 2 | LIAB only | Internal between users |
| `Trade` | 4 | LIAB only | Multi-asset, zero-sum per asset |
| `Fee` | 2 | LIAB ↓, REV ↑ | Fee collection |
| `Adjustment` | 2 | Any | `requires_approval = true` in metadata |

### 6.1 Example: Deposit 100 USDT to Alice

```rust
JournalEntry {
    sequence: 42,
    intent: TransactionIntent::Deposit,
    postings: [
        Posting { account: "ASSET:SYSTEM:VAULT:USDT:MAIN", amount: 100, side: Debit },
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 100, side: Credit },
    ],
    // USDT: Debit 100 - Credit 100 = 0 ✅
}
```

### 6.2 Example: Trade (Alice sells 100 USDT for 0.001 BTC from Bob)

```rust
JournalEntry {
    sequence: 43,
    intent: TransactionIntent::Trade,
    postings: [
        // USDT leg
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 100, side: Debit },
        Posting { account: "LIAB:USER:BOB:USDT:AVAILABLE", amount: 100, side: Credit },
        // BTC leg
        Posting { account: "LIAB:USER:BOB:BTC:AVAILABLE", amount: 0.001, side: Debit },
        Posting { account: "LIAB:USER:ALICE:BTC:AVAILABLE", amount: 0.001, side: Credit },
    ],
    // USDT: Debit 100 - Credit 100 = 0 ✅
    // BTC: Debit 0.001 - Credit 0.001 = 0 ✅
}
```

---

## 7. Workspace Structure (8 Crates)

```
bibank/
├── Cargo.toml                    # Workspace root
├── README.md
│
├── crates/
│   ├── core/                     # Domain types (Amount, Currency)
│   │   └── src/
│   │       ├── lib.rs
│   │       └── amount.rs
│   │
│   ├── ledger/                   # [HEART] Double-entry core
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── account.rs        # AccountKey, AccountCategory
│   │       ├── entry.rs          # JournalEntry, Posting, Side
│   │       ├── store.rs          # LedgerStore trait + JSONL impl
│   │       └── error.rs
│   │
│   ├── risk/                     # [GATEKEEPER] Pre-commit checks
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs         # RiskEngine trait
│   │       ├── state.rs          # In-memory balance state
│   │       └── rules.rs          # Balance invariants
│   │
│   ├── events/                   # Event definitions & JSONL I/O
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── store.rs          # Append-only JSONL writer
│   │       └── reader.rs         # Sequential reader
│   │
│   ├── bus/                      # In-process event distribution
│   │   └── src/
│   │       ├── lib.rs
│   │       └── channel.rs        # Pub/Sub for projections
│   │
│   ├── projection/               # Event → SQLite views
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs         # ProjectionEngine
│   │       ├── balance.rs        # Balance projection
│   │       └── replay.rs         # Rebuild from events
│   │
│   ├── rpc/                      # API/CLI orchestrator
│   │   └── src/
│   │       ├── lib.rs
│   │       └── commands.rs       # Deposit, Transfer, etc.
│   │
│   └── dsl/                      # DSL macros (future)
│       └── src/lib.rs
│
├── data/                         # Runtime data (gitignored)
│   ├── journal/
│   │   └── YYYY-MM-DD.jsonl
│   └── projection.db
│
└── docs/
    ├── IDEA.md
    └── proposed/
        └── Proposed-BiBank-Phase1.md
```

---

## 8. Crate Dependencies

```toml
[workspace.dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rust_decimal = { version = "1.33", features = ["serde-with-str", "maths"] }
thiserror = "2.0"
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }

# Crypto
sha2 = "0.10"
hex = "0.4"

# Utilities
strum = { version = "0.26", features = ["derive"] }
tokio = { version = "1.36", features = ["full"] }
tracing = "0.1"

# Projection
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono"] }
```

### Dependency Graph

```
bibank-core (no deps)
     ↓
bibank-ledger (core)
     ↓
bibank-risk (core, ledger)
     ↓
bibank-events (core, ledger)
     ↓
bibank-bus (events)
     ↓
bibank-projection (bus, sqlx)
     ↓
bibank-rpc (risk, projection)
     ↓
bibank-dsl (rpc)
```

---

## 9. Replay & Bootstrap Guarantees

### 9.1 Startup Sequence

```
1. Read JSONL files from data/journal/
2. Determine last sequence from tail of newest file
3. Replay all entries into RiskEngine in-memory state
4. (Optional) Rebuild SQLite projections
5. Ready to accept new transactions
```

### 9.2 Replay Invariant

```
∀ entries E1...En in JSONL:
    replay(E1...En) produces deterministic state S

    If projection.db is deleted:
        replay(E1...En) → projection.db is identical
```

### 9.3 Hash Chain Verification

```rust
fn verify_chain(entries: &[JournalEntry]) -> Result<()> {
    let mut prev_hash = "GENESIS".to_string();

    for entry in entries {
        if entry.prev_hash != prev_hash {
            return Err(LedgerError::BrokenHashChain);
        }
        prev_hash = entry.hash.clone();
    }
    Ok(())
}
```

---

## 10. Phase 1 Scope & Deliverables

### 10.1 In Scope

| Component | Deliverable |
|-----------|-------------|
| `bibank-core` | Amount, basic types |
| `bibank-ledger` | AccountKey, JournalEntry, Posting, validation |
| `bibank-events` | JSONL append/read |
| `bibank-risk` | Basic balance check (available >= 0) |
| `bibank-projection` | Balance projection from events |
| CLI | `bibank deposit`, `bibank transfer`, `bibank balance`, `bibank replay` |
| Intents | Genesis, Deposit, Withdrawal, Transfer |

### 10.2 Out of Scope (Phase 2+)

| Component | Phase |
|-----------|-------|
| Trade intent | Phase 2 |
| Fee intent | Phase 2 |
| Event Bus (pub/sub) | Phase 2 |
| Margin/Exposure checks | Phase 3 |
| Liquidation | Phase 3 |
| AML hooks | Phase 4 |
| Rule DSL | Phase 4 |
| Digital signatures | Phase 2+ |

### 10.3 Success Criteria (Phase 1 Complete)

- [ ] `bibank replay --reset` drops projection DB, replays all events, state is identical
- [ ] Double-entry validation rejects unbalanced entries at compile/runtime
- [ ] Risk engine rejects withdrawal when balance < amount
- [ ] Hash chain is verified on replay
- [ ] Sequence is derived from JSONL, no gaps after restart

---

## 11. File Formats

### 11.1 JSONL Entry Format

```json
{"sequence":1,"prev_hash":"GENESIS","hash":"abc123...","timestamp":"2026-01-25T10:00:00Z","intent":"genesis","correlation_id":"uuid-1","causality_id":null,"postings":[{"account":"ASSET:SYSTEM:VAULT:USDT:MAIN","amount":"1000000","side":"debit"},{"account":"EQUITY:SYSTEM:CAPITAL:USDT:MAIN","amount":"1000000","side":"credit"}],"metadata":{}}
{"sequence":2,"prev_hash":"abc123...","hash":"def456...","timestamp":"2026-01-25T10:01:00Z","intent":"deposit","correlation_id":"uuid-2","causality_id":null,"postings":[{"account":"ASSET:SYSTEM:VAULT:USDT:MAIN","amount":"100","side":"debit"},{"account":"LIAB:USER:ALICE:USDT:AVAILABLE","amount":"100","side":"credit"}],"metadata":{"source":"bank_transfer"}}
```

### 11.2 File Naming

```
data/journal/2026-01-25.jsonl
data/journal/2026-01-26.jsonl
```

---

## 12. CLI Commands (Phase 1)

```bash
# Initialize system with Genesis entry
bibank init

# Deposit
bibank deposit ALICE 100 USDT --correlation-id uuid-1

# Transfer
bibank transfer ALICE BOB 50 USDT --correlation-id uuid-2

# Withdrawal
bibank withdraw ALICE 30 USDT --correlation-id uuid-3

# Query balance (from projection)
bibank balance ALICE
bibank balance ALICE USDT

# Replay (rebuild projection from events)
bibank replay
bibank replay --reset  # Drop projection first

# Audit (verify hash chain)
bibank audit
bibank audit --from 2026-01-01 --to 2026-01-25
```

---

## 13. Ràng buộc bổ sung

### 13.1 `correlation_id`

- **Sinh bởi:** Outer layer (API/CLI/batch)
- **Ledger:** KHÔNG tự tạo
- **Bắt buộc:** Không empty

### 13.2 `causality_id`

- **Sinh bởi:** Ledger/orchestrator nội bộ
- **Client:** KHÔNG được truyền tùy ý
- **Dùng khi:** Entry B là hệ quả của Entry A

### 13.3 `Adjustment` Intent

- **Flag:** `metadata.requires_approval = true`
- **Audit:** Ghi lý do trong metadata
- **Sử dụng:** Cực kỳ hạn chế, cần review

---

> **Document Status:** ✅ DESIGN LOCKED
> **Next Step:** Implementation Step 1 - Workspace Setup + bibank-ledger
> **Từ thời điểm này, mọi thay đổi = breaking change, phải có lý do OS-level**