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
