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
