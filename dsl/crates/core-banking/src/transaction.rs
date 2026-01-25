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
