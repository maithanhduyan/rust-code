# GitHub Copilot Instructions

## Project Overview

This is a multi-project Rust workspace containing:

1. **`simbank/`** - English Banking DSL with SQLite + JSONL Event Sourcing (active development)
2. **`dsl/`** - Vietnamese Banking DSL Workspace (legacy/reference)
3. **`src/`** - Standalone Rust projects (asset_api, benchmark, foo, helloworld, rusqlite)

---

## Simbank Architecture (`simbank/`)

### Crate Hierarchy

```
simbank/
├── crates/
│   ├── core/           # Domain types (Money, Wallet, Person, Account, Event)
│   ├── persistence/    # SQLite repos + JSONL EventStore
│   ├── business/       # Services (Customer, Employee, Auditor, etc.)
│   ├── dsl/            # English DSL macros (banking_scenario!, rule!)
│   └── reports/        # Report exporters (CSV, JSON, Markdown)
├── migrations/         # SQLite migrations (sqlx)
└── data/events/        # JSONL event files (gitignored)
```

### Dependency Order

```
core → persistence → business → dsl/reports
```

### Dual-Write Pattern (Critical!)

All business operations follow **DB-first, Event-second**:

```rust
// ✅ Correct: Update SQLite, then append JSONL event
BalanceRepo::credit(pool, wallet_id, currency, amount).await?;
TransactionRepo::insert(pool, &tx_row).await?;
self.ctx.events().append(&event)?;  // Only if DB succeeded
```

---

## Key Patterns

### 1. ServiceContext (Business Layer)

All services receive `&ServiceContext` containing pool + event store:

```rust
pub struct CustomerService<'a> {
    ctx: &'a ServiceContext,
}

impl<'a> CustomerService<'a> {
    pub async fn deposit(&self, actor_id: &str, account_id: &str,
                         amount: Decimal, currency: &str) -> BusinessResult<TransactionResult>
}
```

### 2. Repository Pattern (Persistence)

Repos are stateless with static async methods:

```rust
// Get with Option handling
let wallet = WalletRepo::get_by_account_and_type(pool, account_id, WalletType::Funding)
    .await?
    .ok_or_else(|| BusinessError::WalletNotFound(...))?;

// Insert with row struct
let tx_row = TransactionRow { id, account_id, wallet_id, tx_type, amount, ... };
TransactionRepo::insert(pool, &tx_row).await?;
```

### 3. Money with rust_decimal

Store as TEXT in SQLite, use `Decimal` in code:

```rust
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

let amount = dec!(100.50);  // Compile-time checked
let balance: Decimal = row.available.parse()?;  // From DB TEXT
```

### 4. Event Types with AML Flags

```rust
let event = Event::deposit(&event_id, actor_id, account_id, amount, currency)
    .with_metadata(EventMetadata { ip_address, location, device_id });

// AML thresholds: $10,000+ = large_amount, $9,000-$9,999 = near_threshold
```

---

## Error Handling Strategy

| Crate | Strategy |
|-------|----------|
| `core` | `thiserror` enums (`CoreError`) |
| `persistence` | `thiserror` wrapping sqlx (`PersistenceError`) |
| `business` | `anyhow` for aggregation (`BusinessResult<T>`) |

---

## Build & Test Commands

```powershell
# Simbank (main project)
cd simbank; cargo build
cd simbank; cargo test

# Run specific crate tests
cd simbank; cargo test -p simbank-core
cd simbank; cargo test -p simbank-business

# Legacy DSL workspace
cd dsl; cargo build
```

---

## Adding New Features

### New Person Type Operation

1. Create service in `business/src/<role>.rs` implementing operation
2. Verify actor permissions via `PersonRepo::get_by_id`
3. Get wallet via `WalletRepo::get_by_account_and_type` (handle Option!)
4. Update balance via `BalanceRepo::credit/debit`
5. Insert transaction via `TransactionRepo::insert(&TransactionRow)`
6. Append event via `ctx.events().append(&event)`

### New Event Type

1. Add variant to `EventType` in `core/src/event.rs`
2. Add constructor method `Event::new_xyz()`
3. Update AML detection in `business/src/auditor.rs` if needed

---

## Critical API Patterns

### WalletRepo returns Option

```rust
// ❌ WRONG - will fail: no field `id` on Option<WalletRow>
let wallet = WalletRepo::get_by_account_and_type(...).await?;
wallet.id  // ERROR!

// ✅ CORRECT - unwrap Option with error
let wallet = WalletRepo::get_by_account_and_type(...).await?
    .ok_or_else(|| BusinessError::WalletNotFound(...))?;
```

### TransactionRepo::insert takes struct

```rust
// ❌ WRONG - wrong number of arguments
TransactionRepo::insert(pool, &id, &account_id, &wallet_id, "deposit", amount, currency, desc).await?;

// ✅ CORRECT - pass TransactionRow struct
let tx_row = TransactionRow {
    id: txn_id,
    account_id: account_id.to_string(),
    wallet_id: wallet.id.clone(),
    tx_type: "deposit".to_string(),
    amount: amount.to_string(),
    currency_code: currency.to_string(),
    description: Some(desc),
    created_at: Utc::now(),
};
TransactionRepo::insert(pool, &tx_row).await?;
```

---

## Person Types & Permissions

| Type | Has Wallet | Can Do |
|------|------------|--------|
| Customer | Spot + Funding | deposit, withdraw, transfer |
| Employee | Funding only | receive salary, buy insurance |
| Shareholder | Funding only | receive dividends |
| Manager | None | approve operations, pay salary/bonus |
| Auditor | None | scan transactions, generate AML reports |

---

## File Organization (Simbank)

| Path | Purpose |
|------|---------|
| `core/src/money.rs` | Currency, Money newtypes |
| `core/src/event.rs` | Event, EventType, AmlFlag |
| `persistence/src/sqlite/repos.rs` | All *Repo implementations |
| `persistence/src/sqlite/schema.rs` | *Row structs for DB |
| `persistence/src/events/store.rs` | JSONL append-only writer |
| `business/src/services.rs` | ServiceContext, TransactionResult |
| `business/src/customer.rs` | CustomerService |
| `business/src/auditor.rs` | AuditorService + AmlThresholds |

---

## Legacy DSL Workspace (`dsl/`)

Vietnamese DSL using `macro_rules!` with special syntax constraints:

```rust
// ⚠️ expr fragment CANNOT be followed by custom keywords
($balance:expr cho $name:literal)  // ❌ Won't compile
($balance:expr, $name:literal)     // ✅ Use comma/semicolon after expr
```

See `dsl/crates/dsl-macros/src/lib.rs` for macro patterns.
