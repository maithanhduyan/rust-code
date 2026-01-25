# SIMBANK - Proposed Implementation Plan

> **Document Version:** 1.0
> **Date:** 2026-01-25
> **Status:** ✅ Approved | Đã thống nhất, sẵn sàng triển khai

---

## 1. Tổng quan dự án

**Mục tiêu:** Xây dựng ứng dụng ngân hàng/sàn giao dịch đơn giản để minh họa:
- Cách xây dựng DSL (Domain Specific Language) trong Rust
- Kết hợp SQLite (current state) + JSONL Event Sourcing (audit trail)
- Business rules phức tạp cho AML compliance

**Đối tượng sử dụng DSL:** Business Analyst (BA), chuyên gia nghiệp vụ

---

## 2. Kiến trúc hệ thống

### 2.1 Workspace Structure (Option C: Hybrid)

```
simbank/
├── Cargo.toml                    # Workspace root
├── README.md
│
├── crates/
│   ├── core/                     # Pure domain types (no dependencies)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── money.rs          # Currency, Money (rust_decimal)
│   │       ├── wallet.rs         # WalletType, Wallet, Balance
│   │       ├── person.rs         # PersonType, Person
│   │       ├── account.rs        # Account
│   │       ├── event.rs          # Event, EventType, EventMetadata
│   │       └── error.rs          # Domain errors (thiserror)
│   │
│   ├── persistence/              # DB + Events combined
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sqlite/
│   │       │   ├── mod.rs
│   │       │   ├── schema.rs
│   │       │   └── repos.rs      # Repository implementations
│   │       └── events/
│   │           ├── mod.rs
│   │           ├── store.rs      # JSONL writer
│   │           └── replay.rs     # Event replay
│   │
│   ├── business/                 # Service layer
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── customer.rs       # deposit, withdraw, transfer
│   │       ├── employee.rs       # insurance, salary
│   │       ├── management.rs     # bonus
│   │       ├── shareholder.rs    # dividend
│   │       └── auditor.rs        # AML detection rules
│   │
│   ├── dsl/                      # English DSL macros
│   │   └── src/lib.rs            # banking_scenario!, rule!
│   │
│   └── reports/                  # Report generation
│       └── src/
│           ├── lib.rs
│           ├── exporters.rs      # CSV, JSON, Markdown
│           └── aml_report.rs     # AML report formatting
│
├── migrations/                   # sqlx migrate files (workspace root)
│   └── 20260125_init.sql
│
├── data/                         # Runtime data (gitignored)
│   ├── simbank.db
│   └── events/
│       └── *.jsonl
│
├── examples/                     # Scenario-based examples
│   ├── 01_customer_onboarding.rs
│   ├── 02_employee_operations.rs
│   ├── 03_shareholder_dividends.rs
│   ├── 04_auditor_aml_scan.rs
│   └── 05_complex_scenario.rs
│
├── tests/
│   └── integration/
│
└── docs/
    ├── review/                   # Thảo luận đã qua
    └── proposed/                 # Proposal documents
```

### 2.2 Dependency Graph

```
core (no external deps except serde, rust_decimal, thiserror, chrono, uuid)
   ↓
persistence (depends: core, sqlx, serde_json)
   ↓
business (depends: core, persistence)
   ↓
dsl + reports (depends: business)
   ↓
examples/tests/cli
```

---

## 3. Quyết định kỹ thuật đã thống nhất

### 3.1 Core Decisions

| # | Chủ đề | Quyết định | Lý do |
|---|--------|------------|-------|
| 1 | Kiến trúc | **Option C: Hybrid (5 crates)** | Cân bằng giữa separation of concerns và độ phức tạp |
| 2 | `data/` folder | **Gitignore**, chỉ commit `.gitkeep` | Không commit runtime data |
| 3 | `migrations/` | **Workspace root** | Dễ chạy `sqlx migrate` từ root |
| 4 | DSL Style | **Unified macro `banking_scenario!`** | Minh họa sức mạnh Rust macros, dễ đọc cho BA |
| 5 | AML Logic | Detection → `business/auditor.rs`, Format → `reports/aml_report.rs` | Separation of concerns |
| 6 | sqlx version | **0.8** | Latest stable, greenfield project |
| 7 | Examples | **Theo Scenario/Stakeholder** | Map 1:1 với README requirements |
| 8 | Phase 1 Scope | **CLI + Library only** (no REST API) | Focus vào DSL, không loãng trọng tâm |

### 3.2 Money & Currency

| # | Chủ đề | Quyết định | Lý do |
|---|--------|------------|-------|
| 9 | Money Type | **`rust_decimal`** với `serde-with-str` | Precision cho cả fiat và crypto |
| 10 | Multi-currency | **Dynamic decimals** per currency | ETH (18), BTC (8), USD (2), VND (0) |
| 11 | DB storage | **TEXT** cho Decimal | Parse chính xác 100% |

### 3.3 Account & Wallet Model (Exchange-style)

| # | Chủ đề | Quyết định | Lý do |
|---|--------|------------|-------|
| 12 | Model | **Account → Wallets → Balances** | Giống Binance/OKX |
| 13 | Phase 1 Wallets | **Spot + Funding** only | Demo basic flow |
| 14 | Phase 2 Wallets | Margin, Futures, Earn | Mở rộng sau |
| 15 | Internal Transfer | **Miễn phí** | Giống các sàn lớn |
| 16 | Locked Balance | **Schema có, logic Phase 2** | Chuẩn bị sẵn DB |
| 17 | Wallet Creation | **Eager** (auto-create all types) | Dễ query |
| 18 | Account:Person | **1:1** | Đơn giản hóa Phase 1 |

### 3.4 Person Types

| # | Chủ đề | Quyết định | Lý do |
|---|--------|------------|-------|
| 19 | Customer | **Full wallets** (Spot, Funding) | Main user |
| 20 | Employee | **Funding only** (lương, bảo hiểm) | Internal operations |
| 21 | Shareholder | **Funding only** (cổ tức) | Receive dividends |
| 22 | Manager | **No wallet**, permissions only | Approve operations |
| 23 | Auditor | **No wallet**, read-only | Query events |

### 3.5 Event Sourcing

| # | Chủ đề | Quyết định | Lý do |
|---|--------|------------|-------|
| 24 | Granularity | **Ghi tất cả**, filter khi query | Compliance requirement |
| 25 | Format | **JSONL** (append-only) | AML audit trail |
| 26 | Event fields | `from_wallet`, `to_wallet`, `ip_address`, `location` | Traceability |
| 27 | AML flags | `large_amount`, `unusual_pattern`, `cross_border`, `high_risk_country` | Big 4 requirements |

### 3.6 Error Handling

| Crate | Strategy |
|-------|----------|
| `core` | `thiserror` (domain errors) |
| `persistence` | `thiserror` + wrap sqlx errors |
| `business` | `anyhow` (aggregation) |
| `dsl` | Compile-time errors |

### 3.7 ID Generation

| # | Chủ đề | Quyết định | Lý do |
|---|--------|------------|-------|
| 28 | Format | **Prefixed ID** | Readable trong logs |
| 29 | Examples | `ACC_001`, `WAL_002`, `TXN_003`, `CUST_004` | BA dễ hiểu |

### 3.8 Crate Naming

| # | Chủ đề | Quyết định |
|---|--------|------------|
| 30 | Convention | `simbank-core`, `simbank-persistence`, etc. (hyphen) |

---

## 4. Database Schema

### 4.1 Tables

```sql
-- migrations/20260125_init.sql

-- Wallet types enum table
CREATE TABLE wallet_types (
    code TEXT PRIMARY KEY,      -- 'spot', 'margin', 'futures', 'funding', 'earn'
    name TEXT NOT NULL,
    description TEXT
);

-- Currencies with dynamic decimals
CREATE TABLE currencies (
    code TEXT PRIMARY KEY,      -- 'USD', 'VND', 'BTC', 'ETH', 'USDT'
    name TEXT NOT NULL,
    decimals INTEGER NOT NULL,  -- 2, 0, 8, 18, 6
    symbol TEXT
);

-- Person types
CREATE TABLE persons (
    id TEXT PRIMARY KEY,        -- 'CUST_001', 'EMP_001'
    person_type TEXT NOT NULL,  -- 'customer', 'employee', 'shareholder', 'manager', 'auditor'
    name TEXT NOT NULL,
    email TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Accounts (1:1 with Person)
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,        -- 'ACC_001'
    person_id TEXT NOT NULL UNIQUE,
    status TEXT DEFAULT 'active',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (person_id) REFERENCES persons(id)
);

-- Wallets (mỗi account có nhiều wallets)
CREATE TABLE wallets (
    id TEXT PRIMARY KEY,        -- 'WAL_001'
    account_id TEXT NOT NULL,
    wallet_type TEXT NOT NULL,  -- 'spot', 'funding'
    status TEXT DEFAULT 'active',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(account_id, wallet_type),
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (wallet_type) REFERENCES wallet_types(code)
);

-- Balances (mỗi wallet có nhiều currencies)
CREATE TABLE balances (
    wallet_id TEXT NOT NULL,
    currency_code TEXT NOT NULL,
    available TEXT NOT NULL DEFAULT '0',    -- Decimal as TEXT
    locked TEXT NOT NULL DEFAULT '0',       -- Phase 2 logic
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (wallet_id, currency_code),
    FOREIGN KEY (wallet_id) REFERENCES wallets(id),
    FOREIGN KEY (currency_code) REFERENCES currencies(code)
);

-- Transactions (immutable ledger)
CREATE TABLE transactions (
    id TEXT PRIMARY KEY,        -- 'TXN_001'
    account_id TEXT NOT NULL,
    wallet_id TEXT NOT NULL,
    tx_type TEXT NOT NULL,      -- 'deposit', 'withdrawal', 'internal_transfer', 'trade'
    amount TEXT NOT NULL,       -- Decimal as TEXT
    currency_code TEXT NOT NULL,
    description TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (wallet_id) REFERENCES wallets(id)
);

-- Seed data
INSERT INTO wallet_types VALUES
    ('spot', 'Spot Wallet', 'For trading'),
    ('funding', 'Funding Wallet', 'For deposit/withdraw'),
    ('margin', 'Margin Wallet', 'For margin trading'),
    ('futures', 'Futures Wallet', 'For futures contracts'),
    ('earn', 'Earn Wallet', 'For staking/savings');

INSERT INTO currencies VALUES
    ('VND', 'Vietnamese Dong', 0, '₫'),
    ('USD', 'US Dollar', 2, '$'),
    ('USDT', 'Tether', 6, '₮'),
    ('BTC', 'Bitcoin', 8, '₿'),
    ('ETH', 'Ethereum', 18, 'Ξ');
```

---

## 5. Event Schema (JSONL)

```json
{
  "event_id": "EVT_001",
  "timestamp": "2026-01-25T10:00:00Z",
  "event_type": "Deposit",
  "actor_id": "CUST_001",
  "actor_role": "Customer",
  "account_id": "ACC_001",
  "from_wallet": null,
  "to_wallet": "funding",
  "amount": "1000.00",
  "currency": "USDT",
  "metadata": {
    "ip_address": "192.168.1.1",
    "location": "VN",
    "device_id": "mobile_ios_001"
  },
  "aml_flags": []
}
```

---

## 6. DSL Syntax Examples

### 6.1 Banking Scenario (Unified Macro)

```rust
use simbank_dsl::banking_scenario;

banking_scenario! {
    // Customer workflow
    Customer "Alice" {
        deposit(100 USDT, to: Funding);
        transfer(50 USDT, from: Funding, to: Spot);
    }

    // Employee workflow
    Employee "Bob" {
        receive_salary(5000 USD, to: Funding);
        buy_insurance(plan: "Health_Premium", cost: 200 USD);
    }

    // Auditor workflow
    Auditor "Deloitte" {
        scan_transactions(from: "2026-01-01", flags: ["large_amount"]);
        generate_report(format: Markdown);
    }
}
```

### 6.2 Business Rules

```rust
use simbank_dsl::rule;

rule! {
    Name: "Withdrawal Limit"
    If withdrawal.amount > 10000 USD
    Then require_approval(from: Manager)
}

rule! {
    Name: "AML Large Transaction"
    If transaction.amount > 10000 USD
    Then flag_aml("large_amount")
}
```

---

## 7. CLI Commands

```bash
# Account management
simbank account create --type customer --name "Alice"
simbank account list

# Wallet operations
simbank deposit ACC_001 100 USDT --to funding
simbank transfer ACC_001 50 USDT --from funding --to spot
simbank withdraw ACC_001 30 USDT --from funding

# DSL mode (run scenario file)
simbank run examples/01_customer_onboarding.rs

# Audit
simbank audit --from 2026-01-01 --flags large_amount
simbank report --format markdown --output report.md
```

---

## 8. Dependencies (Cargo.toml)

```toml
[workspace]
members = [
    "crates/core",
    "crates/persistence",
    "crates/business",
    "crates/dsl",
    "crates/reports",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Simbank Team"]

[workspace.dependencies]
# Async Runtime
tokio = { version = "1.36", features = ["full"] }

# Database (SQLite)
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Money
rust_decimal = { version = "1.33", features = ["serde-with-str"] }

# Error Handling
thiserror = "2.0"
anyhow = "1.0"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.7", features = ["serde", "v4"] }
tracing = "0.1"
tracing-subscriber = "0.3"

# Internal Crates
simbank-core = { path = "./crates/core" }
simbank-persistence = { path = "./crates/persistence" }
simbank-business = { path = "./crates/business" }
simbank-reports = { path = "./crates/reports" }
simbank-dsl = { path = "./crates/dsl" }
```

---

## 9. Implementation Phases

### Phase 1: Foundation (Tuần 1-2)

| Step | Task | Deliverable |
|------|------|-------------|
| 1 | Setup workspace | `Cargo.toml`, folder structure |
| 2 | Core crate | `money.rs`, `wallet.rs`, `person.rs`, `event.rs`, `error.rs` |
| 3 | DB crate | SQLite schema, migrations, repositories |
| 4 | Events crate | JSONL store, replay |
| 5 | Unit tests | Core types, DB CRUD |

### Phase 2: Business Logic (Tuần 3)

| Step | Task | Deliverable |
|------|------|-------------|
| 6 | Customer operations | `deposit`, `withdraw`, `internal_transfer` |
| 7 | Dual write | Update DB + Write Event |
| 8 | AML detection | Basic rules trong `auditor.rs` |
| 9 | Integration tests | Full workflows |

### Phase 3: DSL & CLI (Tuần 4)

| Step | Task | Deliverable |
|------|------|-------------|
| 10 | DSL macros | `banking_scenario!`, `rule!` |
| 11 | CLI | Basic commands |
| 12 | Examples | 5 scenario files |
| 13 | Reports | CSV, JSON, Markdown exporters |
| 14 | Documentation | README, inline docs |

### Phase 4: Extensions (Future)

- Margin/Futures wallets
- Locked balance logic
- REST API (optional)
- Crypto trading simulation
- Advanced AML patterns

---

## 10. Gitignore

```gitignore
# Runtime data
data/simbank.db
data/simbank.db-shm
data/simbank.db-wal
data/events/*.jsonl

# Keep folder structure
!data/.gitkeep
!data/events/.gitkeep

# Rust
target/
Cargo.lock
```

---

## 11. Next Action

**Start Step 1:** Khởi tạo workspace và Core crate

```bash
cd simbank
# Workspace đã có Cargo.toml
cargo new --lib crates/core
cargo new --lib crates/persistence
cargo new --lib crates/business
cargo new --lib crates/dsl
cargo new --lib crates/reports
mkdir -p migrations data/events examples tests/integration
```

---

> **Document Author:** GitHub Copilot + User
> **Review Status:** ✅ Đã thống nhất tất cả các điểm
