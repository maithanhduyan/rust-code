# BiBank - Financial State OS

> **Status:** Phase 1 Complete ✅
> **Version:** 0.1.0

**BiBank** là một **Financial State OS** - không phải app ngân hàng truyền thống.

## Nguyên tắc nền tảng

- **Ledger là nguồn sự thật duy nhất** - Không DB nào được "sửa state"
- **Không reconcile** - Nếu cần reconcile → kiến trúc sai
- **Correct-by-construction** - Risk engine chặn ngay tại write-time
- **Event-first, snapshot-second** - Snapshot chỉ để tối ưu đọc

## Kiến trúc

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

## Cấu trúc Crates

| Crate | Mô tả |
|-------|-------|
| `bibank-core` | Domain types (Amount) |
| `bibank-ledger` | Double-entry core (AccountKey, JournalEntry, hash chain) |
| `bibank-risk` | Pre-commit gatekeeper (balance checks) |
| `bibank-events` | JSONL append-only store |
| `bibank-bus` | Event distribution |
| `bibank-projection` | SQLite read models |
| `bibank-rpc` | CLI orchestrator |
| `bibank-dsl` | Future DSL macros |

## Quick Start

### Build

```bash
cargo build --release
```

### Initialize System

```bash
./target/release/bibank init
```

### Deposit

```bash
./target/release/bibank deposit ALICE 1000 USDT
```

### Transfer

```bash
./target/release/bibank transfer ALICE BOB 300 USDT
```

### Check Balance

```bash
./target/release/bibank balance ALICE
```

### Withdraw

```bash
./target/release/bibank withdraw ALICE 200 USDT
```

### Audit (verify hash chain)

```bash
./target/release/bibank audit
```

### Replay (rebuild projections)

```bash
./target/release/bibank replay --reset
```

## Account Key Format

```
CATEGORY:SEGMENT:ID:ASSET:SUB_ACCOUNT
```

Ví dụ:
- `LIAB:USER:ALICE:USDT:AVAILABLE` - Số dư USDT khả dụng của Alice
- `ASSET:SYSTEM:VAULT:USDT:MAIN` - Kho USDT của hệ thống
- `REV:SYSTEM:FEE:USDT:REVENUE` - Doanh thu phí giao dịch

### Categories

| Category | Code | Normal Balance | Mô tả |
|----------|------|----------------|-------|
| Asset | `ASSET` | Debit | Tài sản hệ thống |
| Liability | `LIAB` | Credit | User balances |
| Equity | `EQUITY` | Credit | Vốn chủ sở hữu |
| Revenue | `REV` | Credit | Doanh thu |
| Expense | `EXP` | Debit | Chi phí |

## Double-Entry Accounting

Mọi giao dịch phải cân bằng theo từng asset:

```
Deposit 100 USDT to Alice:
  Debit  ASSET:SYSTEM:VAULT:USDT:MAIN       100
  Credit LIAB:USER:ALICE:USDT:AVAILABLE     100
  ──────────────────────────────────────────────
  USDT Balance: +100 - 100 = 0 ✅
```

## Data Storage

- **Source of Truth:** `data/journal/*.jsonl` (append-only)
- **Projection:** `data/projection.db` (disposable, rebuilt from events)

### JSONL Format

```json
{"sequence":1,"prev_hash":"GENESIS","hash":"abc...","timestamp":"2026-01-25T10:00:00Z","intent":"genesis",...}
{"sequence":2,"prev_hash":"abc...","hash":"def...","timestamp":"2026-01-25T10:01:00Z","intent":"deposit",...}
```

## Risk Engine

Mọi giao dịch được kiểm tra **TRƯỚC** khi commit:

- ✅ Deposit: Luôn được phép
- ⚠️ Withdrawal/Transfer: Kiểm tra `balance >= amount`
- ❌ Overdraft: Bị chặn tự động

```bash
$ bibank withdraw BOB 500 USDT
Error: Risk error: Insufficient balance for LIAB:USER:BOB:USDT:AVAILABLE: available 300, required 500
```

## Hash Chain

Mỗi entry có hash của entry trước, tạo chain bất biến:

```
Entry 1: prev_hash = "GENESIS", hash = "abc..."
Entry 2: prev_hash = "abc...",  hash = "def..."
Entry 3: prev_hash = "def...",  hash = "ghi..."
```

Verify chain:
```bash
./target/release/bibank audit
✅ Hash chain verified (3 entries)
```

## Testing

```bash
# Run all tests
cargo test

# Run integration tests only
cargo test -p bibank-rpc --test integration
```

### Test Coverage

| Test | Description |
|------|-------------|
| `test_full_workflow` | Genesis → Deposit → Transfer → Balance check |
| `test_risk_blocks_overdraft` | Risk engine rejects insufficient balance |
| `test_hash_chain_integrity` | Verify hash chain và sequence |
| `test_replay_rebuilds_state` | Restart rebuilds correct state |
| `test_double_entry_validation` | Unbalanced entries rejected |
| `test_multi_asset_balanced` | Multi-asset support |

## Phase 1 Success Criteria

- [x] `bibank replay --reset` drops projection DB, replays all events, state is identical
- [x] Double-entry validation rejects unbalanced entries at compile/runtime
- [x] Risk engine rejects withdrawal when balance < amount
- [x] Hash chain is verified on replay
- [x] Sequence is derived from JSONL, no gaps after restart

## Phase 2+ Roadmap

| Feature | Phase |
|---------|-------|
| Trade intent (multi-asset swap) | Phase 2 |
| Fee intent | Phase 2 |
| Event Bus (pub/sub) | Phase 2 |
| Digital signatures | Phase 2 |
| Margin/Exposure checks | Phase 3 |
| Liquidation | Phase 3 |
| AML hooks | Phase 4 |
| Rule DSL | Phase 4 |

## License

MIT
