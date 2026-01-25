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
