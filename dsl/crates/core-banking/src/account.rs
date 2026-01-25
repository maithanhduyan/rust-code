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
