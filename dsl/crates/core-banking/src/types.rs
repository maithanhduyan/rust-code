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
