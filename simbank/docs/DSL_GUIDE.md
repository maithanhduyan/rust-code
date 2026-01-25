# Hướng dẫn sử dụng DSL 📖

> Tài liệu hướng dẫn chi tiết cách sử dụng Domain Specific Language (DSL) trong SIMBANK

---

## 1. Giới thiệu DSL

### DSL là gì?

**Domain Specific Language (DSL)** là ngôn ngữ được thiết kế riêng cho một lĩnh vực cụ thể. Trong SIMBANK, DSL giúp Business Analyst và các chuyên gia nghiệp vụ định nghĩa:

- Kịch bản ngân hàng (banking scenarios)
- Quy tắc kinh doanh (business rules)
- Quy tắc tuân thủ AML (compliance rules)

### Tại sao dùng DSL?

| Ưu điểm | Mô tả |
|---------|-------|
| **Dễ đọc** | Cú pháp gần với ngôn ngữ tự nhiên |
| **An toàn** | Kiểm tra lỗi tại compile-time |
| **Hiệu quả** | Không có runtime overhead |
| **Tái sử dụng** | Định nghĩa một lần, sử dụng nhiều nơi |

---

## 2. Cài đặt và Import

### Thêm dependency

```toml
# Cargo.toml
[dependencies]
simbank-dsl = { path = "../crates/dsl" }
rust_decimal_macros = "1.33"
```

### Import trong code

```rust
use simbank_dsl::{banking_scenario, rule};
use simbank_dsl::{Scenario, Rule, RuleSet, TransactionContext};
use simbank_dsl::{CustomerOp, EmployeeOp, ShareholderOp, ManagerOp, AuditorOp};
use rust_decimal_macros::dec;
```

---

## 3. banking_scenario! Macro

### 3.1 Cú pháp tổng quát

```rust
let scenario = banking_scenario! {
    <StakeholderType> "<name>" {
        <operation>;
        <operation>;
    }

    <StakeholderType> "<name>" {
        <operation>;
    }
};
```

### 3.2 Customer Operations

**Các thao tác cho khách hàng:**

```rust
banking_scenario! {
    Customer "Nguyễn Văn A" {
        // Gửi tiền vào ví Funding
        deposit 10000 USD to Funding;

        // Gửi tiền vào ví Spot
        deposit 5000 USDT to Spot;

        // Chuyển tiền từ Funding sang Spot
        transfer 3000 USD from Funding to Spot;

        // Chuyển tiền từ Spot sang Margin
        transfer 2000 USDT from Spot to Margin;

        // Rút tiền từ Funding
        withdraw 1000 USD from Funding;

        // Rút tiền từ Spot
        withdraw 500 USDT from Spot;
    }
}
```

**Wallet Types:**

| Loại ví | Mô tả |
|---------|-------|
| `Funding` | Ví nạp/rút tiền fiat |
| `Spot` | Ví giao dịch spot |
| `Margin` | Ví ký quỹ margin trading |

**Currency Codes:**

| Mã | Loại tiền |
|----|-----------|
| `USD` | US Dollar |
| `EUR` | Euro |
| `VND` | Vietnam Dong |
| `USDT` | Tether (stablecoin) |
| `BTC` | Bitcoin |
| `ETH` | Ethereum |

### 3.3 Employee Operations

**Các thao tác cho nhân viên:**

```rust
banking_scenario! {
    Employee "Trần Thị B" {
        // Nhận lương
        receive_salary 8000 USD;

        // Mua bảo hiểm với plan name
        buy_insurance "Premium Health" for 500 USD;

        // Mua bảo hiểm khác
        buy_insurance "Dental Plan" for 100 USD;
    }
}
```

### 3.4 Shareholder Operations

**Các thao tác cho cổ đông:**

```rust
banking_scenario! {
    Shareholder "Công ty ABC Holdings" {
        // Nhận cổ tức
        receive_dividend 50000 USD;
    }

    Shareholder "Quỹ đầu tư XYZ" {
        receive_dividend 100000 USD;
    }
}
```

### 3.5 Manager Operations

**Các thao tác cho quản lý:**

```rust
banking_scenario! {
    Manager "Lê Văn C - CEO" {
        // Trả lương cho nhân viên
        pay_salary to "Trần Thị B" amount 8000 USD;

        // Thưởng với lý do
        pay_bonus to "Trần Thị B" amount 2000 USD reason "Q4 Performance";

        // Chi trả cổ tức
        pay_dividend to "Công ty ABC Holdings" amount 50000 USD;
    }
}
```

### 3.6 Auditor Operations

**Các thao tác cho kiểm toán viên:**

```rust
banking_scenario! {
    Auditor "Deloitte External" {
        // Quét giao dịch với khoảng thời gian và flags
        scan from "2025-01-01" to "2025-12-31" flags ["large_amount", "near_threshold"];

        // Xuất báo cáo
        report Markdown;
    }

    Auditor "Internal Compliance" {
        // Quét từ ngày bắt đầu (không có end date)
        scan from "2025-01-01" flags ["high_risk_country"];

        // Xuất JSON
        report Json;
    }
}
```

**Report Formats:**

| Format | Mô tả |
|--------|-------|
| `Markdown` | Báo cáo dạng Markdown |
| `Json` | Báo cáo dạng JSON |
| `Csv` | Báo cáo dạng CSV |

**AML Flags:**

| Flag | Mô tả |
|------|-------|
| `large_amount` | Giao dịch lớn (> $10,000) |
| `near_threshold` | Gần ngưỡng ($9,000-$10,000) |
| `high_risk_country` | Quốc gia rủi ro cao |
| `unusual_pattern` | Mẫu bất thường |
| `cross_border` | Xuyên biên giới |

---

## 4. rule! Macro

### 4.1 Cú pháp tổng quát

```rust
let rule = rule! {
    name: "<rule_name>"
    when <condition>
    then <action>
};
```

### 4.2 Amount Conditions

```rust
// Lớn hơn ngưỡng
rule! {
    name: "Large Transaction"
    when amount > 10000 USD
    then flag_aml "large_amount"
}

// Lớn hơn hoặc bằng ngưỡng
rule! {
    name: "Threshold Transaction"
    when amount >= 9000 USD
    then flag_aml "near_threshold"
}
```

### 4.3 Location Conditions

```rust
// Kiểm tra quốc gia trong danh sách
rule! {
    name: "High Risk Country"
    when location in ["IR", "KP", "SY", "CU"]
    then flag_aml "high_risk_country"
}

// Nhiều quốc gia
rule! {
    name: "Sanctioned Countries"
    when location in ["IR", "KP", "SY", "CU", "VE", "RU"]
    then block
}
```

### 4.4 Actions

| Action | Mô tả |
|--------|-------|
| `flag_aml "<flag>"` | Đánh dấu AML flag |
| `require_approval` | Yêu cầu phê duyệt |
| `block` | Chặn giao dịch |
| `notify "<message>"` | Gửi thông báo |

```rust
// Flag AML
rule! {
    name: "Large Amount Flag"
    when amount > 10000 USD
    then flag_aml "large_amount"
}

// Require approval
rule! {
    name: "Large Withdrawal"
    when amount > 50000 USD
    then require_approval
}

// Block transaction
rule! {
    name: "Prohibited Country"
    when location in ["KP"]
    then block
}
```

---

## 5. Sử dụng Scenario

### 5.1 Truy xuất operations

```rust
let scenario = banking_scenario! {
    Customer "Alice" {
        deposit 1000 USD to Funding;
    }
    Employee "Bob" {
        receive_salary 5000 USD;
    }
};

// Lấy tất cả customer operations
for (name, ops) in scenario.customers() {
    println!("Customer: {}", name);
    for op in ops {
        match op {
            CustomerOp::Deposit { amount, currency, to_wallet } => {
                println!("  Deposit {} {} to {:?}", amount, currency, to_wallet);
            }
            CustomerOp::Withdraw { amount, currency, from_wallet } => {
                println!("  Withdraw {} {} from {:?}", amount, currency, from_wallet);
            }
            CustomerOp::Transfer { amount, currency, from_wallet, to_wallet } => {
                println!("  Transfer {} {} {:?} -> {:?}", amount, currency, from_wallet, to_wallet);
            }
        }
    }
}

// Tương tự cho các loại khác
for (name, ops) in scenario.employees() { /* ... */ }
for (name, ops) in scenario.shareholders() { /* ... */ }
for (name, ops) in scenario.managers() { /* ... */ }
for (name, ops) in scenario.auditors() { /* ... */ }
```

### 5.2 Đếm stakeholders

```rust
let customer_count = scenario.customers().count();
let employee_count = scenario.employees().count();
let total_blocks = scenario.blocks.len();

println!("Total stakeholders: {}", total_blocks);
```

---

## 6. Sử dụng RuleSet

### 6.1 Tạo RuleSet

```rust
use simbank_dsl::{rule, RuleSet, TransactionContext};
use rust_decimal_macros::dec;

// Tạo các rules
let large_tx_rule = rule! {
    name: "Large Transaction"
    when amount > 10000 USD
    then flag_aml "large_amount"
};

let country_rule = rule! {
    name: "High Risk Country"
    when location in ["IR", "KP", "SY"]
    then flag_aml "high_risk_country"
};

// Tạo RuleSet
let ruleset = RuleSet::new()
    .add(large_tx_rule)
    .add(country_rule);
```

### 6.2 Đánh giá giao dịch

```rust
// Tạo transaction context
let ctx = TransactionContext::new()
    .with_amount(dec!(15000), "USD")
    .with_tx_type("deposit")
    .with_location("US");

// Đánh giá
let actions = ruleset.evaluate(&ctx);

if actions.is_empty() {
    println!("✅ Transaction approved");
} else {
    println!("⚠️ {} rules triggered", actions.len());
    for action in actions {
        println!("  - {:?}", action);
    }
}

// Kiểm tra cụ thể
if ruleset.is_blocked(&ctx) {
    println!("🚫 Transaction blocked");
}

if ruleset.requires_approval(&ctx) {
    println!("📝 Approval required");
}
```

---

## 7. Ví dụ hoàn chỉnh

### 7.1 Kịch bản Q4 Corporate Operations

```rust
use simbank_dsl::{banking_scenario, rule, RuleSet, TransactionContext};
use rust_decimal_macros::dec;

fn main() {
    // Định nghĩa kịch bản Q4
    let scenario = banking_scenario! {
        // Khách hàng VIP
        Customer "Nguyễn Văn A - VIP" {
            deposit 50000 USD to Funding;
            transfer 30000 USD from Funding to Spot;
            withdraw 5000 USD from Funding;
        }

        // Khách hàng thường
        Customer "Trần Thị B - Regular" {
            deposit 2000 USD to Funding;
            withdraw 500 USD from Funding;
        }

        // Nhân viên
        Employee "Lê Văn C - Engineer" {
            receive_salary 12000 USD;
            buy_insurance "Premium Health" for 400 USD;
        }

        // Cổ đông
        Shareholder "Quỹ ABC Investment" {
            receive_dividend 100000 USD;
        }

        // Quản lý
        Manager "CEO Phạm Văn D" {
            pay_salary to "Lê Văn C - Engineer" amount 12000 USD;
            pay_bonus to "Lê Văn C - Engineer" amount 3000 USD reason "Year-end";
        }

        // Kiểm toán
        Auditor "Deloitte Vietnam" {
            scan from "2025-10-01" to "2025-12-31" flags ["large_amount"];
            report Markdown;
        }
    };

    // In thống kê
    println!("📊 Kịch bản Q4 2025");
    println!("   Khách hàng:  {}", scenario.customers().count());
    println!("   Nhân viên:   {}", scenario.employees().count());
    println!("   Cổ đông:     {}", scenario.shareholders().count());
    println!("   Quản lý:     {}", scenario.managers().count());
    println!("   Kiểm toán:   {}", scenario.auditors().count());

    // Định nghĩa rules
    let ruleset = RuleSet::new()
        .add(rule! {
            name: "Large Transaction"
            when amount > 10000 USD
            then flag_aml "large_amount"
        })
        .add(rule! {
            name: "Very Large Withdrawal"
            when amount > 50000 USD
            then require_approval
        });

    // Kiểm tra giao dịch mẫu
    let test_transactions = vec![
        ("VIP Deposit", dec!(50000), "deposit", "VN"),
        ("Regular Deposit", dec!(2000), "deposit", "VN"),
        ("Dividend Payment", dec!(100000), "dividend", "VN"),
    ];

    for (desc, amount, tx_type, location) in test_transactions {
        let ctx = TransactionContext::new()
            .with_amount(amount, "USD")
            .with_tx_type(tx_type)
            .with_location(location);

        let actions = ruleset.evaluate(&ctx);
        let status = if actions.is_empty() { "✅" } else { "⚠️" };

        println!("{} {} - ${}: {} rules", status, desc, amount, actions.len());
    }
}
```

---

## 8. Best Practices

### 8.1 Đặt tên có ý nghĩa

```rust
// ✅ Tốt - tên rõ ràng, mô tả
Customer "Nguyễn Văn A - VIP Client" { }
rule! { name: "CTR Reporting Threshold" ... }

// ❌ Không tốt - tên mơ hồ
Customer "Client1" { }
rule! { name: "Rule1" ... }
```

### 8.2 Tổ chức rules theo category

```rust
// AML Rules
let aml_rules = RuleSet::new()
    .add(rule! { name: "Large Amount" when amount > 10000 USD then flag_aml "large_amount" })
    .add(rule! { name: "Near Threshold" when amount >= 9000 USD then flag_aml "near_threshold" });

// Country Rules
let country_rules = RuleSet::new()
    .add(rule! { name: "Sanctioned" when location in ["IR", "KP"] then block })
    .add(rule! { name: "High Risk" when location in ["RU", "BY"] then flag_aml "high_risk_country" });
```

### 8.3 Comment bằng tiếng Việt

```rust
banking_scenario! {
    // Khách hàng VIP - được ưu tiên xử lý
    Customer "VIP Client" {
        // Gửi tiền khởi tạo tài khoản
        deposit 100000 USD to Funding;
    }
}
```

---

## 9. Xử lý lỗi

### 9.1 Lỗi compile-time

DSL sử dụng macro Rust, lỗi được phát hiện tại compile-time:

```rust
// ❌ Lỗi: thiếu semicolon
Customer "Alice" {
    deposit 1000 USD to Funding  // <- Thiếu ;
}

// ❌ Lỗi: sai tên wallet
Customer "Alice" {
    deposit 1000 USD to Invalid;  // <- Không có wallet "Invalid"
}

// ❌ Lỗi: sai cú pháp amount
Customer "Alice" {
    deposit USD 1000 to Funding;  // <- Amount phải trước currency
}
```

### 9.2 Kiểm tra runtime

```rust
// Kiểm tra scenario có stakeholders
if scenario.blocks.is_empty() {
    eprintln!("Warning: Scenario có 0 stakeholders");
}

// Kiểm tra ruleset có rules
if ruleset.rules().is_empty() {
    eprintln!("Warning: RuleSet trống");
}
```

---

## 10. Tham khảo thêm

| Tài liệu | Mô tả |
|----------|-------|
| [examples/01_customer_onboarding.rs](../examples/01_customer_onboarding.rs) | Ví dụ quy trình khách hàng |
| [examples/02_employee_operations.rs](../examples/02_employee_operations.rs) | Ví dụ nghiệp vụ nhân viên |
| [examples/03_shareholder_dividends.rs](../examples/03_shareholder_dividends.rs) | Ví dụ chia cổ tức |
| [examples/04_auditor_aml_scan.rs](../examples/04_auditor_aml_scan.rs) | Ví dụ kiểm toán AML |
| [examples/05_complex_scenario.rs](../examples/05_complex_scenario.rs) | Ví dụ kịch bản phức tạp |
| [crates/dsl/src/lib.rs](../crates/dsl/src/lib.rs) | Source code macro definitions |
