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
