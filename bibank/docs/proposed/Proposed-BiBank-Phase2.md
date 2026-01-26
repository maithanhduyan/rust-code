# BiBank - Phase 2 Specification

> **Document Version:** 1.1
> **Date:** 2026-01-26
> **Status:** 🔒 LOCKED - Consensus Reached
> **Author:** Team BiBank
> **Depends on:** Phase 1 (Complete ✅)
> **Reviewed by:** GPT5, Gemini3 (Round 2)

---

## 1. Tổng quan Phase 2

### 1.1 Mục tiêu

Phase 2 mở rộng BiBank từ **basic ledger** thành **trading-ready financial OS**:

1. **Trade Intent** - Multi-asset atomic swap (Alice sells USDT for BTC)
2. **Fee Intent** - Fee collection với revenue tracking
3. **Event Bus** - Async pub/sub thay thế sync callbacks
4. **Digital Signatures** - Entry signing cho audit trail

### 1.2 Phase 1 Recap

| Component | Status |
|-----------|--------|
| Double-entry ledger | ✅ |
| Hash chain integrity | ✅ |
| Pre-commit Risk Engine | ✅ |
| JSONL Source of Truth | ✅ |
| SQLite Projections | ✅ |
| CLI (init, deposit, transfer, withdraw, balance, audit, replay) | ✅ |
| 31 tests passing | ✅ |

---

## 2. Trade Intent Specification

### 2.1 Mô tả

Trade là giao dịch **multi-asset atomic swap** giữa 2 users:
- Alice bán 100 USDT, nhận 0.001 BTC
- Bob bán 0.001 BTC, nhận 100 USDT

**Tất cả trong 1 JournalEntry duy nhất** - không có partial fill.

> **Phase 2 Scope:** Manual/OTC trades only - operator tạo trade với agreed price.
> **Phase 3:** Order book matching engine (auto-matching).

### 2.2 Postings Structure

```rust
JournalEntry {
    intent: TransactionIntent::Trade,
    postings: [
        // === USDT leg ===
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 100, side: Debit },
        Posting { account: "LIAB:USER:BOB:USDT:AVAILABLE", amount: 100, side: Credit },

        // === BTC leg ===
        Posting { account: "LIAB:USER:BOB:BTC:AVAILABLE", amount: 0.001, side: Debit },
        Posting { account: "LIAB:USER:ALICE:BTC:AVAILABLE", amount: 0.001, side: Credit },
    ],
    // Zero-sum per asset:
    // USDT: +100 - 100 = 0 ✅
    // BTC:  +0.001 - 0.001 = 0 ✅
}
```

### 2.3 Validation Rules

| Rule | Description |
|------|-------------|
| Min 4 postings | 2 legs × 2 sides |
| Exactly 2 assets | Trade between 2 different assets |
| Zero-sum per asset | Existing rule applies |
| LIAB accounts only | No ASSET/REV/EXP in trade |
| Risk check both users | Both must have sufficient balance |

### 2.4 Trade Metadata

```rust
metadata: {
    "trade_id": "TRADE-2026-001",
    "base_asset": "BTC",
    "quote_asset": "USDT",
    "price": "100000",           // 1 BTC = 100,000 USDT
    "base_amount": "0.001",
    "quote_amount": "100",
    "maker": "ALICE",
    "taker": "BOB",
}
```

### 2.5 CLI Command

```bash
# Trade: Alice sells 100 USDT for 0.001 BTC from Bob
bibank trade ALICE BOB --sell 100 USDT --buy 0.001 BTC

# Or with explicit maker/taker
bibank trade --maker ALICE --taker BOB --base BTC --quote USDT --price 100000 --amount 0.001
```

---

## 3. Fee Intent Specification

### 3.1 Mô tả

Fee là thu phí từ user vào revenue account của hệ thống:
- User trả phí (LIAB giảm)
- System thu phí (REV tăng)

**Fee Calculation Strategy:**
- **RPC Layer**: Tính percentage fee (e.g., 0.1% of trade amount)
- **Ledger Layer**: Nhận **absolute Amount** - không biết percentage
- Lý do: Ledger immutable, không cần biết logic tính phí

### 3.2 Postings Structure

```rust
JournalEntry {
    intent: TransactionIntent::Fee,
    postings: [
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 0.1, side: Debit },
        Posting { account: "REV:SYSTEM:FEE:USDT:TRADING", amount: 0.1, side: Credit },
    ],
}
```

### 3.3 Fee Types (Sub-accounts)

| Sub-account | Mô tả |
|-------------|-------|
| `TRADING` | Phí giao dịch spot |
| `WITHDRAWAL` | Phí rút tiền |
| `MARGIN` | Phí margin interest |
| `LIQUIDATION` | Phí liquidation |

### 3.4 Validation Rules

| Rule | Description |
|------|-------------|
| Min 2 postings | User → System |
| Debit from LIAB | User pays |
| Credit to REV | System receives |
| Same asset | Fee paid in same currency |
| Risk check | User must have balance >= fee |

### 3.5 Fee with Trade (Separate Entry)

**Decision:** Fee là **separate JournalEntry** với `intent: Fee`, không embed trong Trade entry.

Lý do:
- Trade giữ zero-sum invariant trong LIAB accounts
- Fee có invariant riêng (LIAB → REV)
- Dễ audit, query fees riêng biệt
- Trade và Fee được commit **atomically** trong cùng transaction

```rust
// Entry 1: Trade (zero-sum trong LIAB)
JournalEntry {
    intent: TransactionIntent::Trade,
    postings: [...],  // 4 postings, zero-sum per asset
}

// Entry 2: Fee (LIAB → REV)
JournalEntry {
    intent: TransactionIntent::Fee,
    postings: [
        Posting { account: "LIAB:USER:ALICE:USDT:AVAILABLE", amount: 0.1, side: Debit },
        Posting { account: "REV:SYSTEM:FEE:USDT:TRADING", amount: 0.1, side: Credit },
    ],
    metadata: {
        "fee_amount": "0.1",
        "fee_asset": "USDT",
        "related_trade_sequence": "42",
    }
}
```

### 3.6 CLI Commands

```bash
# Charge fee
bibank fee ALICE 0.1 USDT --type trading

# Fee với trade
bibank trade ALICE BOB --sell 100 USDT --buy 0.001 BTC --fee 0.1
```

---

## 4. Event Bus Specification

### 4.1 Mục tiêu

Chuyển từ **synchronous** sang **async pub/sub**:

```
Phase 1:
  Ledger.commit() → projection.apply() (sync, blocking)

Phase 2:
  Ledger.commit() → EventBus.publish() → [async subscribers]
```

### 4.2 Architecture

```
                          ┌─────────────────┐
                          │ Balance Projection │
                          └────────▲────────┘
                                   │
Ledger ──▶ EventBus ──────────────┼──────────▶ Risk State (rebuild)
                                   │
                          ┌────────▼────────┐
                          │ Trade Projection │
                          └─────────────────┘
```

### 4.3 Event Types

```rust
pub enum LedgerEvent {
    EntryCommitted {
        entry: JournalEntry,
        timestamp: DateTime<Utc>,
    },
    ChainVerified {
        last_sequence: u64,
        last_hash: String,
    },
    ReplayStarted {
        from_sequence: u64,
    },
    ReplayCompleted {
        entries_count: usize,
    },
}
```

### 4.4 Subscriber Trait

```rust
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Subscriber name for logging
    fn name(&self) -> &str;

    /// Handle a ledger event
    async fn handle(&self, event: &LedgerEvent) -> Result<(), SubscriberError>;

    /// Called on replay start (optional)
    async fn on_replay_start(&self) -> Result<(), SubscriberError> {
        Ok(())
    }

    /// Called on replay complete (optional)
    async fn on_replay_complete(&self) -> Result<(), SubscriberError> {
        Ok(())
    }
}
```

### 4.5 Event Bus Implementation

```rust
pub struct EventBus {
    subscribers: Vec<Arc<dyn EventSubscriber>>,
    buffer: tokio::sync::broadcast::Sender<LedgerEvent>,
}

impl EventBus {
    pub fn subscribe(&mut self, subscriber: Arc<dyn EventSubscriber>);

    pub async fn publish(&self, event: LedgerEvent) -> Result<(), BusError>;

    /// Replay từ offset
    pub async fn replay_from(&self, sequence: u64) -> Result<(), BusError>;
}
```

### 4.6 Guarantees

| Guarantee | Description |
|-----------|-------------|
| Ordered | Events delivered in sequence order |
| At-least-once | Subscriber may receive duplicates |
| Replayable | Can replay from any sequence |
| Non-blocking | Publisher không wait subscriber |
| **No Retention** | Bus không giữ events, chỉ broadcast |

**Replay Strategy:**
- Event Bus **không lưu trữ** events (memory-only broadcast)
- Replay **từ JSONL files** - Source of Truth
- Subscriber tracks `last_processed_sequence`
- On restart: `EventReader::read_from(last_sequence)` → re-publish to bus

### 4.7 Subscriber Failure Handling

```rust
// Subscriber failure KHÔNG ảnh hưởng ledger
// Subscriber tự track last processed sequence
// On restart: replay from last known sequence
```

---

## 5. Digital Signatures Specification

### 5.1 Mục tiêu

Mỗi JournalEntry được ký bởi:
1. **System key** - BiBank server signs
2. **Operator key** (optional) - Human approval for Adjustment

### 5.2 Signature Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySignature {
    pub signer_id: String,           // "SYSTEM" or operator ID
    pub algorithm: SignatureAlgorithm,
    pub public_key: String,          // hex-encoded
    pub signature: String,           // hex-encoded
    pub signed_at: DateTime<Utc>,
}

pub enum SignatureAlgorithm {
    Ed25519,
    Secp256k1,  // For future blockchain compatibility
}
```

### 5.3 Updated JournalEntry

```rust
pub struct JournalEntry {
    // ... existing fields ...

    /// Digital signatures (Phase 2)
    pub signatures: Vec<EntrySignature>,
}
```

### 5.4 Signing Flow

```
1. Build UnsignedEntry
2. Risk Check
3. Calculate hash (existing)
4. Sign hash with system key
5. Append to JSONL with signature
```

### 5.5 Verification

```rust
impl JournalEntry {
    pub fn verify_signatures(&self) -> Result<(), SignatureError> {
        for sig in &self.signatures {
            let payload = self.signable_payload(&sig.signed_at);
            verify_signature(&sig.public_key, &payload, &sig.signature)?;
        }
        Ok(())
    }

    /// Signable payload = 8 specific fields (deterministic order)
    fn signable_payload(&self, signed_at: &DateTime<Utc>) -> Vec<u8> {
        // Canonical JSON of these 8 fields:
        let signable = SignablePayload {
            sequence: self.sequence,
            timestamp: self.timestamp,
            intent: self.intent.clone(),
            postings: self.postings.clone(),
            metadata: self.metadata.clone(),
            prev_hash: self.prev_hash.clone(),
            hash: self.hash.clone(),
            signed_at: *signed_at,
        };
        serde_json::to_vec(&signable).unwrap()
    }
}

/// Explicit struct for signable fields - no ambiguity
#[derive(Serialize)]
struct SignablePayload {
    sequence: u64,
    timestamp: DateTime<Utc>,
    intent: TransactionIntent,
    postings: Vec<Posting>,
    metadata: HashMap<String, String>,
    prev_hash: String,
    hash: String,
    signed_at: DateTime<Utc>,
}
```

### 5.6 Key Management

| Key Type | Storage | Rotation | Usage |
|----------|---------|----------|-------|
| System key | **Environment variable** `BIBANK_SYSTEM_KEY` | Monthly | Auto-sign all entries |
| Operator keys | **File** `~/.bibank/keys/<operator_id>.key` | Per operator | Manual approval (Adjustment) |

**Rationale:**
- System key in env: Easy rotation, no file I/O, 12-factor app compliant
- Operator key in file: Portable, can be on USB/HSM, explicit load
- Production: System key should be in HSM, exposed via env var interface

### 5.7 CLI Integration

```bash
# Verify all signatures
bibank audit --verify-signatures

# Sign pending adjustment (requires operator key)
bibank sign --entry-sequence 42 --key-file operator.key
```

---

## 6. Updated Workspace Structure

```
bibank/
├── crates/
│   ├── core/
│   │   ├── amount.rs
│   │   └── currency.rs        # NEW: Currency enum
│   │
│   ├── ledger/
│   │   ├── account.rs
│   │   ├── entry.rs
│   │   ├── hash.rs
│   │   ├── signature.rs       # NEW: Digital signatures
│   │   └── trade.rs           # NEW: Trade validation
│   │
│   ├── risk/
│   │   ├── engine.rs
│   │   ├── state.rs
│   │   └── trade_check.rs     # NEW: Multi-user balance check
│   │
│   ├── events/
│   │   └── (unchanged)
│   │
│   ├── bus/
│   │   ├── lib.rs
│   │   ├── channel.rs         # UPDATED: Async broadcast
│   │   ├── subscriber.rs      # NEW: Subscriber trait
│   │   └── replay.rs          # NEW: Replay logic
│   │
│   ├── projection/
│   │   ├── balance.rs
│   │   └── trade.rs           # NEW: Trade history projection
│   │
│   ├── rpc/
│   │   ├── commands.rs        # UPDATED: trade, fee commands
│   │   └── main.rs
│   │
│   └── dsl/
│       └── (placeholder)
│
└── data/
    ├── journal/
    ├── projection.db
    └── keys/                   # NEW: Key storage
        └── system.key
```

---

## 7. New Dependencies

```toml
[workspace.dependencies]
# Existing...

# Phase 2 additions
ed25519-dalek = "2.1"          # Digital signatures
rand = "0.8"                    # Key generation
async-trait = "0.1"             # Async subscriber trait
tokio = { version = "1.36", features = ["full", "sync"] }  # broadcast channel
```

---

## 8. Migration Plan

### 8.1 Backward Compatibility

| Field | Handling |
|-------|----------|
| `signatures` | Optional, empty Vec for Phase 1 entries |
| New intents (Trade, Fee) | New entries only |
| Event bus | Opt-in, projection still works sync |

### 8.2 Migration Steps

1. Add `signatures: Vec<EntrySignature>` to `JournalEntry` (default empty)
2. Update serde to handle missing field
3. Replay existing entries (no signature = valid)
4. New entries get signed

---

## 9. Phase 2 Deliverables

### 9.1 In Scope

| Component | Deliverable |
|-----------|-------------|
| Trade Intent | Multi-asset atomic swap |
| Fee Intent | Fee collection to REV accounts |
| Event Bus | Async pub/sub with replay |
| Signatures | Ed25519 entry signing |
| CLI | `trade`, `fee`, `sign`, `audit --verify-signatures` |
| Projections | Trade history view |
| Tests | Trade tests, fee tests, bus tests, signature tests |

### 9.2 Out of Scope (Phase 3+)

| Component | Phase |
|-----------|-------|
| Order matching engine | Phase 3 |
| Margin/leverage | Phase 3 |
| Liquidation engine | Phase 3 |
| Multi-signature approval | Phase 3 |
| AML real-time hooks | Phase 4 |
| Rule DSL | Phase 4 |

### 9.3 Success Criteria

- [ ] Trade command creates valid 4+ posting entry
- [ ] Fee command deducts from user, credits to REV
- [ ] Event bus delivers events to all subscribers
- [ ] Subscriber failure doesn't block ledger commit
- [ ] All entries are signed with system key
- [ ] `audit --verify-signatures` passes
- [ ] Replay rebuilds state identically
- [ ] 50+ tests passing

---

## 10. CLI Commands (Phase 2)

```bash
# === Trade ===
bibank trade ALICE BOB --sell 100 USDT --buy 0.001 BTC
bibank trade ALICE BOB --sell 100 USDT --buy 0.001 BTC --fee 0.1

# === Fee ===
bibank fee ALICE 0.1 USDT --type trading
bibank fee ALICE 0.5 USDT --type withdrawal

# === Signature ===
bibank audit --verify-signatures
bibank keygen --output system.key
bibank sign --entry-sequence 42 --key-file operator.key

# === Existing (unchanged) ===
bibank init
bibank deposit ALICE 1000 USDT
bibank transfer ALICE BOB 100 USDT
bibank withdraw ALICE 50 USDT
bibank balance ALICE
bibank replay --reset
bibank audit
```

---

## 11. Validation Matrix (Updated)

| Intent | Min Postings | Allowed Categories | Special Rules |
|--------|-------------|-------------------|---------------|
| `Genesis` | 2 | ASSET, EQUITY | sequence = 1 |
| `Deposit` | 2 | ASSET ↑, LIAB ↑ | - |
| `Withdrawal` | 2 | ASSET ↓, LIAB ↓ | Risk check |
| `Transfer` | 2 | LIAB only | Risk check |
| **`Trade`** | 4+ | LIAB only | 2 assets, risk check both users |
| **`Fee`** | 2 | LIAB ↓, REV ↑ | Risk check |
| `Adjustment` | 2 | Any | `requires_approval = true` |

---

## 12. Timeline Estimate

| Week | Deliverable |
|------|-------------|
| 1 | Trade intent + validation + tests |
| 2 | Fee intent + validation + tests |
| 3 | Event bus async + subscriber trait |
| 4 | Digital signatures + verification |
| 5 | CLI commands + integration tests |
| 6 | Documentation + review |

**Total: ~6 weeks**

---

## 13. Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Signature key compromise | Critical | HSM, key rotation, audit logs |
| Event bus message loss | Medium | At-least-once, replay capability |
| Trade race condition | High | Single-threaded commit, Risk pre-check |
| Backward compatibility | Low | Optional fields, migration tested |

---

## 14. Design Decisions (Consensus)

| # | Question | Decision | Rationale |
|---|----------|----------|----------|
| 1 | Fee structure | **Percentage at RPC, Absolute at Ledger** | Ledger immutable, không cần biết % |
| 2 | Trade matching | **Phase 2: Manual/OTC only** | Auto-matching → Phase 3 |
| 3 | Key storage | **Env var (System), File (Operator)** | 12-factor + portability |
| 4 | Event retention | **No retention in bus** | Replay từ JSONL |
| 5 | Signature payload | **8 fields explicitly** | Deterministic, auditable |
| 6 | Trade+Fee | **Separate entries, atomic commit** | Clean invariants |

> ✅ **Consensus reached:** GPT5 + Gemini3 + Author (2026-01-26)

---

> **Document Status:** � LOCKED
> **Consensus:** GPT5 ✅ | Gemini3 ✅ | Author ✅
> **Next Step:** Implementation
> **Estimated Duration:** 6 weeks
