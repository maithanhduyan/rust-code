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
