---
date: 2026-01-25 17:55:34 
---

# Cấu trúc Dự án như sau:

```
./dsl
├── Cargo.toml
├── crates
│   ├── business
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── fee.rs
│   │       ├── interest.rs
│   │       ├── lib.rs
│   │       ├── process.rs
│   │       └── tax.rs
│   ├── core-banking
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── account.rs
│   │       ├── lib.rs
│   │       ├── traits.rs
│   │       ├── transaction.rs
│   │       └── types.rs
│   ├── dsl-macros
│   │   ├── Cargo.toml
│   │   └── src
│   │       └── lib.rs
│   └── reports
│       ├── Cargo.toml
│       └── src
│           ├── export.rs
│           ├── lib.rs
│           ├── summary.rs
│           └── yearly.rs
└── examples
    ├── advanced
    │   ├── Cargo.toml
    │   └── src
    │       └── main.rs
    └── basic
        ├── Cargo.toml
        └── src
            └── main.rs
```

# Danh sách chi tiết các file:

## File ./dsl\crates\business\src\fee.rs:
```rust
//! Quy tắc phí quản lý tài khoản

use core_banking::{VND, Account, AccountType, FeeCalculator};

/// Loại phí
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeType {
    /// Phí quản lý hàng năm
    AnnualMaintenance,
    /// Phí giao dịch
    Transaction,
    /// Phí rút tiền sớm
    EarlyWithdrawal,
    /// Phí chuyển khoản
    Transfer,
}

/// Quy tắc phí
#[derive(Debug, Clone)]
pub struct FeeRule {
    /// Loại phí
    pub fee_type: FeeType,
    /// Số tiền phí cố định
    pub fixed_amount: Option<VND>,
    /// Tỷ lệ phí (% số dư)
    pub percentage: Option<f64>,
    /// Phí tối thiểu
    pub min_fee: VND,
    /// Phí tối đa
    pub max_fee: Option<VND>,
    /// Mô tả
    pub description: String,
}

impl FeeRule {
    /// Tạo phí cố định
    pub fn fixed(fee_type: FeeType, amount: f64, description: impl Into<String>) -> Self {
        FeeRule {
            fee_type,
            fixed_amount: Some(VND::new(amount)),
            percentage: None,
            min_fee: VND::zero(),
            max_fee: None,
            description: description.into(),
        }
    }

    /// Tạo phí theo tỷ lệ
    pub fn percentage(fee_type: FeeType, rate: f64, min: f64, max: Option<f64>, description: impl Into<String>) -> Self {
        FeeRule {
            fee_type,
            fixed_amount: None,
            percentage: Some(rate),
            min_fee: VND::new(min),
            max_fee: max.map(VND::new),
            description: description.into(),
        }
    }

    /// Tính phí dựa trên số dư
    pub fn calculate(&self, balance: VND) -> VND {
        if let Some(fixed) = self.fixed_amount {
            return fixed;
        }

        if let Some(rate) = self.percentage {
            let mut fee = VND::new(balance.value() * rate);
            
            // Áp dụng min
            if fee.value() < self.min_fee.value() {
                fee = self.min_fee;
            }
            
            // Áp dụng max
            if let Some(max) = self.max_fee {
                if fee.value() > max.value() {
                    fee = max;
                }
            }
            
            return fee.round();
        }

        VND::zero()
    }
}

/// Bảng phí theo loại tài khoản
#[derive(Debug, Clone)]
pub struct FeeSchedule {
    /// Tên bảng phí
    pub name: String,
    /// Phí cho từng loại tài khoản
    rules: Vec<(AccountType, FeeRule)>,
    /// Phí mặc định
    default_fee: VND,
}

impl FeeSchedule {
    /// Tạo bảng phí mới
    pub fn new(name: impl Into<String>) -> Self {
        FeeSchedule {
            name: name.into(),
            rules: Vec::new(),
            default_fee: VND::new(1.0),
        }
    }

    /// Thêm quy tắc phí cho loại tài khoản
    pub fn for_account_type(mut self, account_type: AccountType, rule: FeeRule) -> Self {
        self.rules.push((account_type, rule));
        self
    }

    /// Đặt phí mặc định
    pub fn default_fee(mut self, fee: f64) -> Self {
        self.default_fee = VND::new(fee);
        self
    }

    /// Tìm quy tắc phí cho loại tài khoản
    pub fn find_rule(&self, account_type: AccountType) -> Option<&FeeRule> {
        self.rules
            .iter()
            .find(|(at, _)| *at == account_type)
            .map(|(_, rule)| rule)
    }

    /// Hiển thị bảng phí
    pub fn display(&self) {
        println!("📋 BẢNG PHÍ: {}", self.name);
        println!("───────────────────────────────────────");
        for (account_type, rule) in &self.rules {
            let fee_str = if let Some(fixed) = rule.fixed_amount {
                format!("{}", fixed)
            } else if let Some(rate) = rule.percentage {
                format!("{:.2}%", rate * 100.0)
            } else {
                "N/A".to_string()
            };
            println!("   {}: {} - {}", account_type, fee_str, rule.description);
        }
        println!("───────────────────────────────────────");
    }
}

impl FeeCalculator for FeeSchedule {
    fn calculate_fee(&self, account: &Account) -> VND {
        match self.find_rule(account.account_type) {
            Some(rule) => rule.calculate(account.balance()),
            None => self.default_fee,
        }
    }
}

/// Builder cho bảng phí chuẩn
pub fn standard_fee_schedule() -> FeeSchedule {
    FeeSchedule::new("Phí quản lý tài khoản")
        .for_account_type(
            AccountType::Savings,
            FeeRule::fixed(FeeType::AnnualMaintenance, 1.0, "Phí quản lý tiết kiệm")
        )
        .for_account_type(
            AccountType::Checking,
            FeeRule::fixed(FeeType::AnnualMaintenance, 2.0, "Phí quản lý thanh toán")
        )
        .for_account_type(
            AccountType::Premium,
            FeeRule::fixed(FeeType::AnnualMaintenance, 0.0, "Miễn phí VIP")
        )
        .default_fee(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_fee() {
        let rule = FeeRule::fixed(FeeType::AnnualMaintenance, 10.0, "Test");
        assert_eq!(rule.calculate(VND::new(1000.0)).value(), 10.0);
    }

    #[test]
    fn test_percentage_fee() {
        let rule = FeeRule::percentage(FeeType::Transaction, 0.01, 1.0, Some(100.0), "Test");
        
        // 1% of 500 = 5
        assert_eq!(rule.calculate(VND::new(500.0)).value(), 5.0);
        
        // 1% of 50 = 0.5 < min 1.0, so 1.0
        assert_eq!(rule.calculate(VND::new(50.0)).value(), 1.0);
        
        // 1% of 20000 = 200 > max 100, so 100
        assert_eq!(rule.calculate(VND::new(20000.0)).value(), 100.0);
    }
}

```

## File ./dsl\crates\business\src\interest.rs:
```rust
//! Quy tắc lãi suất theo cấp số dư (Tiered Interest)

use core_banking::{VND, Percentage, InterestCalculator};

/// Một cấp lãi suất
#[derive(Debug, Clone)]
pub struct InterestTier {
    /// Số dư tối thiểu
    pub min_balance: VND,
    /// Số dư tối đa (None = không giới hạn)
    pub max_balance: Option<VND>,
    /// Lãi suất áp dụng
    pub rate: Percentage,
    /// Mô tả cấp lãi suất
    pub description: String,
}

impl InterestTier {
    /// Tạo cấp lãi suất mới
    pub fn new(min: f64, max: Option<f64>, rate_percent: f64, description: impl Into<String>) -> Self {
        InterestTier {
            min_balance: VND::new(min),
            max_balance: max.map(VND::new),
            rate: Percentage::from_percent(rate_percent),
            description: description.into(),
        }
    }

    /// Kiểm tra số dư có thuộc cấp này không
    pub fn matches(&self, balance: VND) -> bool {
        let above_min = balance.value() >= self.min_balance.value();
        let below_max = match self.max_balance {
            Some(max) => balance.value() < max.value(),
            None => true,
        };
        above_min && below_max
    }
}

/// Bảng lãi suất theo cấp
#[derive(Debug, Clone)]
pub struct TieredInterestTable {
    /// Tên bảng lãi suất
    pub name: String,
    /// Các cấp lãi suất
    tiers: Vec<InterestTier>,
}

impl TieredInterestTable {
    /// Tạo bảng lãi suất mới
    pub fn new(name: impl Into<String>) -> Self {
        TieredInterestTable {
            name: name.into(),
            tiers: Vec::new(),
        }
    }

    /// Thêm cấp lãi suất
    pub fn add_tier(mut self, tier: InterestTier) -> Self {
        self.tiers.push(tier);
        self
    }

    /// Thêm cấp lãi suất với builder pattern
    pub fn tier(self, min: f64, max: Option<f64>, rate_percent: f64, description: impl Into<String>) -> Self {
        self.add_tier(InterestTier::new(min, max, rate_percent, description))
    }

    /// Tìm cấp lãi suất phù hợp
    pub fn find_tier(&self, balance: VND) -> Option<&InterestTier> {
        self.tiers.iter().find(|tier| tier.matches(balance))
    }

    /// Hiển thị bảng lãi suất
    pub fn display(&self) {
        println!("📋 BẢNG LÃI SUẤT: {}", self.name);
        println!("───────────────────────────────────────");
        for (i, tier) in self.tiers.iter().enumerate() {
            let max_str = match tier.max_balance {
                Some(max) => format!("{:.0}", max.value()),
                None => "∞".to_string(),
            };
            println!(
                "   {}. {:.0} - {} VND: {} ({})",
                i + 1,
                tier.min_balance.value(),
                max_str,
                tier.rate,
                tier.description
            );
        }
        println!("───────────────────────────────────────");
    }
}

impl InterestCalculator for TieredInterestTable {
    fn calculate_interest(&self, balance: VND) -> VND {
        match self.find_tier(balance) {
            Some(tier) => tier.rate.apply(balance).round(),
            None => VND::zero(),
        }
    }

    fn get_applicable_rate(&self, balance: VND) -> Percentage {
        match self.find_tier(balance) {
            Some(tier) => tier.rate,
            None => Percentage::from_decimal(0.0),
        }
    }
}

/// Builder cho bảng lãi suất chuẩn ngân hàng
pub fn standard_interest_table() -> TieredInterestTable {
    TieredInterestTable::new("Lãi suất tiết kiệm chuẩn")
        .tier(0.0, Some(1_000.0), 0.1, "Cấp cơ bản")
        .tier(1_000.0, Some(10_000.0), 0.2, "Cấp trung bình")
        .tier(10_000.0, None, 0.15, "Cấp cao")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_matching() {
        let tier = InterestTier::new(1000.0, Some(5000.0), 0.2, "Test");
        
        assert!(!tier.matches(VND::new(500.0)));
        assert!(tier.matches(VND::new(1000.0)));
        assert!(tier.matches(VND::new(3000.0)));
        assert!(!tier.matches(VND::new(5000.0)));
    }

    #[test]
    fn test_tiered_interest_calculation() {
        let table = standard_interest_table();
        
        // 500 VND -> 0.1% = 0.50
        assert!((table.calculate_interest(VND::new(500.0)).value() - 0.50).abs() < 0.01);
        
        // 5000 VND -> 0.2% = 10.00
        assert!((table.calculate_interest(VND::new(5000.0)).value() - 10.0).abs() < 0.01);
        
        // 25000 VND -> 0.15% = 37.50
        assert!((table.calculate_interest(VND::new(25000.0)).value() - 37.50).abs() < 0.01);
    }
}

```

## File ./dsl\crates\business\src\lib.rs:
```rust
//! # Business Logic
//! 
//! Module chứa các quy tắc nghiệp vụ ngân hàng:
//! - Lãi suất theo cấp số dư (tiered interest)
//! - Thuế thu nhập từ tiền lãi
//! - Phí quản lý tài khoản

pub mod interest;
pub mod tax;
pub mod fee;
pub mod process;

pub use interest::*;
pub use tax::*;
pub use fee::*;
pub use process::*;

```

## File ./dsl\crates\business\src\process.rs:
```rust
//! Quy trình nghiệp vụ tổng hợp

use core_banking::{Account, VND, InterestCalculator, TaxCalculator, FeeCalculator};
use crate::interest::TieredInterestTable;
use crate::tax::TaxTable;
use crate::fee::FeeSchedule;

/// Kết quả mô phỏng năm tài chính
#[derive(Debug, Clone)]
pub struct YearlySimulationResult {
    pub year: u32,
    pub opening_balance: VND,
    pub fee_charged: VND,
    pub interest_earned: VND,
    pub tax_paid: VND,
    pub net_interest: VND,
    pub closing_balance: VND,
}

impl YearlySimulationResult {
    /// Hiển thị kết quả
    pub fn display(&self) {
        println!("📅 Năm {}:", self.year);
        println!("   Số dư đầu kỳ:    {}", self.opening_balance);
        println!("   Phí quản lý:     -{}", self.fee_charged);
        println!("   Tiền lãi:        +{}", self.interest_earned);
        println!("   Thuế:            -{}", self.tax_paid);
        println!("   Lãi ròng:        +{}", self.net_interest);
        println!("   Số dư cuối kỳ:   {}", self.closing_balance);
    }
}

/// Quy trình mô phỏng năm tài chính
#[derive(Debug)]
pub struct YearlyProcess {
    pub interest_table: TieredInterestTable,
    pub tax_table: TaxTable,
    pub fee_schedule: FeeSchedule,
}

impl YearlyProcess {
    /// Tạo quy trình mới
    pub fn new(
        interest_table: TieredInterestTable,
        tax_table: TaxTable,
        fee_schedule: FeeSchedule,
    ) -> Self {
        YearlyProcess {
            interest_table,
            tax_table,
            fee_schedule,
        }
    }

    /// Thực thi mô phỏng 1 năm
    pub fn execute(&self, account: &mut Account, year: u32) -> YearlySimulationResult {
        let opening_balance = account.balance();

        // 1. Trừ phí quản lý
        let fee = self.fee_schedule.calculate_fee(account);
        let _ = account.apply_fee(fee, format!("Phí quản lý năm {}", year));

        // 2. Tính lãi (sau khi trừ phí)
        let balance_after_fee = account.balance();
        let interest = self.interest_table.calculate_interest(balance_after_fee);
        let rate = self.interest_table.get_applicable_rate(balance_after_fee);

        // 3. Tính thuế trên tiền lãi
        let tax = self.tax_table.calculate_tax(interest);
        let tax_rate = self.tax_table.get_applicable_rate(interest);

        // 4. Lãi ròng = lãi - thuế
        let net_interest = interest - tax;

        // 5. Cập nhật tài khoản
        account.apply_interest(
            interest,
            format!("Lãi suất {} trên số dư {}", rate, balance_after_fee),
        );
        account.apply_tax(tax, format!("Thuế {} trên lãi {}", tax_rate, interest));

        let closing_balance = account.balance();

        YearlySimulationResult {
            year,
            opening_balance,
            fee_charged: fee,
            interest_earned: interest,
            tax_paid: tax,
            net_interest,
            closing_balance,
        }
    }

    /// Mô phỏng nhiều năm
    pub fn simulate_years(&self, account: &mut Account, years: u32) -> Vec<YearlySimulationResult> {
        println!("\n🔄 BẮT ĐẦU MÔ PHỎNG {} NĂM", years);
        println!("═══════════════════════════════════════");
        
        self.interest_table.display();
        self.tax_table.display();
        self.fee_schedule.display();
        
        println!("\n📊 KẾT QUẢ TỪNG NĂM:");
        println!("───────────────────────────────────────");

        let mut results = Vec::new();
        for year in 1..=years {
            let result = self.execute(account, year);
            result.display();
            println!();
            results.push(result);
        }

        println!("═══════════════════════════════════════");
        println!("💰 SỐ DƯ CUỐI CÙNG: {}", account.balance());
        
        results
    }
}

/// Builder để tạo quy trình với cấu hình tùy chỉnh
pub struct ProcessBuilder {
    interest_table: Option<TieredInterestTable>,
    tax_table: Option<TaxTable>,
    fee_schedule: Option<FeeSchedule>,
}

impl ProcessBuilder {
    pub fn new() -> Self {
        ProcessBuilder {
            interest_table: None,
            tax_table: None,
            fee_schedule: None,
        }
    }

    pub fn interest(mut self, table: TieredInterestTable) -> Self {
        self.interest_table = Some(table);
        self
    }

    pub fn tax(mut self, table: TaxTable) -> Self {
        self.tax_table = Some(table);
        self
    }

    pub fn fee(mut self, schedule: FeeSchedule) -> Self {
        self.fee_schedule = Some(schedule);
        self
    }

    pub fn build(self) -> YearlyProcess {
        YearlyProcess::new(
            self.interest_table.unwrap_or_else(crate::standard_interest_table),
            self.tax_table.unwrap_or_else(crate::standard_tax_table),
            self.fee_schedule.unwrap_or_else(crate::standard_fee_schedule),
        )
    }
}

impl Default for ProcessBuilder {
    fn default() -> Self {
        Self::new()
    }
}

```

## File ./dsl\crates\business\src\tax.rs:
```rust
//! Quy tắc thuế thu nhập từ tiền lãi

use core_banking::{VND, Percentage, TaxCalculator};

/// Loại mức thuế
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaxBracket {
    /// Thuế thấp (5%)
    Low,
    /// Thuế trung bình (10%)
    Medium,
    /// Thuế cao (15%)
    High,
    /// Miễn thuế
    Exempt,
}

impl TaxBracket {
    /// Lấy tỷ lệ thuế
    pub fn rate(&self) -> Percentage {
        match self {
            TaxBracket::Low => Percentage::from_percent(5.0),
            TaxBracket::Medium => Percentage::from_percent(10.0),
            TaxBracket::High => Percentage::from_percent(15.0),
            TaxBracket::Exempt => Percentage::from_percent(0.0),
        }
    }
}

/// Một quy tắc thuế
#[derive(Debug, Clone)]
pub struct TaxRule {
    /// Ngưỡng tiền lãi tối đa áp dụng quy tắc này
    pub threshold: VND,
    /// Mức thuế
    pub bracket: TaxBracket,
    /// Mô tả
    pub description: String,
}

impl TaxRule {
    /// Tạo quy tắc thuế mới
    pub fn new(threshold: f64, bracket: TaxBracket, description: impl Into<String>) -> Self {
        TaxRule {
            threshold: VND::new(threshold),
            bracket,
            description: description.into(),
        }
    }

    /// Kiểm tra tiền lãi có thuộc quy tắc này không
    pub fn matches(&self, interest: VND) -> bool {
        interest.value() < self.threshold.value()
    }
}

/// Bảng thuế thu nhập
#[derive(Debug, Clone)]
pub struct TaxTable {
    /// Tên bảng thuế
    pub name: String,
    /// Các quy tắc thuế (sắp xếp theo ngưỡng tăng dần)
    rules: Vec<TaxRule>,
    /// Mức thuế mặc định nếu vượt tất cả ngưỡng
    default_bracket: TaxBracket,
}

impl TaxTable {
    /// Tạo bảng thuế mới
    pub fn new(name: impl Into<String>) -> Self {
        TaxTable {
            name: name.into(),
            rules: Vec::new(),
            default_bracket: TaxBracket::Medium,
        }
    }

    /// Thêm quy tắc thuế
    pub fn add_rule(mut self, rule: TaxRule) -> Self {
        self.rules.push(rule);
        // Sắp xếp theo ngưỡng tăng dần
        self.rules.sort_by(|a, b| {
            a.threshold.value().partial_cmp(&b.threshold.value()).unwrap()
        });
        self
    }

    /// Thêm quy tắc với builder pattern
    pub fn rule(self, threshold: f64, bracket: TaxBracket, description: impl Into<String>) -> Self {
        self.add_rule(TaxRule::new(threshold, bracket, description))
    }

    /// Đặt mức thuế mặc định
    pub fn default(mut self, bracket: TaxBracket) -> Self {
        self.default_bracket = bracket;
        self
    }

    /// Tìm mức thuế phù hợp
    pub fn find_bracket(&self, interest: VND) -> TaxBracket {
        self.rules
            .iter()
            .find(|rule| rule.matches(interest))
            .map(|rule| rule.bracket)
            .unwrap_or(self.default_bracket)
    }

    /// Hiển thị bảng thuế
    pub fn display(&self) {
        println!("📋 BẢNG THUẾ: {}", self.name);
        println!("───────────────────────────────────────");
        for rule in &self.rules {
            println!(
                "   Lãi < {:.0} VND: {:?} ({}) - {}",
                rule.threshold.value(),
                rule.bracket,
                rule.bracket.rate(),
                rule.description
            );
        }
        println!(
            "   Mặc định: {:?} ({})",
            self.default_bracket,
            self.default_bracket.rate()
        );
        println!("───────────────────────────────────────");
    }
}

impl TaxCalculator for TaxTable {
    fn calculate_tax(&self, interest: VND) -> VND {
        let bracket = self.find_bracket(interest);
        bracket.rate().apply(interest).round()
    }

    fn get_applicable_rate(&self, interest: VND) -> Percentage {
        self.find_bracket(interest).rate()
    }
}

/// Builder cho bảng thuế chuẩn
pub fn standard_tax_table() -> TaxTable {
    TaxTable::new("Thuế thu nhập từ lãi tiết kiệm")
        .rule(100.0, TaxBracket::Exempt, "Miễn thuế lãi nhỏ")
        .rule(500.0, TaxBracket::Low, "Thuế suất ưu đãi")
        .default(TaxBracket::Medium)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tax_calculation() {
        let table = standard_tax_table();
        
        // Lãi 50 VND -> Miễn thuế
        assert_eq!(table.calculate_tax(VND::new(50.0)).value(), 0.0);
        
        // Lãi 200 VND -> 5% = 10 VND
        assert_eq!(table.calculate_tax(VND::new(200.0)).value(), 10.0);
        
        // Lãi 1000 VND -> 10% = 100 VND
        assert_eq!(table.calculate_tax(VND::new(1000.0)).value(), 100.0);
    }
}

```

## File ./dsl\crates\core-banking\src\account.rs:
```rust
//! Định nghĩa tài khoản ngân hàng cơ bản

use crate::types::{VND, AccountType, AccountStatus};
use crate::transaction::Transaction;

/// Tài khoản ngân hàng cơ bản
#[derive(Debug, Clone)]
pub struct Account {
    /// ID tài khoản
    pub id: String,
    /// Số dư hiện tại
    balance: VND,
    /// Loại tài khoản
    pub account_type: AccountType,
    /// Trạng thái
    pub status: AccountStatus,
    /// Lịch sử giao dịch
    transactions: Vec<Transaction>,
}

impl Account {
    /// Tạo tài khoản mới
    pub fn new(id: impl Into<String>, initial_balance: VND, account_type: AccountType) -> Self {
        let id = id.into();
        let mut account = Account {
            id: id.clone(),
            balance: initial_balance,
            account_type,
            status: AccountStatus::Active,
            transactions: Vec::new(),
        };
        
        account.record_transaction(Transaction::deposit(
            format!("Mở tài khoản {}", id),
            initial_balance,
        ));
        
        account
    }

    /// Tạo tài khoản tiết kiệm
    pub fn savings(id: impl Into<String>, initial_balance: f64) -> Self {
        Self::new(id, VND::new(initial_balance), AccountType::Savings)
    }

    /// Tạo tài khoản thanh toán
    pub fn checking(id: impl Into<String>, initial_balance: f64) -> Self {
        Self::new(id, VND::new(initial_balance), AccountType::Checking)
    }

    /// Lấy số dư
    pub fn balance(&self) -> VND {
        self.balance
    }

    /// Gửi tiền
    pub fn deposit(&mut self, amount: VND, description: impl Into<String>) -> Result<VND, AccountError> {
        if self.status != AccountStatus::Active {
            return Err(AccountError::InactiveAccount);
        }
        
        if !amount.is_positive() {
            return Err(AccountError::InvalidAmount);
        }

        self.balance = self.balance + amount;
        self.record_transaction(Transaction::deposit(description, amount));
        
        Ok(self.balance)
    }

    /// Rút tiền
    pub fn withdraw(&mut self, amount: VND, description: impl Into<String>) -> Result<VND, AccountError> {
        if self.status != AccountStatus::Active {
            return Err(AccountError::InactiveAccount);
        }

        if !amount.is_positive() {
            return Err(AccountError::InvalidAmount);
        }

        if self.balance.value() < amount.value() {
            return Err(AccountError::InsufficientFunds {
                requested: amount,
                available: self.balance,
            });
        }

        self.balance = self.balance - amount;
        self.record_transaction(Transaction::withdrawal(description, amount));
        
        Ok(self.balance)
    }

    /// Áp dụng phí
    pub fn apply_fee(&mut self, fee: VND, description: impl Into<String>) -> Result<VND, AccountError> {
        self.balance = self.balance - fee;
        self.record_transaction(Transaction::fee(description, fee));
        Ok(self.balance)
    }

    /// Áp dụng lãi
    pub fn apply_interest(&mut self, interest: VND, description: impl Into<String>) -> VND {
        self.balance = self.balance + interest;
        self.record_transaction(Transaction::interest(description, interest));
        self.balance
    }

    /// Áp dụng thuế
    pub fn apply_tax(&mut self, tax: VND, description: impl Into<String>) -> VND {
        self.balance = self.balance - tax;
        self.record_transaction(Transaction::tax(description, tax));
        self.balance
    }

    /// Ghi nhận giao dịch
    fn record_transaction(&mut self, transaction: Transaction) {
        self.transactions.push(transaction);
    }

    /// Lấy lịch sử giao dịch
    pub fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    /// Hiển thị thông tin tài khoản
    pub fn display(&self) {
        println!("═══════════════════════════════════════");
        println!("📊 THÔNG TIN TÀI KHOẢN");
        println!("═══════════════════════════════════════");
        println!("   ID: {}", self.id);
        println!("   Loại: {}", self.account_type);
        println!("   Số dư: {}", self.balance);
        println!("   Trạng thái: {:?}", self.status);
        println!("═══════════════════════════════════════");
    }

    /// Hiển thị lịch sử giao dịch
    pub fn display_transactions(&self) {
        println!("\n📜 LỊCH SỬ GIAO DỊCH ({} giao dịch):", self.transactions.len());
        println!("───────────────────────────────────────");
        for tx in &self.transactions {
            println!("  {}", tx);
        }
        println!("───────────────────────────────────────");
    }
}

/// Lỗi liên quan đến tài khoản
#[derive(Debug, Clone)]
pub enum AccountError {
    InsufficientFunds { requested: VND, available: VND },
    InvalidAmount,
    InactiveAccount,
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountError::InsufficientFunds { requested, available } => {
                write!(f, "Số dư không đủ. Yêu cầu: {}, Hiện có: {}", requested, available)
            }
            AccountError::InvalidAmount => write!(f, "Số tiền không hợp lệ"),
            AccountError::InactiveAccount => write!(f, "Tài khoản không hoạt động"),
        }
    }
}

impl std::error::Error for AccountError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_account() {
        let account = Account::savings("TK001", 100.0);
        assert_eq!(account.balance().value(), 100.0);
    }

    #[test]
    fn test_deposit() {
        let mut account = Account::savings("TK001", 100.0);
        account.deposit(VND::new(50.0), "Gửi thêm").unwrap();
        assert_eq!(account.balance().value(), 150.0);
    }

    #[test]
    fn test_withdraw_success() {
        let mut account = Account::savings("TK001", 100.0);
        account.withdraw(VND::new(30.0), "Rút tiền").unwrap();
        assert_eq!(account.balance().value(), 70.0);
    }

    #[test]
    fn test_withdraw_insufficient() {
        let mut account = Account::savings("TK001", 100.0);
        let result = account.withdraw(VND::new(150.0), "Rút tiền");
        assert!(matches!(result, Err(AccountError::InsufficientFunds { .. })));
    }
}

```

## File ./dsl\crates\core-banking\src\lib.rs:
```rust
//! # Core Banking
//! 
//! Module cốt lõi chứa các kiểu dữ liệu, traits và abstractions
//! cho hệ thống ngân hàng.

pub mod types;
pub mod account;
pub mod transaction;
pub mod traits;

pub use types::*;
pub use account::*;
pub use transaction::*;
pub use traits::*;

```

## File ./dsl\crates\core-banking\src\traits.rs:
```rust
//! Traits định nghĩa các hành vi nghiệp vụ

use crate::types::{VND, Percentage};
use crate::account::Account;

/// Trait cho tính lãi suất
pub trait InterestCalculator {
    /// Tính tiền lãi dựa trên số dư
    fn calculate_interest(&self, balance: VND) -> VND;
    
    /// Lấy tỷ lệ lãi suất áp dụng
    fn get_applicable_rate(&self, balance: VND) -> Percentage;
}

/// Trait cho tính thuế
pub trait TaxCalculator {
    /// Tính thuế dựa trên tiền lãi
    fn calculate_tax(&self, interest: VND) -> VND;
    
    /// Lấy tỷ lệ thuế áp dụng
    fn get_applicable_rate(&self, interest: VND) -> Percentage;
}

/// Trait cho tính phí
pub trait FeeCalculator {
    /// Tính phí dựa trên tài khoản
    fn calculate_fee(&self, account: &Account) -> VND;
}

/// Trait cho quy trình nghiệp vụ
pub trait BusinessProcess {
    /// Thực thi quy trình
    fn execute(&self, account: &mut Account) -> Result<ProcessResult, ProcessError>;
    
    /// Tên quy trình
    fn name(&self) -> &str;
}

/// Kết quả xử lý quy trình
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub description: String,
    pub before_balance: VND,
    pub after_balance: VND,
    pub details: Vec<String>,
}

/// Lỗi xử lý quy trình
#[derive(Debug, Clone)]
pub struct ProcessError {
    pub message: String,
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ProcessError {}

```

## File ./dsl\crates\core-banking\src\transaction.rs:
```rust
//! Định nghĩa giao dịch ngân hàng

use crate::types::VND;
use std::fmt;

/// Loại giao dịch
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Fee,
    Interest,
    Tax,
    Transfer,
}

impl TransactionType {
    pub fn icon(&self) -> &'static str {
        match self {
            TransactionType::Deposit => "📥",
            TransactionType::Withdrawal => "📤",
            TransactionType::Fee => "💳",
            TransactionType::Interest => "💰",
            TransactionType::Tax => "🏛️",
            TransactionType::Transfer => "🔄",
        }
    }
}

/// Giao dịch ngân hàng
#[derive(Debug, Clone)]
pub struct Transaction {
    /// Loại giao dịch
    pub tx_type: TransactionType,
    /// Mô tả
    pub description: String,
    /// Số tiền
    pub amount: VND,
    /// Thời gian (đơn giản hóa)
    pub timestamp: u64,
}

impl Transaction {
    /// Tạo giao dịch mới
    pub fn new(tx_type: TransactionType, description: impl Into<String>, amount: VND) -> Self {
        Transaction {
            tx_type,
            description: description.into(),
            amount,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Giao dịch gửi tiền
    pub fn deposit(description: impl Into<String>, amount: VND) -> Self {
        Self::new(TransactionType::Deposit, description, amount)
    }

    /// Giao dịch rút tiền
    pub fn withdrawal(description: impl Into<String>, amount: VND) -> Self {
        Self::new(TransactionType::Withdrawal, description, amount)
    }

    /// Giao dịch phí
    pub fn fee(description: impl Into<String>, amount: VND) -> Self {
        Self::new(TransactionType::Fee, description, amount)
    }

    /// Giao dịch lãi
    pub fn interest(description: impl Into<String>, amount: VND) -> Self {
        Self::new(TransactionType::Interest, description, amount)
    }

    /// Giao dịch thuế
    pub fn tax(description: impl Into<String>, amount: VND) -> Self {
        Self::new(TransactionType::Tax, description, amount)
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.tx_type.icon(),
            self.description,
            self.amount
        )
    }
}

```

## File ./dsl\crates\core-banking\src\types.rs:
```rust
//! Kiểu dữ liệu cơ bản cho hệ thống ngân hàng

use std::fmt;
use std::ops::{Add, Sub, Mul};

/// Kiểu tiền tệ VND với độ chính xác cao
/// 
/// Sử dụng newtype pattern để đảm bảo an toàn kiểu
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct VND(f64);

impl VND {
    /// Tạo giá trị VND mới
    pub fn new(amount: f64) -> Self {
        VND(amount)
    }

    /// Lấy giá trị số
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Kiểm tra giá trị dương
    pub fn is_positive(&self) -> bool {
        self.0 > 0.0
    }

    /// Giá trị không
    pub fn zero() -> Self {
        VND(0.0)
    }

    /// Giá trị tối đa
    pub fn max() -> Self {
        VND(f64::MAX)
    }

    /// Làm tròn đến 2 chữ số thập phân
    pub fn round(&self) -> Self {
        VND((self.0 * 100.0).round() / 100.0)
    }
}

impl fmt::Display for VND {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2} VND", self.0)
    }
}

impl Add for VND {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        VND(self.0 + rhs.0)
    }
}

impl Sub for VND {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        VND(self.0 - rhs.0)
    }
}

impl Mul<f64> for VND {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self::Output {
        VND(self.0 * rhs)
    }
}

impl From<f64> for VND {
    fn from(value: f64) -> Self {
        VND(value)
    }
}

/// Tỷ lệ phần trăm (0.0 - 1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Percentage(f64);

impl Percentage {
    /// Tạo từ giá trị thập phân (0.05 = 5%)
    pub fn from_decimal(value: f64) -> Self {
        Percentage(value)
    }

    /// Tạo từ giá trị phần trăm (5.0 = 5%)
    pub fn from_percent(value: f64) -> Self {
        Percentage(value / 100.0)
    }

    /// Lấy giá trị thập phân
    pub fn as_decimal(&self) -> f64 {
        self.0
    }

    /// Lấy giá trị phần trăm
    pub fn as_percent(&self) -> f64 {
        self.0 * 100.0
    }

    /// Áp dụng tỷ lệ lên số tiền
    pub fn apply(&self, amount: VND) -> VND {
        amount * self.0
    }
}

impl fmt::Display for Percentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}%", self.0 * 100.0)
    }
}

/// Loại tài khoản
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    /// Tài khoản tiết kiệm
    Savings,
    /// Tài khoản thanh toán
    Checking,
    /// Tài khoản tiền gửi có kỳ hạn
    TermDeposit,
    /// Tài khoản VIP
    Premium,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountType::Savings => write!(f, "Tiết kiệm"),
            AccountType::Checking => write!(f, "Thanh toán"),
            AccountType::TermDeposit => write!(f, "Có kỳ hạn"),
            AccountType::Premium => write!(f, "VIP"),
        }
    }
}

/// Trạng thái tài khoản
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    Active,
    Frozen,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vnd_operations() {
        let a = VND::new(100.0);
        let b = VND::new(50.0);
        
        assert_eq!((a + b).value(), 150.0);
        assert_eq!((a - b).value(), 50.0);
        assert_eq!((a * 0.1).value(), 10.0);
    }

    #[test]
    fn test_percentage() {
        let rate = Percentage::from_percent(5.0);
        let amount = VND::new(1000.0);
        
        assert_eq!(rate.apply(amount).value(), 50.0);
    }
}

```

## File ./dsl\crates\dsl-macros\src\lib.rs:
```rust
//! # DSL Macros
//! 
//! Module chứa các macro DSL cho nghiệp vụ ngân hàng.
//! Cung cấp cú pháp thân thiện gần với ngôn ngữ tự nhiên.

// Re-export dependencies để người dùng không cần import riêng
pub use core_banking;
pub use business;

pub use core_banking::{Account, VND, Percentage, AccountType};
pub use business::{
    TieredInterestTable, InterestTier,
    TaxTable, TaxRule, TaxBracket,
    FeeSchedule, FeeRule, FeeType,
    YearlyProcess, ProcessBuilder,
};

/// Macro tạo tài khoản tiết kiệm
/// 
/// # Cú pháp
/// - `tài_khoản!(tiết_kiệm "ID", số_dư)` - Tạo tài khoản tiết kiệm
/// - `tài_khoản!(thanh_toán "ID", số_dư)` - Tạo tài khoản thanh toán
#[macro_export]
macro_rules! tài_khoản {
    (tiết_kiệm $id:expr, $balance:expr) => {
        $crate::Account::savings($id, $balance)
    };
    (thanh_toán $id:expr, $balance:expr) => {
        $crate::Account::checking($id, $balance)
    };
}

/// Macro định nghĩa bảng lãi suất bậc thang
/// 
/// # Cú pháp
/// ```ignore
/// lãi_suất! {
///     tên: "Bảng lãi suất",
///     cấp: [
///         (0, 1000): 0.1% => "Cấp cơ bản",
///         (1000, 10000): 0.2% => "Cấp trung",
///         (10000, MAX): 0.15% => "Cấp cao",
///     ]
/// }
/// ```
#[macro_export]
macro_rules! lãi_suất {
    {
        tên: $name:expr,
        cấp: [
            $(
                ($min:expr, $max:tt): $rate:tt% => $desc:expr
            ),* $(,)?
        ]
    } => {{
        let mut table = $crate::TieredInterestTable::new($name);
        $(
            table = table.tier(
                $min as f64,
                $crate::__parse_max!($max),
                $rate,
                $desc
            );
        )*
        table
    }};
}

/// Helper macro để parse max value
#[macro_export]
#[doc(hidden)]
macro_rules! __parse_max {
    (MAX) => { None };
    ($val:expr) => { Some($val as f64) };
}

/// Macro định nghĩa bảng thuế
/// 
/// # Cú pháp
/// ```ignore
/// thuế! {
///     tên: "Bảng thuế",
///     quy_tắc: [
///         lãi_dưới 100 => Miễn,
///         lãi_dưới 500 => Thấp,
///     ],
///     mặc_định: Trung_bình
/// }
/// ```
#[macro_export]
macro_rules! thuế {
    {
        tên: $name:expr,
        quy_tắc: [
            $(lãi_dưới $threshold:expr => $bracket:ident),* $(,)?
        ],
        mặc_định: $default:ident
    } => {{
        let mut table = $crate::TaxTable::new($name);
        $(
            table = table.rule(
                $threshold as f64,
                $crate::__tax_bracket!($bracket),
                format!("Lãi < {} VND", $threshold)
            );
        )*
        table.default($crate::__tax_bracket!($default))
    }};
}

/// Helper macro để chuyển đổi tên thuế tiếng Việt sang enum
#[macro_export]
#[doc(hidden)]
macro_rules! __tax_bracket {
    (Miễn) => { $crate::TaxBracket::Exempt };
    (Thấp) => { $crate::TaxBracket::Low };
    (Trung_bình) => { $crate::TaxBracket::Medium };
    (Cao) => { $crate::TaxBracket::High };
}

/// Macro định nghĩa bảng phí
/// 
/// # Cú pháp
/// ```ignore
/// phí! {
///     tên: "Bảng phí",
///     tiết_kiệm: 1.0,
///     thanh_toán: 2.0,
///     vip: 0.0
/// }
/// ```
#[macro_export]
macro_rules! phí {
    {
        tên: $name:expr
        $(, tiết_kiệm: $savings:expr)?
        $(, thanh_toán: $checking:expr)?
        $(, vip: $premium:expr)?
    } => {{
        let mut schedule = $crate::FeeSchedule::new($name);
        $(
            schedule = schedule.for_account_type(
                $crate::AccountType::Savings,
                $crate::FeeRule::fixed(
                    $crate::FeeType::AnnualMaintenance,
                    $savings,
                    "Phí quản lý tiết kiệm"
                )
            );
        )?
        $(
            schedule = schedule.for_account_type(
                $crate::AccountType::Checking,
                $crate::FeeRule::fixed(
                    $crate::FeeType::AnnualMaintenance,
                    $checking,
                    "Phí quản lý thanh toán"
                )
            );
        )?
        $(
            schedule = schedule.for_account_type(
                $crate::AccountType::Premium,
                $crate::FeeRule::fixed(
                    $crate::FeeType::AnnualMaintenance,
                    $premium,
                    "Phí VIP"
                )
            );
        )?
        schedule
    }};
}

/// Macro mô phỏng năm tài chính
/// 
/// # Cú pháp
/// ```ignore
/// mô_phỏng! {
///     tài_khoản: tk,
///     số_năm: 3,
///     lãi_suất: interest_table,
///     thuế: tax_table,
///     phí: fee_schedule
/// }
/// ```
#[macro_export]
macro_rules! mô_phỏng {
    {
        tài_khoản: $account:ident,
        số_năm: $years:expr,
        lãi_suất: $interest:expr,
        thuế: $tax:expr,
        phí: $fee:expr
    } => {{
        let process = $crate::YearlyProcess::new($interest, $tax, $fee);
        process.simulate_years(&mut $account, $years)
    }};
    
    // Phiên bản đơn giản với cấu hình mặc định
    {
        tài_khoản: $account:ident,
        số_năm: $years:expr
    } => {{
        let process = $crate::ProcessBuilder::new().build();
        process.simulate_years(&mut $account, $years)
    }};
}

/// Macro tạo quy trình nghiệp vụ hoàn chỉnh
/// 
/// # Cú pháp
/// ```ignore
/// nghiệp_vụ! {
///     // Định nghĩa tài khoản
///     let tk = tiết_kiệm("TK001", 5000.0);
///     
///     // Định nghĩa quy tắc
///     lãi_suất: {
///         (0 -> 1000): 0.1%,
///         (1000 -> 10000): 0.2%,
///         (từ 10000): 0.15%
///     },
///     thuế: {
///         lãi_dưới 100 => Miễn,
///         lãi_dưới 500 => Thấp,
///         mặc_định => Trung_bình
///     },
///     phí: 1.0,
///     
///     // Thực thi
///     mô_phỏng: 3
/// }
/// ```
#[macro_export]
macro_rules! nghiệp_vụ {
    {
        tài_khoản: $account_type:ident($id:expr, $balance:expr),
        lãi_suất: {
            $(($min:expr, $max:tt): $rate:tt%),* $(,)?
        },
        thuế: {
            $(lãi_dưới $threshold:expr => $bracket:ident),* $(,)?
            mặc_định => $default:ident
        },
        phí: $fee:expr,
        mô_phỏng: $years:expr
    } => {{
        println!("╔═══════════════════════════════════════════════════════════╗");
        println!("║        🏦 MÔ PHỎNG NGHIỆP VỤ NGÂN HÀNG 🏦                 ║");
        println!("╚═══════════════════════════════════════════════════════════╝\n");
        
        // Tạo tài khoản
        let mut account = $crate::tài_khoản!($account_type $id, $balance);
        
        // Tạo bảng lãi suất
        let interest_table = $crate::lãi_suất! {
            tên: "Lãi suất bậc thang",
            cấp: [
                $(($min, $max): $rate% => concat!("Cấp ", stringify!($min))),*
            ]
        };
        
        // Tạo bảng thuế
        let tax_table = $crate::thuế! {
            tên: "Thuế thu nhập từ lãi",
            quy_tắc: [
                $(lãi_dưới $threshold => $bracket),*
            ],
            mặc_định: $default
        };
        
        // Tạo bảng phí
        let fee_schedule = $crate::phí! {
            tên: "Phí quản lý",
            tiết_kiệm: $fee
        };
        
        // Thực thi mô phỏng
        let results = $crate::mô_phỏng! {
            tài_khoản: account,
            số_năm: $years,
            lãi_suất: interest_table,
            thuế: tax_table,
            phí: fee_schedule
        };
        
        println!("\n╔═══════════════════════════════════════════════════════════╗");
        println!("║                   🎉 HOÀN TẤT 🎉                           ║");
        println!("╚═══════════════════════════════════════════════════════════╝");
        
        (account, results)
    }};
}

```

## File ./dsl\crates\reports\src\export.rs:
```rust
//! Xuất báo cáo ra các định dạng khác nhau

use business::YearlySimulationResult;

/// Trait xuất báo cáo
pub trait ReportExporter {
    fn export(&self, results: &[YearlySimulationResult]) -> String;
}

/// Xuất CSV
pub struct CsvExporter;

impl ReportExporter for CsvExporter {
    fn export(&self, results: &[YearlySimulationResult]) -> String {
        let mut csv = String::new();
        csv.push_str("Năm,Số dư đầu kỳ,Phí,Lãi,Thuế,Lãi ròng,Số dư cuối kỳ\n");
        
        for r in results {
            csv.push_str(&format!(
                "{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
                r.year,
                r.opening_balance.value(),
                r.fee_charged.value(),
                r.interest_earned.value(),
                r.tax_paid.value(),
                r.net_interest.value(),
                r.closing_balance.value()
            ));
        }
        
        csv
    }
}

/// Xuất JSON
pub struct JsonExporter;

impl ReportExporter for JsonExporter {
    fn export(&self, results: &[YearlySimulationResult]) -> String {
        let mut json = String::from("[\n");
        
        for (i, r) in results.iter().enumerate() {
            json.push_str(&format!(
                r#"  {{
    "year": {},
    "opening_balance": {:.2},
    "fee_charged": {:.2},
    "interest_earned": {:.2},
    "tax_paid": {:.2},
    "net_interest": {:.2},
    "closing_balance": {:.2}
  }}"#,
                r.year,
                r.opening_balance.value(),
                r.fee_charged.value(),
                r.interest_earned.value(),
                r.tax_paid.value(),
                r.net_interest.value(),
                r.closing_balance.value()
            ));
            
            if i < results.len() - 1 {
                json.push_str(",\n");
            } else {
                json.push('\n');
            }
        }
        
        json.push(']');
        json
    }
}

/// Xuất Markdown
pub struct MarkdownExporter;

impl ReportExporter for MarkdownExporter {
    fn export(&self, results: &[YearlySimulationResult]) -> String {
        let mut md = String::new();
        md.push_str("# Báo cáo Mô phỏng Tài chính\n\n");
        md.push_str("| Năm | Số dư đầu kỳ | Phí | Lãi | Thuế | Lãi ròng | Số dư cuối kỳ |\n");
        md.push_str("|-----|--------------|-----|-----|------|----------|---------------|\n");
        
        for r in results {
            md.push_str(&format!(
                "| {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
                r.year,
                r.opening_balance.value(),
                r.fee_charged.value(),
                r.interest_earned.value(),
                r.tax_paid.value(),
                r.net_interest.value(),
                r.closing_balance.value()
            ));
        }
        
        md
    }
}

```

## File ./dsl\crates\reports\src\lib.rs:
```rust
//! # Reports Module
//! 
//! Module báo cáo và xuất dữ liệu nghiệp vụ ngân hàng

pub mod summary;
pub mod yearly;
pub mod export;

pub use summary::*;
pub use yearly::*;
pub use export::*;

```

## File ./dsl\crates\reports\src\summary.rs:
```rust
//! Báo cáo tổng hợp tài khoản

use core_banking::{Account, VND};

/// Báo cáo tổng hợp
#[derive(Debug, Clone)]
pub struct AccountSummary {
    pub account_id: String,
    pub account_type: String,
    pub total_deposits: VND,
    pub total_withdrawals: VND,
    pub total_fees: VND,
    pub total_interest: VND,
    pub total_tax: VND,
    pub current_balance: VND,
}

impl AccountSummary {
    /// Tạo báo cáo từ tài khoản
    pub fn from_account(account: &Account) -> Self {
        let mut total_deposits = VND::zero();
        let mut total_withdrawals = VND::zero();
        let mut total_fees = VND::zero();
        let mut total_interest = VND::zero();
        let mut total_tax = VND::zero();

        for tx in account.transactions() {
            match tx.tx_type {
                core_banking::TransactionType::Deposit => {
                    total_deposits = total_deposits + tx.amount;
                }
                core_banking::TransactionType::Withdrawal => {
                    total_withdrawals = total_withdrawals + tx.amount;
                }
                core_banking::TransactionType::Fee => {
                    total_fees = total_fees + tx.amount;
                }
                core_banking::TransactionType::Interest => {
                    total_interest = total_interest + tx.amount;
                }
                core_banking::TransactionType::Tax => {
                    total_tax = total_tax + tx.amount;
                }
                _ => {}
            }
        }

        AccountSummary {
            account_id: account.id.clone(),
            account_type: format!("{}", account.account_type),
            total_deposits,
            total_withdrawals,
            total_fees,
            total_interest,
            total_tax,
            current_balance: account.balance(),
        }
    }

    /// Hiển thị báo cáo
    pub fn display(&self) {
        println!("╔═══════════════════════════════════════════════════════════╗");
        println!("║              📊 BÁO CÁO TỔNG HỢP TÀI KHOẢN                ║");
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║  Mã tài khoản:    {:>38}  ║", self.account_id);
        println!("║  Loại tài khoản:  {:>38}  ║", self.account_type);
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║  💰 TỔNG GỬI VÀO:             {:>26}  ║", format!("{}", self.total_deposits));
        println!("║  📤 TỔNG RÚT RA:              {:>26}  ║", format!("{}", self.total_withdrawals));
        println!("║  💳 TỔNG PHÍ:                 {:>26}  ║", format!("{}", self.total_fees));
        println!("║  💰 TỔNG LÃI:                 {:>26}  ║", format!("{}", self.total_interest));
        println!("║  🏛️  TỔNG THUẾ:               {:>26}  ║", format!("{}", self.total_tax));
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║  💵 SỐ DƯ HIỆN TẠI:           {:>26}  ║", format!("{}", self.current_balance));
        println!("╚═══════════════════════════════════════════════════════════╝");
    }
}

```

## File ./dsl\crates\reports\src\yearly.rs:
```rust
//! Báo cáo theo năm

use business::YearlySimulationResult;
use core_banking::VND;

/// Báo cáo nhiều năm
#[derive(Debug, Clone)]
pub struct YearlyReport {
    pub years: Vec<YearlySimulationResult>,
}

impl YearlyReport {
    /// Tạo từ kết quả mô phỏng
    pub fn from_results(results: Vec<YearlySimulationResult>) -> Self {
        YearlyReport { years: results }
    }

    /// Tổng phí qua các năm
    pub fn total_fees(&self) -> VND {
        self.years.iter().fold(VND::zero(), |acc, r| acc + r.fee_charged)
    }

    /// Tổng lãi qua các năm
    pub fn total_interest(&self) -> VND {
        self.years.iter().fold(VND::zero(), |acc, r| acc + r.interest_earned)
    }

    /// Tổng thuế qua các năm
    pub fn total_tax(&self) -> VND {
        self.years.iter().fold(VND::zero(), |acc, r| acc + r.tax_paid)
    }

    /// Tổng lãi ròng qua các năm
    pub fn total_net_interest(&self) -> VND {
        self.years.iter().fold(VND::zero(), |acc, r| acc + r.net_interest)
    }

    /// Hiển thị báo cáo
    pub fn display(&self) {
        if self.years.is_empty() {
            println!("Không có dữ liệu");
            return;
        }

        let first = &self.years[0];
        let last = &self.years[self.years.len() - 1];

        println!("╔═══════════════════════════════════════════════════════════╗");
        println!("║              📈 BÁO CÁO TỔNG HỢP {} NĂM                    ║", self.years.len());
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║  Số dư ban đầu:   {:>38}  ║", format!("{}", first.opening_balance));
        println!("║  Số dư cuối cùng: {:>38}  ║", format!("{}", last.closing_balance));
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║  📊 THỐNG KÊ TỔNG HỢP                                      ║");
        println!("║  ─────────────────────────────────────────────────────────║");
        println!("║  Tổng phí:        {:>38}  ║", format!("{}", self.total_fees()));
        println!("║  Tổng lãi:        {:>38}  ║", format!("{}", self.total_interest()));
        println!("║  Tổng thuế:       {:>38}  ║", format!("{}", self.total_tax()));
        println!("║  Lãi ròng:        {:>38}  ║", format!("{}", self.total_net_interest()));
        println!("╠═══════════════════════════════════════════════════════════╣");
        
        let growth = last.closing_balance.value() - first.opening_balance.value();
        let growth_pct = (growth / first.opening_balance.value()) * 100.0;
        println!("║  📈 TĂNG TRƯỞNG: {:+.2} VND ({:+.2}%)                        ║", growth, growth_pct);
        println!("╚═══════════════════════════════════════════════════════════╝");
    }
}

```

## File ./dsl\examples\advanced\src\main.rs:
```rust
//! # Ví dụ nâng cao - Mô hình nghiệp vụ phức tạp
//! 
//! Triển khai DSL theo yêu cầu từ DSL_COMPLICATE.md:
//! - Lãi suất theo cấp số dư
//! - Thuế thu nhập từ tiền lãi
//! - Báo cáo tổng hợp

use dsl_macros::*;
use reports::{AccountSummary, YearlyReport, CsvExporter, JsonExporter, MarkdownExporter, ReportExporter};

fn main() {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║       🏦 MÔ HÌNH NGHIỆP VỤ NÂNG CAO - BANKING DSL 🏦      ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 1: Tài khoản 5,000 VND với lãi suất bậc thang
    // ═══════════════════════════════════════════════════════════════════
    example_1_tiered_interest();

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 2: Tài khoản 25,000 VND - VIP
    // ═══════════════════════════════════════════════════════════════════
    example_2_vip_account();

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 3: Sử dụng DSL macro tổng hợp
    // ═══════════════════════════════════════════════════════════════════
    example_3_full_dsl();

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 4: Xuất báo cáo
    // ═══════════════════════════════════════════════════════════════════
    example_4_reports();

    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║              🎉 HOÀN TẤT MÔ PHỎNG NÂNG CAO 🎉             ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
}

fn example_1_tiered_interest() {
    println!("\n🎯 VÍ DỤ 1: Tài khoản 5,000 VND - Lãi suất bậc thang");
    println!("═══════════════════════════════════════════════════════════════\n");
    
    println!("📋 QUY TẮC NGHIỆP VỤ:");
    println!("   Lãi suất theo cấp số dư:");
    println!("     - Dưới 1,000 VND: 0.1%/năm");
    println!("     - 1,000 - 10,000 VND: 0.2%/năm");
    println!("     - Trên 10,000 VND: 0.15%/năm");
    println!("   Thuế thu nhập từ lãi:");
    println!("     - Lãi < 100: Miễn thuế");
    println!("     - Lãi < 500: 5%");
    println!("     - Lãi >= 500: 10%");
    println!();

    // Tạo tài khoản
    let mut tk = tài_khoản!(tiết_kiệm "TK-5000", 5000.0);

    // Định nghĩa bảng lãi suất bậc thang bằng DSL
    let interest_table = lãi_suất! {
        tên: "Lãi suất tiết kiệm bậc thang",
        cấp: [
            (0, 1000): 0.1% => "Cấp cơ bản",
            (1000, 10000): 0.2% => "Cấp trung bình",
            (10000, MAX): 0.15% => "Cấp cao cấp",
        ]
    };

    // Định nghĩa bảng thuế bằng DSL
    let tax_table = thuế! {
        tên: "Thuế thu nhập cá nhân từ lãi",
        quy_tắc: [
            lãi_dưới 100 => Miễn,
            lãi_dưới 500 => Thấp,
        ],
        mặc_định: Trung_bình
    };

    // Định nghĩa bảng phí
    let fee_schedule = phí! {
        tên: "Phí quản lý tiêu chuẩn",
        tiết_kiệm: 1.0
    };

    // Mô phỏng 3 năm
    let results = mô_phỏng! {
        tài_khoản: tk,
        số_năm: 3,
        lãi_suất: interest_table,
        thuế: tax_table,
        phí: fee_schedule
    };

    // Hiển thị báo cáo
    let summary = AccountSummary::from_account(&tk);
    summary.display();

    let yearly_report = YearlyReport::from_results(results);
    yearly_report.display();
}

fn example_2_vip_account() {
    println!("\n\n🎯 VÍ DỤ 2: Tài khoản VIP 25,000 VND");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut tk_vip = tài_khoản!(tiết_kiệm "TK-VIP-25000", 25000.0);

    // Bảng lãi suất VIP (cao hơn)
    let vip_interest = lãi_suất! {
        tên: "Lãi suất VIP",
        cấp: [
            (0, 5000): 0.15% => "VIP cơ bản",
            (5000, 20000): 0.25% => "VIP trung",
            (20000, MAX): 0.30% => "VIP cao cấp",
        ]
    };

    // Thuế giống nhau
    let tax_table = thuế! {
        tên: "Thuế TNCN",
        quy_tắc: [
            lãi_dưới 100 => Miễn,
            lãi_dưới 500 => Thấp,
        ],
        mặc_định: Trung_bình
    };

    // VIP miễn phí
    let vip_fee = phí! {
        tên: "Phí VIP",
        tiết_kiệm: 0.0
    };

    let results = mô_phỏng! {
        tài_khoản: tk_vip,
        số_năm: 5,
        lãi_suất: vip_interest,
        thuế: tax_table,
        phí: vip_fee
    };

    let yearly_report = YearlyReport::from_results(results);
    yearly_report.display();
}

fn example_3_full_dsl() {
    println!("\n\n🎯 VÍ DỤ 3: DSL Macro tổng hợp");
    println!("═══════════════════════════════════════════════════════════════\n");
    
    println!("Sử dụng macro nghiệp_vụ! để định nghĩa toàn bộ logic trong một block:\n");

    // Sử dụng macro nghiệp_vụ! - cú pháp gần với ngôn ngữ tự nhiên nhất
    let (account, results) = nghiệp_vụ! {
        tài_khoản: tiết_kiệm("TK-FULL-DSL", 10000.0),
        lãi_suất: {
            (0, 1000): 0.1%,
            (1000, 10000): 0.2%,
            (10000, MAX): 0.15%
        },
        thuế: {
            lãi_dưới 100 => Miễn,
            lãi_dưới 500 => Thấp,
            mặc_định => Trung_bình
        },
        phí: 1.0,
        mô_phỏng: 3
    };

    let summary = AccountSummary::from_account(&account);
    summary.display();

    let yearly_report = YearlyReport::from_results(results);
    yearly_report.display();
}

fn example_4_reports() {
    println!("\n\n🎯 VÍ DỤ 4: Xuất báo cáo đa định dạng");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut tk = tài_khoản!(tiết_kiệm "TK-REPORT", 8000.0);

    let process = ProcessBuilder::new().build();
    let results = process.simulate_years(&mut tk, 3);

    // Xuất CSV
    println!("📄 XUẤT CSV:");
    println!("─────────────────────────────────────────────────────────────");
    let csv = CsvExporter.export(&results);
    println!("{}", csv);

    // Xuất JSON
    println!("📄 XUẤT JSON:");
    println!("─────────────────────────────────────────────────────────────");
    let json = JsonExporter.export(&results);
    println!("{}", json);

    // Xuất Markdown
    println!("\n📄 XUẤT MARKDOWN:");
    println!("─────────────────────────────────────────────────────────────");
    let md = MarkdownExporter.export(&results);
    println!("{}", md);
}

```

## File ./dsl\examples\basic\src\main.rs:
```rust
//! # Ví dụ cơ bản - Banking DSL
//! 
//! Minh họa cách sử dụng DSL cho nghiệp vụ ngân hàng đơn giản.

use dsl_macros::*;

fn main() {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║           🏦 VÍ DỤ CƠ BẢN - BANKING DSL 🏦                ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 1: Tạo tài khoản đơn giản
    // ═══════════════════════════════════════════════════════════════════
    println!("📋 VÍ DỤ 1: Tạo tài khoản");
    println!("─────────────────────────────────────────────────────────────");

    // Sử dụng DSL macro
    let mut tk = tài_khoản!(tiết_kiệm "TK001", 100.0);
    tk.display();

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 2: Giao dịch cơ bản
    // ═══════════════════════════════════════════════════════════════════
    println!("\n📋 VÍ DỤ 2: Giao dịch cơ bản");
    println!("─────────────────────────────────────────────────────────────");

    // Gửi thêm tiền
    let _ = tk.deposit(VND::new(50.0), "Gửi thêm tiền");
    
    // Rút tiền
    let _ = tk.withdraw(VND::new(30.0), "Rút tiền");
    
    // Áp dụng phí
    let _ = tk.apply_fee(VND::new(1.0), "Phí quản lý");
    
    // Áp dụng lãi
    let interest = tk.balance() * 0.002; // 0.2%
    tk.apply_interest(interest, "Lãi suất 0.2%");

    tk.display();
    tk.display_transactions();

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 3: Sử dụng bảng lãi suất chuẩn
    // ═══════════════════════════════════════════════════════════════════
    println!("\n📋 VÍ DỤ 3: Bảng lãi suất bậc thang");
    println!("─────────────────────────────────────────────────────────────");

    use core_banking::InterestCalculator;

    // Sử dụng bảng lãi suất chuẩn từ business module
    let interest_table = business::standard_interest_table();
    interest_table.display();

    // Tính lãi cho các mức số dư khác nhau
    let balances = [500.0, 5000.0, 25000.0];
    for balance in balances {
        let b = VND::new(balance);
        let rate = interest_table.get_applicable_rate(b);
        let interest = interest_table.calculate_interest(b);
        println!("   Số dư {}: lãi suất {} → tiền lãi {}", b, rate, interest);
    }

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 4: Mô phỏng với cấu hình mặc định
    // ═══════════════════════════════════════════════════════════════════
    println!("\n📋 VÍ DỤ 4: Mô phỏng với cấu hình mặc định");
    println!("─────────────────────────────────────────────────────────────");

    let mut tk_sim = tài_khoản!(tiết_kiệm "TK002", 5000.0);
    
    // Mô phỏng 3 năm với cấu hình mặc định
    let _results = mô_phỏng! {
        tài_khoản: tk_sim,
        số_năm: 3
    };

    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                    🎉 HOÀN TẤT DEMO 🎉                     ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
}

```

# Thông tin bổ sung:

## Cargo.toml dependencies:
- resolver = "2"
- members = [
- version = "0.1.0"
- edition = "2021"
- authors = ["Banking Team"]

