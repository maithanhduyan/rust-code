# Kiến trúc hệ thống SIMBANK 🏗️

> Tài liệu mô tả chi tiết kiến trúc kỹ thuật của ứng dụng SIMBANK

---

## 1. Tổng quan kiến trúc

SIMBANK được xây dựng theo kiến trúc **Clean Architecture** với các lớp tách biệt rõ ràng:

```
┌─────────────────────────────────────────────────────────────┐
│                         CLI Layer                           │
│                    (simbank-cli crate)                      │
├─────────────────────────────────────────────────────────────┤
│                         DSL Layer                           │
│              (simbank-dsl + simbank-reports)                │
├─────────────────────────────────────────────────────────────┤
│                      Business Layer                         │
│                  (simbank-business crate)                   │
├─────────────────────────────────────────────────────────────┤
│                    Persistence Layer                        │
│                (simbank-persistence crate)                  │
├─────────────────────────────────────────────────────────────┤
│                       Core Layer                            │
│                   (simbank-core crate)                      │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Workspace Structure

```
simbank/
├── Cargo.toml                    # Workspace root
├── README.md
│
├── crates/
│   ├── core/                     # Domain types thuần túy
│   │   └── src/
│   │       ├── lib.rs           # Re-exports
│   │       ├── money.rs         # Currency, Money (rust_decimal)
│   │       ├── wallet.rs        # WalletType, Wallet, Balance
│   │       ├── person.rs        # PersonType, Person
│   │       ├── account.rs       # Account, AccountStatus
│   │       ├── event.rs         # Event, EventType, AmlFlag
│   │       └── error.rs         # CoreError (thiserror)
│   │
│   ├── persistence/             # Lớp lưu trữ dữ liệu
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sqlite/
│   │       │   ├── mod.rs
│   │       │   ├── schema.rs    # *Row structs cho DB
│   │       │   └── repos.rs     # Repository implementations
│   │       └── events/
│   │           ├── mod.rs
│   │           ├── store.rs     # JSONL append-only writer
│   │           └── replay.rs    # Event replay & filtering
│   │
│   ├── business/                # Lớp nghiệp vụ
│   │   └── src/
│   │       ├── lib.rs           # ServiceContext
│   │       ├── services.rs      # TransactionResult
│   │       ├── customer.rs      # CustomerService
│   │       ├── employee.rs      # EmployeeService
│   │       ├── shareholder.rs   # ShareholderService
│   │       ├── manager.rs       # ManagerService
│   │       ├── auditor.rs       # AuditorService, AmlThresholds
│   │       └── error.rs         # BusinessError (anyhow)
│   │
│   ├── dsl/                     # DSL macros
│   │   └── src/
│   │       ├── lib.rs           # banking_scenario!, rule!
│   │       ├── scenario.rs      # Scenario, CustomerOp, etc.
│   │       └── rules.rs         # Rule, RuleCondition, RuleAction
│   │
│   ├── reports/                 # Xuất báo cáo
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── exporters.rs     # CSV, JSON, Markdown exporters
│   │       └── aml_report.rs    # AmlReport, VelocityReport
│   │
│   └── cli/                     # Command-line interface
│       └── src/
│           └── main.rs          # Clap-based CLI
│
├── examples/                    # Ví dụ minh họa DSL
│   ├── Cargo.toml
│   ├── 01_customer_onboarding.rs
│   ├── 02_employee_operations.rs
│   ├── 03_shareholder_dividends.rs
│   ├── 04_auditor_aml_scan.rs
│   └── 05_complex_scenario.rs
│
├── migrations/                  # SQLite migrations
│   └── 20260125_init.sql
│
├── data/                        # Runtime data (gitignored)
│   ├── simbank.db
│   └── events/*.jsonl
│
└── docs/                        # Tài liệu
```

---

## 3. Chi tiết từng crate

### 3.1 simbank-core

**Mục đích:** Domain types thuần túy, không có dependencies ngoại trừ các thư viện cơ bản.

**Dependencies:**
- `serde` - Serialization
- `rust_decimal` - Số thập phân chính xác cho tiền tệ
- `thiserror` - Error handling
- `chrono` - Date/time
- `uuid` - Unique IDs

**Các module chính:**

| Module | Types | Mô tả |
|--------|-------|-------|
| `money.rs` | `Currency`, `Money` | Đại diện tiền tệ với độ chính xác cao |
| `wallet.rs` | `WalletType`, `Wallet`, `Balance` | Ví tiền và số dư |
| `person.rs` | `PersonType`, `Person` | Người dùng hệ thống |
| `account.rs` | `Account`, `AccountStatus` | Tài khoản ngân hàng |
| `event.rs` | `Event`, `EventType`, `AmlFlag` | Sự kiện cho event sourcing |
| `error.rs` | `CoreError` | Lỗi domain |

### 3.2 simbank-persistence

**Mục đích:** Lớp lưu trữ dữ liệu với dual-write pattern (SQLite + JSONL).

**Dependencies:**
- `simbank-core`
- `sqlx` - SQLite async driver
- `serde_json` - JSON serialization

**Pattern quan trọng: Dual-Write**

```rust
// ✅ Đúng: Ghi SQLite trước, sau đó ghi JSONL
BalanceRepo::credit(pool, wallet_id, currency, amount).await?;
TransactionRepo::insert(pool, &tx_row).await?;
event_store.append(&event)?;  // Chỉ khi DB thành công
```

**Repository Pattern:**

```rust
// Repos là stateless với static async methods
impl WalletRepo {
    pub async fn get_by_account_and_type(
        pool: &SqlitePool,
        account_id: &str,
        wallet_type: WalletType,
    ) -> Result<Option<WalletRow>, PersistenceError>;
}
```

### 3.3 simbank-business

**Mục đích:** Logic nghiệp vụ, orchestration giữa repos và events.

**Dependencies:**
- `simbank-core`
- `simbank-persistence`
- `anyhow` - Error aggregation

**ServiceContext:**

```rust
pub struct ServiceContext {
    pool: SqlitePool,
    event_store: EventStore,
}

impl ServiceContext {
    pub fn pool(&self) -> &SqlitePool;
    pub fn events(&self) -> &EventStore;
}
```

**Services:**

| Service | Actor | Operations |
|---------|-------|------------|
| `CustomerService` | Customer | deposit, withdraw, transfer |
| `EmployeeService` | Employee | receive_salary, buy_insurance |
| `ShareholderService` | Shareholder | receive_dividend |
| `ManagerService` | Manager | pay_salary, pay_bonus, pay_dividend |
| `AuditorService` | Auditor | scan_transactions, check_aml |

### 3.4 simbank-dsl

**Mục đích:** DSL macros cho nghiệp vụ ngân hàng.

**Dependencies:**
- `simbank-core`
- `simbank-business`
- `rust_decimal_macros`

**Macros:**

| Macro | Mô tả |
|-------|-------|
| `banking_scenario!` | Định nghĩa kịch bản với nhiều stakeholders |
| `rule!` | Định nghĩa quy tắc AML |

**Syntax `banking_scenario!`:**

```rust
banking_scenario! {
    Customer "name" {
        deposit <amount> <currency> to <wallet>;
        withdraw <amount> <currency> from <wallet>;
        transfer <amount> <currency> from <wallet> to <wallet>;
    }

    Employee "name" {
        receive_salary <amount> <currency>;
        buy_insurance "plan" for <amount> <currency>;
    }

    Shareholder "name" {
        receive_dividend <amount> <currency>;
    }

    Manager "name" {
        pay_salary to "employee" amount <amount> <currency>;
        pay_bonus to "employee" amount <amount> <currency> reason "reason";
        pay_dividend to "shareholder" amount <amount> <currency>;
    }

    Auditor "name" {
        scan from "date" to "date" flags ["flag1", "flag2"];
        report <Format>;
    }
}
```

**Syntax `rule!`:**

```rust
rule! {
    name: "Rule Name"
    when amount > 10000 USD
    then flag_aml "large_amount"
}

rule! {
    name: "Location Rule"
    when location in ["IR", "KP"]
    then flag_aml "high_risk_country"
}
```

### 3.5 simbank-reports

**Mục đích:** Xuất báo cáo ở nhiều định dạng.

**Dependencies:**
- `simbank-core`

**Exporters:**

| Exporter | Mô tả |
|----------|-------|
| `CsvExporter` | Xuất CSV |
| `JsonExporter` | Xuất JSON (compact hoặc pretty) |
| `MarkdownExporter` | Xuất Markdown với TOC |

**Report Types:**

| Report | Mô tả |
|--------|-------|
| `AmlReport` | Báo cáo AML với risk score |
| `VelocityReport` | Phân tích tần suất giao dịch |
| `TransactionReport` | Danh sách giao dịch |
| `AccountSummaryReport` | Tổng hợp tài khoản |

### 3.6 simbank-cli

**Mục đích:** Giao diện dòng lệnh.

**Dependencies:**
- All crates
- `clap` - Command-line parsing
- `tokio` - Async runtime

**Commands:**

```
simbank
├── init              # Khởi tạo database
├── status            # Xem trạng thái
├── account
│   ├── create        # Tạo tài khoản
│   ├── list          # Liệt kê tài khoản
│   ├── show          # Xem chi tiết
│   └── balance       # Xem số dư
├── deposit           # Gửi tiền
├── withdraw          # Rút tiền
├── transfer          # Chuyển khoản
├── audit             # Kiểm toán
└── report            # Xuất báo cáo
```

---

## 4. Luồng dữ liệu

### 4.1 Deposit Flow

```
┌─────────┐    ┌──────────────┐    ┌─────────────┐    ┌───────────┐
│   CLI   │───▶│ CustomerSvc  │───▶│ WalletRepo  │───▶│  SQLite   │
└─────────┘    └──────────────┘    └─────────────┘    └───────────┘
                      │                    │
                      │                    ▼
                      │            ┌─────────────┐
                      │            │ BalanceRepo │
                      │            └─────────────┘
                      │                    │
                      ▼                    ▼
               ┌─────────────┐     ┌─────────────┐
               │ EventStore  │───▶ │   JSONL     │
               └─────────────┘     └─────────────┘
```

### 4.2 AML Scan Flow

```
┌─────────┐    ┌──────────────┐    ┌─────────────┐    ┌───────────┐
│   CLI   │───▶│ AuditorSvc   │───▶│ EventReader │───▶│   JSONL   │
└─────────┘    └──────────────┘    └─────────────┘    └───────────┘
                      │
                      ▼
               ┌─────────────┐
               │  RuleSet    │
               │  Evaluate   │
               └─────────────┘
                      │
                      ▼
               ┌─────────────┐    ┌───────────┐
               │ AmlReport   │───▶│  Export   │
               └─────────────┘    └───────────┘
```

---

## 5. Error Handling Strategy

| Crate | Strategy | Error Type |
|-------|----------|------------|
| `core` | `thiserror` enums | `CoreError` |
| `persistence` | `thiserror` wrapping sqlx | `PersistenceError` |
| `business` | `anyhow` for aggregation | `BusinessResult<T>` |
| `dsl` | Compile-time macro errors | N/A |
| `reports` | `thiserror` | `ReportError` |
| `cli` | `anyhow` + colored output | N/A |

---

## 6. Testing Strategy

### Unit Tests

```rust
// Mỗi module có tests riêng
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_money_add() {
        let a = Money::new(dec!(100), Currency::USD);
        let b = Money::new(dec!(50), Currency::USD);
        assert_eq!((a + b).amount(), dec!(150));
    }
}
```

### Integration Tests

```rust
// tests/integration/
#[tokio::test]
async fn test_full_deposit_flow() {
    let ctx = setup_test_context().await;
    let service = CustomerService::new(&ctx);

    let result = service.deposit("actor", "account", dec!(1000), "USD").await;
    assert!(result.is_ok());
}
```

### Test Coverage

| Crate | Tests | Coverage |
|-------|-------|----------|
| `core` | 29 | Domain logic |
| `persistence` | 6 | DB operations |
| `business` | 6 | Service layer |
| `dsl` | 16 | Macro expansion |
| `reports` | 17 | Export formats |
| **Total** | **74** | |

---

## 7. Performance Considerations

### SQLite

- **WAL mode** cho concurrent reads
- **Prepared statements** cho queries lặp lại
- **Connection pooling** qua sqlx

### Event Sourcing

- **Append-only** JSONL cho writes nhanh
- **File rotation** theo ngày
- **Lazy loading** cho event replay

### Decimal Precision

- Sử dụng `rust_decimal` thay vì `f64`
- Lưu dưới dạng TEXT trong SQLite
- Tránh rounding errors

---

## 8. Security Considerations

### AML Compliance

- Ghi log tất cả giao dịch
- Phát hiện patterns đáng ngờ
- Audit trail không thể sửa đổi

### Data Protection

- Không lưu credentials trong code
- Environment variables cho configs
- Prepared statements chống SQL injection

---

## 9. Deployment

### Development

```powershell
cargo build
cargo test
cargo run -p simbank-cli
```

### Production

```powershell
cargo build --release
./target/release/simbank init
./target/release/simbank status
```

### Docker (Future)

```dockerfile
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/simbank /usr/local/bin/
CMD ["simbank", "status"]
```

---

## 10. Mở rộng trong tương lai

| Feature | Mô tả | Priority |
|---------|-------|----------|
| REST API | HTTP endpoints | High |
| Multi-currency | Hỗ trợ nhiều loại tiền | Medium |
| Scheduled jobs | Tự động chạy AML scan | Medium |
| Notifications | Email/SMS alerts | Low |
| Dashboard | Web UI | Low |
