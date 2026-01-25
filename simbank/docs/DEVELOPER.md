# Hướng dẫn dành cho nhà phát triển 👨‍💻

> Tài liệu dành cho developers muốn đóng góp hoặc mở rộng SIMBANK

---

## 1. Thiết lập môi trường phát triển

### 1.1 Yêu cầu

| Tool | Version | Mục đích |
|------|---------|----------|
| Rust | 1.75+ | Compiler |
| Cargo | 1.75+ | Build tool |
| SQLite | 3.x | Database |
| Git | 2.x | Version control |

### 1.2 Clone và build

```powershell
# Clone repository
git clone <repository-url>
cd simbank

# Build tất cả crates
cargo build

# Chạy tests
cargo test

# Build release
cargo build --release
```

### 1.3 IDE Recommendations

**VS Code Extensions:**
- rust-analyzer
- crates
- Even Better TOML
- CodeLLDB (debugging)

**Settings (`.vscode/settings.json`):**
```json
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy",
    "editor.formatOnSave": true
}
```

---

## 2. Cấu trúc dự án

### 2.1 Workspace layout

```
simbank/
├── Cargo.toml              # Workspace definition
├── crates/
│   ├── core/               # Domain layer
│   ├── persistence/        # Data access layer
│   ├── business/           # Service layer
│   ├── dsl/                # DSL macros
│   ├── reports/            # Report generation
│   └── cli/                # CLI application
├── examples/               # Example scenarios
├── migrations/             # SQL migrations
└── docs/                   # Documentation
```

### 2.2 Dependency rules

```
                    ┌─────────┐
                    │   CLI   │
                    └────┬────┘
                         │
        ┌────────────────┼────────────────┐
        ▼                ▼                ▼
    ┌───────┐      ┌─────────┐      ┌─────────┐
    │  DSL  │      │ Reports │      │ Business│
    └───┬───┘      └────┬────┘      └────┬────┘
        │               │                │
        └───────────────┼────────────────┘
                        ▼
                ┌───────────────┐
                │  Persistence  │
                └───────┬───────┘
                        ▼
                    ┌──────┐
                    │ Core │
                    └──────┘
```

**Quy tắc:**
- Core không depend vào bất kỳ crate nào khác
- Persistence chỉ depend vào Core
- Business depend vào Core và Persistence
- DSL/Reports depend vào Core và Business
- CLI depend vào tất cả

---

## 3. Coding Conventions

### 3.1 Rust Style

```rust
// ✅ Tốt - sử dụng thiserror cho errors
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Currency mismatch: expected {expected}, got {actual}")]
    CurrencyMismatch { expected: String, actual: String },
}

// ✅ Tốt - sử dụng builder pattern
pub struct PersonBuilder {
    name: Option<String>,
    person_type: Option<PersonType>,
}

impl PersonBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn build(self) -> Result<Person, CoreError> {
        Ok(Person {
            name: self.name.ok_or(CoreError::MissingField("name"))?,
            // ...
        })
    }
}
```

### 3.2 Naming conventions

| Loại | Convention | Ví dụ |
|------|------------|-------|
| Modules | snake_case | `account.rs`, `aml_report.rs` |
| Types | PascalCase | `Account`, `WalletType` |
| Functions | snake_case | `get_balance()`, `check_aml()` |
| Constants | SCREAMING_SNAKE_CASE | `MAX_AMOUNT`, `DEFAULT_CURRENCY` |
| Traits | PascalCase | `Repository`, `Exporter` |

### 3.3 Documentation

```rust
/// Đại diện một tài khoản ngân hàng.
///
/// Account chứa thông tin người dùng và các ví tiền liên quan.
///
/// # Examples
///
/// ```rust
/// use simbank_core::Account;
///
/// let account = Account::new("Alice", PersonType::Customer);
/// assert!(account.is_active());
/// ```
///
/// # Errors
///
/// Trả về `CoreError::InvalidName` nếu tên trống.
pub struct Account {
    /// ID duy nhất của tài khoản
    pub id: String,

    /// ID của người sở hữu
    pub person_id: String,

    /// Trạng thái tài khoản
    pub status: AccountStatus,
}
```

---

## 4. Testing

### 4.1 Unit Tests

```rust
// Đặt trong cùng file với code
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_money_add() {
        let a = Money::new(dec!(100), Currency::USD);
        let b = Money::new(dec!(50), Currency::USD);

        let result = a + b;

        assert_eq!(result.amount(), dec!(150));
        assert_eq!(result.currency(), Currency::USD);
    }

    #[test]
    #[should_panic(expected = "Currency mismatch")]
    fn test_money_add_different_currencies_panics() {
        let a = Money::new(dec!(100), Currency::USD);
        let b = Money::new(dec!(50), Currency::EUR);

        let _ = a + b; // Should panic
    }
}
```

### 4.2 Integration Tests

```rust
// tests/integration/customer_flow.rs
use simbank_business::CustomerService;
use simbank_persistence::SqlitePool;
use tempfile::tempdir;

#[tokio::test]
async fn test_customer_deposit_flow() {
    // Setup
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = SqlitePool::connect(&db_path).await.unwrap();

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Create service
    let ctx = ServiceContext::new(pool);
    let service = CustomerService::new(&ctx);

    // Test
    let result = service
        .deposit("actor_id", "account_id", dec!(1000), "USD")
        .await;

    // Assert
    assert!(result.is_ok());
    let tx_result = result.unwrap();
    assert_eq!(tx_result.amount, dec!(1000));
}
```

### 4.3 Chạy tests

```powershell
# Tất cả tests
cargo test

# Tests cho một crate
cargo test -p simbank-core

# Test cụ thể
cargo test test_money_add

# Với output
cargo test -- --nocapture

# Parallel tests
cargo test -- --test-threads=4
```

---

## 5. Thêm feature mới

### 5.1 Thêm Person Type mới

**Bước 1: Cập nhật Core**

```rust
// crates/core/src/person.rs
#[derive(Debug, Clone, PartialEq)]
pub enum PersonType {
    Customer,
    Employee,
    Shareholder,
    Manager,
    Auditor,
    // Thêm mới
    Partner,  // Đối tác kinh doanh
}

impl PersonType {
    pub fn has_wallet(&self) -> bool {
        matches!(self,
            PersonType::Customer |
            PersonType::Employee |
            PersonType::Shareholder |
            PersonType::Partner  // Thêm Partner
        )
    }
}
```

**Bước 2: Cập nhật Persistence**

```rust
// crates/persistence/src/sqlite/repos.rs
impl PersonRepo {
    fn person_type_to_str(pt: &PersonType) -> &'static str {
        match pt {
            PersonType::Customer => "customer",
            PersonType::Employee => "employee",
            PersonType::Shareholder => "shareholder",
            PersonType::Manager => "manager",
            PersonType::Auditor => "auditor",
            PersonType::Partner => "partner",  // Thêm mapping
        }
    }
}
```

**Bước 3: Thêm Service**

```rust
// crates/business/src/partner.rs
pub struct PartnerService<'a> {
    ctx: &'a ServiceContext,
}

impl<'a> PartnerService<'a> {
    pub fn new(ctx: &'a ServiceContext) -> Self {
        Self { ctx }
    }

    pub async fn receive_commission(
        &self,
        actor_id: &str,
        account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> BusinessResult<TransactionResult> {
        // Implementation
    }
}
```

**Bước 4: Cập nhật DSL**

```rust
// crates/dsl/src/lib.rs

// Thêm enum
pub enum PartnerOp {
    ReceiveCommission { amount: Decimal, currency: String },
}

// Thêm macro rules
(@block $builder:expr, Partner, $name:literal, $($op:tt)*) => {{
    let mut ops = Vec::new();
    $crate::banking_scenario!(@partner_ops ops, $($op)*);
    $builder.partner($name, ops)
}};
```

**Bước 5: Thêm Tests**

```rust
#[test]
fn test_partner_operations() {
    let scenario = banking_scenario! {
        Partner "ABC Corp" {
            receive_commission 5000 USD;
        }
    };

    assert_eq!(scenario.partners().count(), 1);
}
```

### 5.2 Thêm Event Type mới

```rust
// crates/core/src/event.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    // Existing
    Deposit,
    Withdrawal,
    InternalTransfer,
    ExternalTransfer,

    // Thêm mới
    Commission,      // Hoa hồng
    Fee,             // Phí
    Interest,        // Lãi suất
}

impl Event {
    /// Tạo event Commission
    pub fn commission(
        id: String,
        actor_id: String,
        account_id: String,
        amount: Decimal,
        currency: &str,
    ) -> Self {
        Self {
            id,
            event_type: EventType::Commission,
            actor_id,
            actor_type: PersonType::Partner,
            account_id,
            amount: Some(amount),
            currency: Some(currency.to_string()),
            timestamp: Utc::now(),
            aml_flags: vec![],
            metadata: None,
        }
    }
}
```

### 5.3 Thêm Rule Condition mới

```rust
// crates/dsl/src/rules.rs
#[derive(Debug, Clone)]
pub enum RuleCondition {
    // Existing
    AmountGreaterThan { threshold: Decimal, currency: String },
    LocationIn { countries: Vec<String> },

    // Thêm mới
    FrequencyExceeds { count: u32, period: Duration },
    AccountAgeBelow { days: u32 },
    CumulativeAmountExceeds { threshold: Decimal, currency: String, period: Duration },
}

impl RuleCondition {
    pub fn evaluate(&self, ctx: &TransactionContext) -> bool {
        match self {
            // ... existing matches

            RuleCondition::AccountAgeBelow { days } => {
                ctx.account_age_days
                    .map(|age| age < *days)
                    .unwrap_or(false)
            }
        }
    }
}
```

---

## 6. Database Migrations

### 6.1 Tạo migration mới

```powershell
# Tạo file migration
New-Item -Path "migrations/20260126_add_partners.sql" -ItemType File
```

### 6.2 Viết migration

```sql
-- migrations/20260126_add_partners.sql

-- Thêm cột cho partners
ALTER TABLE persons ADD COLUMN partner_code TEXT;
ALTER TABLE persons ADD COLUMN commission_rate TEXT DEFAULT '0.00';

-- Tạo bảng commissions
CREATE TABLE IF NOT EXISTS commissions (
    id TEXT PRIMARY KEY,
    partner_id TEXT NOT NULL REFERENCES persons(id),
    amount TEXT NOT NULL,
    currency_code TEXT NOT NULL,
    source_transaction_id TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Index
CREATE INDEX idx_commissions_partner ON commissions(partner_id);
```

### 6.3 Chạy migrations

```rust
// Tự động khi khởi tạo
sqlx::migrate!("./migrations")
    .run(&pool)
    .await?;
```

---

## 7. Error Handling

### 7.1 Định nghĩa errors

```rust
// Core errors - thiserror
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Wallet not found: {0}")]
    WalletNotFound(String),
}

// Persistence errors - wrap sqlx
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Record not found: {0}")]
    NotFound(String),
}

// Business errors - anyhow
pub type BusinessResult<T> = anyhow::Result<T>;
```

### 7.2 Propagating errors

```rust
// Sử dụng ? operator
pub async fn deposit(&self, amount: Decimal) -> BusinessResult<()> {
    let wallet = WalletRepo::get_by_id(self.pool, &self.wallet_id)
        .await?  // PersistenceError -> anyhow
        .ok_or_else(|| anyhow!("Wallet not found"))?;

    BalanceRepo::credit(self.pool, &wallet.id, amount)
        .await?;  // PersistenceError -> anyhow

    Ok(())
}
```

---

## 8. Logging

### 8.1 Setup tracing

```rust
// CLI setup
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn setup_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "simbank=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
```

### 8.2 Sử dụng trong code

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self))]
pub async fn deposit(
    &self,
    actor_id: &str,
    account_id: &str,
    amount: Decimal,
    currency: &str,
) -> BusinessResult<TransactionResult> {
    info!(actor_id, account_id, %amount, currency, "Processing deposit");

    // ... implementation

    if amount > dec!(10000) {
        warn!(%amount, "Large transaction detected");
    }

    debug!("Deposit completed successfully");
    Ok(result)
}
```

---

## 9. Performance Tips

### 9.1 Database

```rust
// ✅ Batch operations
pub async fn insert_many(pool: &SqlitePool, items: &[Item]) -> Result<()> {
    let mut tx = pool.begin().await?;

    for item in items {
        sqlx::query("INSERT INTO items VALUES (?)")
            .bind(&item.value)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

// ✅ Prepared statements
static INSERT_QUERY: &str = "INSERT INTO items (id, value) VALUES (?, ?)";

pub async fn insert(pool: &SqlitePool, item: &Item) -> Result<()> {
    sqlx::query(INSERT_QUERY)
        .bind(&item.id)
        .bind(&item.value)
        .execute(pool)
        .await?;
    Ok(())
}
```

### 9.2 Memory

```rust
// ✅ Stream thay vì collect all
use futures::StreamExt;

pub async fn process_events(reader: &EventReader) -> Result<()> {
    let mut stream = reader.stream_events();

    while let Some(event) = stream.next().await {
        process_single_event(event?).await?;
    }

    Ok(())
}
```

---

## 10. Release Checklist

- [ ] Tất cả tests pass (`cargo test`)
- [ ] Clippy không có warnings (`cargo clippy`)
- [ ] Format đúng (`cargo fmt --check`)
- [ ] Documentation build thành công (`cargo doc`)
- [ ] Examples chạy được
- [ ] CHANGELOG.md được cập nhật
- [ ] Version bump trong Cargo.toml
- [ ] Git tag được tạo

```powershell
# Pre-release script
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo doc --no-deps
cargo build --release --examples
```

---

## 11. Tài nguyên

| Tài liệu | Link |
|----------|------|
| Rust Book | https://doc.rust-lang.org/book/ |
| Async Rust | https://rust-lang.github.io/async-book/ |
| SQLx | https://github.com/launchbadge/sqlx |
| Clap | https://docs.rs/clap/latest/clap/ |
| rust_decimal | https://docs.rs/rust_decimal/ |
