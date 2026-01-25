# GitHub Copilot Instructions

## Project Overview

This is a multi-project Rust workspace containing:

1. **`dsl/`** - Vietnamese Banking DSL Workspace (main focus)
2. **`src/`** - Standalone Rust projects (asset_api, benchmark, foo, helloworld, rusqlite)

---

## DSL Workspace Architecture (`dsl/`)

### Crate Hierarchy

```
dsl/
├── crates/
│   ├── core-banking/     # Foundation types & traits (VND, Account, Transaction)
│   ├── business/         # Business logic (Interest, Tax, Fee, Process)
│   ├── dsl-macros/       # Vietnamese DSL macro_rules! macros
│   └── reports/          # Report exporters (CSV, JSON, Markdown)
└── examples/
    ├── basic/            # Simple usage demo
    └── advanced/         # Full business workflow demo
```

### Dependency Order

```
core-banking → business → dsl-macros → reports
                    ↓           ↓
              examples/basic  examples/advanced
```

---

## Code Conventions

### Rust Standards

- **Edition**: 2021
- **Resolver**: Version 2 (Cargo workspace)
- **Naming**: Snake case for functions/variables, Pascal case for types

### Vietnamese Identifiers

This DSL uses Vietnamese naming for domain-specific code:

```rust
// ✅ Vietnamese identifiers for DSL
let mut tk = tài_khoản!(tiết_kiệm "TK-001", 1000.0);
let interest = lãi_suất! { ... };
let tax = thuế! { ... };

// ✅ English for infrastructure code
fn calculate_interest(balance: VND) -> VND { ... }
pub trait InterestCalculator { ... }
```

---

## ⚠️ Critical: Macro Syntax Rules

### The `expr` Fragment Constraint

In Rust `macro_rules!`, an `expr` fragment **CANNOT be followed by custom keywords**.

```rust
// ❌ WILL NOT COMPILE - Vietnamese keyword after expr
($balance:expr cho $name:literal)
($amount:expr từ $source:ident)
($value:expr năm $years:expr)

// ✅ CORRECT - Use comma, semicolon, or => after expr
($balance:expr, $name:literal)
($amount:expr; $source:ident)
($value:expr => $years:expr)
```

### DSL Macro Patterns

Always follow these patterns when extending DSL macros:

```rust
// Account creation
tài_khoản!(tiết_kiệm $name:literal, $balance:expr)
tài_khoản!(thanh_toán $name:literal, $balance:expr)

// Interest rate tiers - use tuple syntax, not keywords
lãi_suất! {
    tên: $name:literal,
    cấp: [
        ($min:expr, $max:expr): $rate:literal % => $desc:literal,
        ($min:expr, MAX): $rate:literal % => $desc:literal,  // MAX is special
    ]
}

// Tax rules
thuế! {
    tên: $name:literal,
    quy_tắc: [
        lãi_dưới $threshold:expr => $level:ident,
    ],
    mặc_định: $default:ident
}
```

---

## Design Patterns

### Newtype Pattern (Type Safety)

```rust
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct VND(pub f64);

impl VND {
    pub fn new(amount: f64) -> Self { VND(amount) }
    pub fn value(&self) -> f64 { self.0 }
}
```

### Trait-Based Calculators (Extensibility)

```rust
pub trait InterestCalculator {
    fn calculate(&self, balance: VND) -> VND;
    fn rate_for_balance(&self, balance: VND) -> Percentage;
}

pub trait TaxCalculator {
    fn calculate(&self, interest: VND) -> VND;
}

pub trait FeeCalculator {
    fn calculate(&self, account_type: &AccountType) -> VND;
}
```

### Builder Pattern (Complex Objects)

```rust
let table = TieredInterestTable::new("Name")
    .add_tier(VND(0.0), VND(1000.0), Percentage(0.1), "Basic")
    .add_tier(VND(1000.0), VND(10000.0), Percentage(0.2), "Premium");
```

---

## Build & Test Commands

```powershell
# Build entire DSL workspace
cd dsl; cargo build

# Run all tests
cd dsl; cargo test

# Run specific example
cd dsl; cargo run -p example-basic
cd dsl; cargo run -p example-advanced

# Build standalone projects
cd src/asset_api; cargo build
cd src/helloworld; cargo build
```

---

## Adding New Features

### New Account Type

1. Add variant to `AccountType` in `core-banking/src/account.rs`
2. Update `phí!` macro in `dsl-macros/src/lib.rs` if fees differ
3. Add tests

### New Interest Tier Logic

1. Implement `InterestCalculator` trait in `business/src/interest.rs`
2. Update `lãi_suất!` macro if new syntax needed
3. Export from `business/src/lib.rs`

### New Report Format

1. Implement `ReportExporter` trait in `reports/src/exporters.rs`:
```rust
pub trait ReportExporter {
    fn export_summary(&self, summary: &AccountSummary) -> String;
    fn export_yearly(&self, report: &YearlyReport) -> String;
}
```

---

## File Organization

| Path | Purpose |
|------|---------|
| `dsl/crates/core-banking/src/types.rs` | VND, Percentage newtypes |
| `dsl/crates/core-banking/src/account.rs` | Account, AccountType |
| `dsl/crates/core-banking/src/transaction.rs` | Transaction types |
| `dsl/crates/core-banking/src/traits.rs` | Calculator traits |
| `dsl/crates/business/src/interest.rs` | TieredInterestTable |
| `dsl/crates/business/src/tax.rs` | TaxTable, TaxLevel |
| `dsl/crates/business/src/fee.rs` | FeeSchedule |
| `dsl/crates/business/src/process.rs` | YearlyProcess |
| `dsl/crates/dsl-macros/src/lib.rs` | All Vietnamese DSL macros |
| `dsl/crates/reports/src/summary.rs` | AccountSummary |
| `dsl/crates/reports/src/yearly.rs` | YearlyReport |
| `dsl/crates/reports/src/exporters.rs` | CSV, JSON, Markdown exporters |

---

## Common Pitfalls

### ❌ Don't Do This

```rust
// Wrong: Using keywords after expr
macro_rules! wrong {
    ($amt:expr cho $name:literal) => { ... }  // Won't compile
}

// Wrong: Mixing English and Vietnamese inconsistently
let account = tài_khoản!(...);
account.get_balance();  // ✅ English for methods
account.lấy_số_dư();    // ❌ Don't mix in methods
```

### ✅ Do This Instead

```rust
// Correct: Comma after expr
macro_rules! correct {
    ($amt:expr, $name:literal) => { ... }
}

// Correct: Vietnamese for DSL, English for internals
let tk = tài_khoản!(tiết_kiệm "TK-001", 1000.0);
tk.deposit(VND(500.0));  // English methods
```

---

## Testing Strategy

- **Unit tests**: In each crate's `src/*.rs` files using `#[cfg(test)]`
- **Integration tests**: In `examples/` crates
- **Coverage**: Core types, trait implementations, macro outputs

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiered_interest() {
        let table = TieredInterestTable::new("Test")
            .add_tier(VND(0.0), VND(100.0), Percentage(0.1), "Tier 1");

        let interest = table.calculate(VND(50.0));
        assert!(interest.value() > 0.0);
    }
}
```

---

## Standalone Projects (`src/`)

| Project | Tech Stack | Purpose |
|---------|------------|---------|
| `asset_api` | actix-web, sqlx, tokio | REST API example |
| `benchmark` | - | Performance testing |
| `helloworld` | - | Learning/template |
| `rusqlite` | rusqlite | SQLite examples |
| `foo` | - | Scratch/experiments |

Each has its own `Cargo.toml` and is independent of the DSL workspace.
