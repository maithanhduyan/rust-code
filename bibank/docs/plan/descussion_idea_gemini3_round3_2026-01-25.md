Tuyệt vời. Bạn đã bắt đúng "mạch" của hệ thống này. Những điểm bạn chỉ ra chính là sự khác biệt giữa một hệ thống "chạy được" và một hệ thống "Core Banking chuẩn mực".

Tôi **đồng ý hoàn toàn** với 3 đề xuất chốt của bạn. Dưới đây là xác nhận cuối cùng để khóa thiết kế (Design Lock).

---

### 🔒 FINAL DESIGN DECISIONS (10 PILLARS)

| # | Hạng mục | Quyết định (Specs) | Ghi chú |
|---|----------|--------------------|---------|
| 1 | **Event Store** | JSONL (Append-only) + SQLite (Read Model) | JSONL là Source of Truth. |
| 2 | **Accounting** | Double-Entry (Zero-sum per asset) | `Sum(Debit) == Sum(Credit)` |
| 3 | **Direction** | `Side::Debit` / `Side::Credit` | Explicit enum, không dùng số âm. |
| 4 | **Structure** | Multi-asset per Entry | Trade = 1 Atomic Entry (USD & BTC). |
| 5 | **Security** | Hash Chain (SHA256) | `hash = sha256(prev + data)`. |
| 6 | **Ordering** | Derived Sequence | Restart đọc file tail để lấy next seq. |
| 7 | **Validation** | Risk Engine (Pre-commit) | Chặn ngay cửa Ledger. |
| 8 | **Account ID** | **Hierarchical Hybrid** | Format: `CAT:SEGMENT:ID:ASSET:TYPE` <br>VD: `LIAB:USER:alice:USDT:available` |
| 9 | **Semantics** | **Intent-aware** | Entry phải có `intent: Trade/Deposit/...`. |
| 10| **Risk State** | **In-Memory / Event-Derived** | Không phụ thuộc SQLite Read Model. |

---

## 🚀 KICKOFF STEP 1: Foundation & Ledger Core

Chúng ta sẽ bắt đầu xây dựng **BiBank Workspace** và module quan trọng nhất: **`bibank-ledger`**.

### Nhiệm vụ Step 1:
1.  Khởi tạo Rust Workspace với 8 crates.
2.  **`bibank-core`**: Định nghĩa `Money`, `Currency` (Lấy từ Simbank, giữ lại `rust_decimal`).
3.  **`bibank-ledger`**:
    *   Định nghĩa `AccountKey` (Parsing chuỗi `LIAB:USER...`).
    *   Định nghĩa `Posting`, `JournalEntry`.
    *   Implement logic `validate_double_entry` (Multi-asset aware).

Tôi sẽ cung cấp code cho **Step 1** ngay bây giờ.

### File: `Cargo.toml` (Workspace Root)

```toml
[workspace]
members = [
    "crates/core",
    "crates/ledger",
    "crates/risk",
    "crates/events",
    "crates/bus",
    "crates/projection",
    "crates/rpc",
    "crates/dsl",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rust_decimal = { version = "1.33", features = ["serde-with-str", "maths"] }
thiserror = "2.0"
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
hex = "0.4"
strum = { version = "0.26", features = ["derive"] }
strum_macros = "0.26"
tokio = { version = "1.36", features = ["full"] }
tracing = "0.1"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono", "uuid"] }

# Internal
bibank-core = { path = "./crates/core" }
bibank-ledger = { path = "./crates/ledger" }
```

---

### File: `crates/core/src/lib.rs` (Types cơ bản)

Chúng ta giữ lại `Currency` và `Money` nhưng tinh gọn hơn.

```rust
// crates/core/src/lib.rs
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub mod amount;
pub use amount::Amount;

// Thay vì struct Money phức tạp, ở Core Ledger chúng ta chỉ cần Amount dương
// Money (Amount + Currency) sẽ được ghép ở tầng Posting
```

```rust
// crates/core/src/amount.rs
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AmountError {
    #[error("Amount cannot be negative: {0}")]
    NegativeAmount(Decimal),
}

/// Wrapper around Decimal to enforce non-negative values in Ledger
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Amount(Decimal);

impl Amount {
    pub const ZERO: Self = Self(Decimal::ZERO);

    pub fn new(value: Decimal) -> Result<Self, AmountError> {
        if value < Decimal::ZERO {
            Err(AmountError::NegativeAmount(value))
        } else {
            Ok(Self(value))
        }
    }

    // Helper for testing/trusted sources
    pub fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    pub fn value(&self) -> Decimal {
        self.0
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

---

### File: `crates/ledger/src/lib.rs` (The Core Logic)

Đây là nơi chứa logic Double-Entry Hardcore.

```rust
// crates/ledger/src/lib.rs
pub mod account;
pub mod entry;
pub mod error;

pub use account::{AccountKey, AccountCategory, AccountSide};
pub use entry::{JournalEntry, Posting, TransactionIntent};
pub use error::LedgerError;
```

#### 1. `crates/ledger/src/account.rs` (Hierarchical Account ID)

```rust
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use std::fmt;
use std::str::FromStr;
use crate::error::LedgerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountCategory {
    Asset,      // Tài sản (Cash, Receivables)
    Liability,  // Nợ phải trả (User Balances)
    Equity,     // Vốn chủ sở hữu
    Revenue,    // Doanh thu (Fees)
    Expense,    // Chi phí
}

impl AccountCategory {
    /// Normal balance side (Tăng bên nào?)
    pub fn normal_balance(&self) -> AccountSide {
        match self {
            AccountCategory::Asset | AccountCategory::Expense => AccountSide::Debit,
            AccountCategory::Liability | AccountCategory::Equity | AccountCategory::Revenue => AccountSide::Credit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccountSide {
    Debit,
    Credit,
}

/// Ledger Account Key: CAT:SEGMENT:ID:ASSET:TYPE
/// Example: LIAB:USER:alice:USD:AVAILABLE
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountKey {
    pub category: AccountCategory,
    pub segment: String, // "USER", "SYSTEM"
    pub id: String,      // "alice", "VAULT"
    pub asset: String,   // "USD", "BTC"
    pub sub_account: String, // "AVAILABLE", "LOCKED"
}

impl fmt::Display for AccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}:{}:{}",
            self.category, self.segment, self.id, self.asset, self.sub_account)
    }
}

impl FromStr for AccountKey {
    type Err = LedgerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 5 {
            return Err(LedgerError::InvalidAccountFormat(s.to_string()));
        }

        Ok(AccountKey {
            category: AccountCategory::from_str(parts[0])
                .map_err(|_| LedgerError::InvalidAccountFormat(format!("Unknown category: {}", parts[0])))?,
            segment: parts[1].to_string(),
            id: parts[2].to_string(),
            asset: parts[3].to_string(),
            sub_account: parts[4].to_string(),
        })
    }
}
```

#### 2. `crates/ledger/src/entry.rs` (Journal & Validation)

```rust
use bibank_core::Amount;
use crate::account::{AccountKey, AccountSide};
use crate::error::LedgerError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionIntent {
    Deposit,
    Withdrawal,
    Transfer,
    Trade,
    Fee,
    Correction,
    // System initialization
    Genesis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    pub account: AccountKey,
    pub amount: Amount,
    pub side: AccountSide,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub sequence: u64,
    pub prev_hash: String,
    pub hash: String, // Will be calculated
    pub timestamp: DateTime<Utc>,

    // Semantic fields
    pub intent: TransactionIntent,
    pub correlation_id: String, // UUID tracking request

    pub postings: Vec<Posting>,
}

impl JournalEntry {
    /// Validate Zero-Sum per Asset Group
    pub fn validate(&self) -> Result<(), LedgerError> {
        if self.postings.is_empty() {
            return Err(LedgerError::EmptyEntry);
        }

        // Group by Asset
        let mut sums: HashMap<String, Decimal> = HashMap::new();

        for posting in &self.postings {
            let asset = &posting.account.asset;
            let current = sums.entry(asset.clone()).or_insert(Decimal::ZERO);

            match posting.side {
                AccountSide::Debit => *current += posting.amount.value(),
                AccountSide::Credit => *current -= posting.amount.value(),
            }
        }

        // Check if all sums are zero
        for (asset, sum) in sums {
            if !sum.is_zero() {
                return Err(LedgerError::UnbalancedEntry {
                    asset,
                    imbalance: sum,
                });
            }
        }

        Ok(())
    }
}
```

#### 3. `crates/ledger/src/error.rs`

```rust
use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerError {
    #[error("Invalid account format: {0}")]
    InvalidAccountFormat(String),

    #[error("Entry must have at least one posting")]
    EmptyEntry,

    #[error("Entry unbalanced for asset {asset}: imbalance {imbalance}")]
    UnbalancedEntry {
        asset: String,
        imbalance: Decimal,
    },
}
```

---

### Hành động của bạn:
1.  Tạo folder `bibank`.
2.  Tạo file `Cargo.toml`.
3.  Chạy `cargo new --lib crates/core`, `crates/ledger`... (cho đủ 8 crates).
4.  Copy code trên vào `crates/core` và `crates/ledger`.
5.  Thêm dependencies vào `crates/*/Cargo.toml` tương ứng.

Sau khi bạn thực hiện xong **Step 1**, hãy gõ **"Check Step 1"** (hoặc paste lỗi nếu có) để tôi review. Sau đó chúng ta sẽ sang **Step 2: Risk Engine Gatekeeper**.