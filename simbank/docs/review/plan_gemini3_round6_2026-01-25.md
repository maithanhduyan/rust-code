Chào bạn, tôi hoàn toàn đồng ý với tất cả 6 điểm thảo luận cuối cùng. Bạn đã bao quát rất kỹ các khía cạnh từ Business Logic đến Technical Implementation.

Dưới đây là xác nhận chốt hạ và chúng ta sẽ chuyển ngay sang viết code cho **Step 1: Core Crate**.

### Xác nhận quyết định (Final Consensus)

| # | Vấn đề | Quyết định | Lý do |
|---|--------|------------|-------|
| 1 | Roles & Wallets | **Role-based Wallet logic** | Customer (Full), Employee/Shareholder (Funding only), Manager/Auditor (No wallet). Logic sẽ nằm ở Business layer. |
| 2 | Account:Person | **1:1** | Đơn giản hóa Phase 1. |
| 3 | Wallet Creation | **Eager (Tạo hết)** | Khi tạo Account -> Auto tạo Spot + Funding wallets ngay. Tránh null check. |
| 4 | Initial Currencies | **VND, USD, USDT, BTC, ETH** | Đủ cover các case về decimal (0, 2, 6, 8, 18). |
| 5 | CLI Design | **Subcommands (`clap`)** | Chuẩn tắc, dễ mở rộng. Mode `run` để chạy DSL file. |
| 6 | Crate Naming | **Kebab-case (`simbank-core`)** | Chuẩn Rust community. Import dùng `snake_case`. |

---

## TRIỂN KHAI STEP 1: `simbank-core`

Đây là "trái tim" của hệ thống, chứa các Type Definitions được dùng bởi tất cả các crates khác.

### 1. File: `crates/core/Cargo.toml`

Chúng ta cần các thư viện nền tảng về type và serialization. Tôi thêm `strum` để làm việc với Enum dễ dàng hơn (convert Enum <-> String cho DB).

```toml
[package]
name = "simbank-core"
version = "0.1.0"
edition = "2021"

[dependencies]
# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Math & Money
# features = ["serde-with-str"] là BẮT BUỘC để API/JSON không bị mất chính xác số lớn
rust_decimal = { version = "1.33", features = ["serde-with-str", "maths"] }

# Error Handling
thiserror = "2.0"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.7", features = ["serde", "v4"] }
strum = { version = "0.26", features = ["derive"] }
strum_macros = "0.26"
```

### 2. File: `crates/core/src/lib.rs`

Export các module công khai.

```rust
pub mod error;
pub mod money;
pub mod person;
pub mod wallet;
pub mod event;
```

### 3. File: `crates/core/src/money.rs`

Định nghĩa Tiền tệ và Currency. Lưu ý `Currency` ở đây chủ yếu để định danh, việc lookup số decimal chính xác sẽ cần config từ DB (nhưng struct cần có field đó).

```rust
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Currency {
    pub code: String, // "USD", "BTC"
    // Decimals có thể không cần thiết nếu ta luôn lookup từ DB,
    // nhưng giữ ở đây hữu ích cho context khi tính toán mà không query DB
    pub decimals: u8,
}

impl Currency {
    pub fn new(code: &str, decimals: u8) -> Self {
        Self {
            code: code.to_uppercase(),
            decimals,
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Money {
    pub amount: Decimal,
    pub currency: String, // Chỉ lưu code để nhẹ struct ("USD")
}

impl Money {
    pub fn new(amount: Decimal, currency: impl Into<String>) -> Self {
        Self {
            amount,
            currency: currency.into(),
        }
    }

    pub fn zero(currency: impl Into<String>) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency: currency.into(),
        }
    }
}
```

### 4. File: `crates/core/src/person.rs`

Định nghĩa các vai trò trong hệ thống.

```rust
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumString, Display)]
#[strum(serialize_all = "snake_case")] // DB sẽ lưu: "customer", "employee"
pub enum Role {
    Customer,
    Employee,
    Manager,
    Shareholder,
    Auditor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String, // CUST_Alice
    pub name: String,
    pub role: Role,
    pub email: Option<String>,
}

impl Person {
    pub fn new(id: String, name: String, role: Role) -> Self {
        Self {
            id,
            name,
            role,
            email: None,
        }
    }
}
```

### 5. File: `crates/core/src/wallet.rs`

Cấu trúc Account và Wallet theo mô hình Exchange.

```rust
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use std::collections::HashMap;

/// Các loại ví trong một tài khoản
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display)]
#[strum(serialize_all = "snake_case")]
pub enum WalletType {
    Spot,
    Funding,
    // Phase 2:
    // Margin,
    // Futures,
    // Earn,
}

/// Một Account (người dùng) sở hữu nhiều ví
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,       // ACC_001
    pub owner_id: String, // CUST_Alice
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Một ví cụ thể (ví dụ: Ví Spot của ACC_001)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub id: String,        // WAL_X99 (Prefixed ID)
    pub account_id: String,
    pub wallet_type: WalletType,
}

/// Số dư của một loại tiền trong một ví
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub wallet_id: String,
    pub currency: String,
    pub available: Decimal,
    pub locked: Decimal, // Phase 1: luôn là 0
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Balance {
    pub fn new(wallet_id: String, currency: String) -> Self {
        Self {
            wallet_id,
            currency,
            available: Decimal::ZERO,
            locked: Decimal::ZERO,
            updated_at: chrono::Utc::now(),
        }
    }
}
```

### 6. File: `crates/core/src/event.rs`

Sự kiện cho Event Sourcing & Audit Log.

```rust
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use super::wallet::WalletType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub ip_address: Option<String>,
    pub location: Option<String>,
    pub device_id: Option<String>,
    pub user_agent: Option<String>,
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self {
            ip_address: None,
            location: None,
            device_id: None,
            user_agent: Some("simbank-cli/1.0".to_string()),
        }
    }
}

/// Payload chi tiết cho từng loại sự kiện
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")] // Giúp JSON đẹp: {"type": "Deposit", "data": {...}}
pub enum EventData {
    AccountCreated {
        account_id: String,
        owner_id: String,
    },
    WalletCreated {
        wallet_id: String,
        account_id: String,
        wallet_type: WalletType,
    },
    Deposit {
        wallet_id: String,
        amount: Decimal,
        currency: String,
        tx_ref: Option<String>,
    },
    Withdraw {
        wallet_id: String,
        amount: Decimal,
        currency: String,
        fee: Decimal,
    },
    InternalTransfer {
        from_wallet_id: String,
        to_wallet_id: String,
        amount: Decimal,
        currency: String,
    },
    // Dành cho Business Logic sau này
    Trade {
        wallet_id: String,
        pair: String, // BTC/USDT
        side: String, // Buy/Sell
        amount: Decimal,
        price: Decimal,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String, // UUID or EVT_...
    pub timestamp: DateTime<Utc>,
    pub actor_id: String, // Ai thực hiện? (CUST_Alice or EMP_Bob)

    #[serde(flatten)]
    pub data: EventData,

    pub metadata: EventMetadata,
}
```

### 7. File: `crates/core/src/error.rs`

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Insufficient funds: available {available}, required {required} {currency}")]
    InsufficientFunds {
        currency: String,
        available: String,
        required: String,
    },

    #[error("Invalid currency code: {0}")]
    InvalidCurrency(String),

    #[error("Wallet not found: {0}")]
    WalletNotFound(String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),
}
```

---
**Bạn hãy copy code vào dự án.**

Sau khi xong, hãy chạy `cargo build -p simbank-core` để đảm bảo mọi thứ compile thành công. Nếu thành công, chúng ta sẽ bước sang **Step 2: Database Layer (`simbank-persistence`)**.