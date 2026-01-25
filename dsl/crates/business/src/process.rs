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
