# SIMBANK 🏦

> **Ứng dụng ngân hàng mô phỏng với DSL (Domain Specific Language) trong Rust**

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/Tests-74%20passing-green.svg)]()

---

## 📋 Tổng quan

SIMBANK là ứng dụng ngân hàng/sàn giao dịch đơn giản được xây dựng để minh họa:

- ✅ **DSL (Domain Specific Language)** - Ngôn ngữ đặc tả miền cho nghiệp vụ ngân hàng
- ✅ **Event Sourcing** - SQLite (current state) + JSONL (audit trail)
- ✅ **AML Compliance** - Phát hiện giao dịch đáng ngờ theo quy định chống rửa tiền

### 🎯 Đối tượng sử dụng

| Vai trò | Mô tả |
|---------|-------|
| **Business Analyst** | Định nghĩa quy tắc kinh doanh bằng DSL |
| **Developer** | Tích hợp DSL vào ứng dụng |
| **Auditor** | Kiểm toán giao dịch và báo cáo AML |

---

## 🚀 Bắt đầu nhanh

### Yêu cầu hệ thống

- Rust 1.75 hoặc cao hơn
- SQLite 3.x

### Cài đặt và chạy

```powershell
# Clone repository
git clone <repository-url>
cd simbank

# Build toàn bộ workspace
cargo build

# Chạy tests (74 tests)
cargo test

# Khởi tạo database và chạy CLI
cargo run -p simbank-cli -- init
cargo run -p simbank-cli -- status
```

### Chạy ví dụ

```powershell
# Ví dụ 1: Quy trình khách hàng
cargo run -p simbank-examples --example 01_customer_onboarding

# Ví dụ 2: Nghiệp vụ nhân viên
cargo run -p simbank-examples --example 02_employee_operations

# Ví dụ 3: Chia cổ tức
cargo run -p simbank-examples --example 03_shareholder_dividends

# Ví dụ 4: Kiểm toán AML
cargo run -p simbank-examples --example 04_auditor_aml_scan

# Ví dụ 5: Kịch bản đa bên liên quan
cargo run -p simbank-examples --example 05_complex_scenario
```

---

## 📖 DSL - Ngôn ngữ đặc tả miền

SIMBANK sử dụng macro Rust để định nghĩa DSL thân thiện với người dùng nghiệp vụ.

### banking_scenario! - Định nghĩa kịch bản

```rust
use simbank_dsl::banking_scenario;

let scenario = banking_scenario! {
    // Khách hàng gửi tiền và chuyển khoản
    Customer "Nguyễn Văn A" {
        deposit 10000 USD to Funding;
        transfer 5000 USD from Funding to Spot;
        withdraw 2000 USD from Funding;
    }

    // Nhân viên nhận lương
    Employee "Trần Thị B" {
        receive_salary 8000 USD;
        buy_insurance "Premium Health" for 500 USD;
    }

    // Cổ đông nhận cổ tức
    Shareholder "Công ty ABC" {
        receive_dividend 50000 USD;
    }

    // Kiểm toán viên quét giao dịch
    Auditor "Deloitte" {
        scan from "2025-01-01" to "2025-12-31" flags ["large_amount"];
        report Markdown;
    }
};
```

### rule! - Định nghĩa quy tắc AML

```rust
use simbank_dsl::rule;

// Quy tắc phát hiện giao dịch lớn
let aml_rule = rule! {
    name: "Large Transaction"
    when amount > 10000 USD
    then flag_aml "large_amount"
};

// Quy tắc yêu cầu phê duyệt
let approval_rule = rule! {
    name: "Withdrawal Limit"
    when amount > 50000 USD
    then require_approval
};

// Quy tắc quốc gia rủi ro cao
let country_rule = rule! {
    name: "High Risk Country"
    when location in ["IR", "KP", "SY"]
    then flag_aml "high_risk_country"
};
```

---

## 🏗️ Kiến trúc dự án

```
simbank/
├── crates/
│   ├── core/           # Domain types (Money, Wallet, Person, Event)
│   ├── persistence/    # SQLite repos + JSONL EventStore
│   ├── business/       # Services (Customer, Employee, Auditor)
│   ├── dsl/            # DSL macros (banking_scenario!, rule!)
│   ├── reports/        # Report exporters (CSV, JSON, Markdown)
│   └── cli/            # Command-line interface
│
├── examples/           # 5 ví dụ minh họa DSL
├── migrations/         # SQLite migrations
├── data/               # Runtime data (gitignored)
└── docs/               # Tài liệu
```

### Luồng phụ thuộc (Dependency Graph)

```
core → persistence → business → dsl/reports → cli
```

| Crate | Mô tả | Dependencies |
|-------|-------|--------------|
| `simbank-core` | Domain types thuần túy | serde, rust_decimal, thiserror |
| `simbank-persistence` | Lớp lưu trữ dữ liệu | core, sqlx, serde_json |
| `simbank-business` | Lớp nghiệp vụ | core, persistence |
| `simbank-dsl` | Macro DSL | core, business |
| `simbank-reports` | Xuất báo cáo | core |
| `simbank-cli` | Giao diện dòng lệnh | all crates |

---

## 💼 Các loại người dùng (Person Types)

| Loại | Có ví tiền | Quyền hạn |
|------|------------|-----------|
| **Customer** | Spot + Funding | deposit, withdraw, transfer |
| **Employee** | Funding | receive_salary, buy_insurance |
| **Shareholder** | Funding | receive_dividend |
| **Manager** | Không | approve, pay_salary, pay_bonus, pay_dividend |
| **Auditor** | Không | scan_transactions, generate_report |

---

## 🔧 CLI Commands

```powershell
# Khởi tạo database
simbank init

# Xem trạng thái hệ thống
simbank status

# Quản lý tài khoản
simbank account create --name "Nguyễn Văn A" --type customer
simbank account list
simbank account show ACC_001
simbank account balance ACC_001

# Giao dịch
simbank deposit ACC_001 10000 USD
simbank withdraw ACC_001 5000 USD
simbank transfer ACC_001 ACC_002 3000 USD

# Kiểm toán
simbank audit --from 2025-01-01 --to 2025-12-31

# Báo cáo
simbank report transactions --format csv
simbank report aml --format markdown
```

---

## 📊 AML Compliance

SIMBANK tích hợp các quy tắc chống rửa tiền (Anti-Money Laundering):

| Flag | Ngưỡng | Mô tả |
|------|--------|-------|
| `large_amount` | > $10,000 | Giao dịch lớn, cần báo cáo CTR |
| `near_threshold` | $9,000 - $10,000 | Nghi ngờ chia nhỏ giao dịch |
| `high_risk_country` | IR, KP, SY, CU | Quốc gia bị cấm vận |
| `unusual_pattern` | Varies | Mẫu giao dịch bất thường |
| `cross_border` | International | Giao dịch xuyên biên giới |

---

## 📁 Cấu trúc dữ liệu

### SQLite Tables

```sql
-- Người dùng
CREATE TABLE persons (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    person_type TEXT NOT NULL,
    email TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Tài khoản
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    person_id TEXT NOT NULL,
    status TEXT DEFAULT 'active',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Ví tiền
CREATE TABLE wallets (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    wallet_type TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Số dư theo loại tiền
CREATE TABLE balances (
    wallet_id TEXT,
    currency_code TEXT,
    available TEXT DEFAULT '0',
    locked TEXT DEFAULT '0',
    PRIMARY KEY (wallet_id, currency_code)
);

-- Lịch sử giao dịch
CREATE TABLE transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    wallet_id TEXT NOT NULL,
    tx_type TEXT NOT NULL,
    amount TEXT NOT NULL,
    currency_code TEXT NOT NULL,
    description TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### JSONL Event Format

```json
{"id":"EVT_001","event_type":"Deposit","actor_id":"CUST_001","actor_type":"Customer","account_id":"ACC_001","amount":"10000","currency":"USD","timestamp":"2025-01-25T10:30:00Z","aml_flags":["large_amount"]}
```

---

## 🧪 Testing

```powershell
# Chạy tất cả tests
cargo test

# Chạy tests cho từng crate
cargo test -p simbank-core       # 29 tests
cargo test -p simbank-persistence # 6 tests
cargo test -p simbank-business   # 6 tests
cargo test -p simbank-dsl        # 16 tests
cargo test -p simbank-reports    # 17 tests

# Chạy với output chi tiết
cargo test -- --nocapture
```

---

## 📚 Tài liệu bổ sung

| Tài liệu | Mô tả |
|----------|-------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Kiến trúc hệ thống chi tiết |
| [docs/DSL_GUIDE.md](docs/DSL_GUIDE.md) | Hướng dẫn sử dụng DSL |
| [docs/CLI_GUIDE.md](docs/CLI_GUIDE.md) | Hướng dẫn sử dụng CLI |
| [docs/DEVELOPER.md](docs/DEVELOPER.md) | Hướng dẫn dành cho nhà phát triển |

---

## 📄 License

MIT License - Xem file [LICENSE](LICENSE) để biết thêm chi tiết.

---

## 🤝 Đóng góp

1. Fork repository
2. Tạo branch mới (`git checkout -b feature/amazing-feature`)
3. Commit thay đổi (`git commit -m 'Add amazing feature'`)
4. Push lên branch (`git push origin feature/amazing-feature`)
5. Tạo Pull Request

---

> **Lưu ý:** DSL viết bằng tiếng Anh, comment và tài liệu viết bằng tiếng Việt.