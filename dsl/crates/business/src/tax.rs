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
