//! # Wallet Module
//!
//! Định nghĩa WalletType, Wallet, và Balance cho mô hình Exchange-style.
//! Mỗi Account có nhiều Wallets (Spot, Funding, Margin, Futures, Earn),
//! mỗi Wallet chứa nhiều loại tiền tệ.

use crate::money::{Currency, Money};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Loại ví trong hệ thống Exchange-style.
///
/// Phase 1: Chỉ implement Spot + Funding
/// Phase 2: Margin, Futures, Earn
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalletType {
    /// Ví giao dịch Spot
    Spot,
    /// Ví nạp/rút tiền
    Funding,
    /// Ví giao dịch ký quỹ (Phase 2)
    Margin,
    /// Ví hợp đồng tương lai (Phase 2)
    Futures,
    /// Ví staking/savings (Phase 2)
    Earn,
}

impl WalletType {
    /// Trả về code string cho DB
    pub fn as_str(&self) -> &'static str {
        match self {
            WalletType::Spot => "spot",
            WalletType::Funding => "funding",
            WalletType::Margin => "margin",
            WalletType::Futures => "futures",
            WalletType::Earn => "earn",
        }
    }

    /// Parse từ string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "spot" => Some(WalletType::Spot),
            "funding" => Some(WalletType::Funding),
            "margin" => Some(WalletType::Margin),
            "futures" => Some(WalletType::Futures),
            "earn" => Some(WalletType::Earn),
            _ => None,
        }
    }

    /// Các wallet types cho Phase 1
    pub fn phase1_types() -> Vec<WalletType> {
        vec![WalletType::Spot, WalletType::Funding]
    }

    /// Tất cả wallet types
    pub fn all_types() -> Vec<WalletType> {
        vec![
            WalletType::Spot,
            WalletType::Funding,
            WalletType::Margin,
            WalletType::Futures,
            WalletType::Earn,
        ]
    }
}

impl fmt::Display for WalletType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Số dư của một loại tiền trong wallet.
///
/// - `available`: Số dư khả dụng, có thể sử dụng ngay
/// - `locked`: Số dư bị khóa (đang trong order, staking, margin)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    /// Currency của balance này
    pub currency: Currency,
    /// Số dư khả dụng
    pub available: Decimal,
    /// Số dư bị khóa (Phase 2)
    pub locked: Decimal,
    /// Thời gian cập nhật cuối
    pub updated_at: DateTime<Utc>,
}

impl Balance {
    /// Tạo Balance mới với available = 0, locked = 0
    pub fn new(currency: Currency) -> Self {
        Self {
            currency,
            available: Decimal::ZERO,
            locked: Decimal::ZERO,
            updated_at: Utc::now(),
        }
    }

    /// Tạo Balance với số dư khởi tạo
    pub fn with_amount(currency: Currency, available: Decimal) -> Self {
        Self {
            currency,
            available,
            locked: Decimal::ZERO,
            updated_at: Utc::now(),
        }
    }

    /// Tổng số dư (available + locked)
    pub fn total(&self) -> Decimal {
        self.available + self.locked
    }

    /// Kiểm tra có đủ số dư available để thực hiện giao dịch
    pub fn can_spend(&self, amount: Decimal) -> bool {
        self.available >= amount
    }

    /// Cộng thêm vào available
    pub fn credit(&mut self, amount: Decimal) {
        self.available += amount;
        self.updated_at = Utc::now();
    }

    /// Trừ từ available
    ///
    /// # Returns
    /// - `Ok(())` nếu thành công
    /// - `Err(amount_needed)` nếu không đủ số dư
    pub fn debit(&mut self, amount: Decimal) -> Result<(), Decimal> {
        if self.available >= amount {
            self.available -= amount;
            self.updated_at = Utc::now();
            Ok(())
        } else {
            Err(amount - self.available)
        }
    }

    /// Chuyển sang Money object
    pub fn as_money(&self) -> Money {
        Money::new(self.available, self.currency.clone())
    }
}

impl fmt::Display for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.locked > Decimal::ZERO {
            write!(
                f,
                "{} {} (locked: {})",
                self.available, self.currency.code, self.locked
            )
        } else {
            write!(f, "{} {}", self.available, self.currency.code)
        }
    }
}

/// Ví của người dùng.
///
/// Mỗi ví thuộc một loại (Spot, Funding, ...) và chứa nhiều loại tiền tệ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    /// ID của wallet (WAL_001, WAL_002, ...)
    pub id: String,
    /// ID của account sở hữu
    pub account_id: String,
    /// Loại ví
    pub wallet_type: WalletType,
    /// Map từ currency code -> Balance
    pub balances: HashMap<String, Balance>,
    /// Trạng thái ví
    pub status: WalletStatus,
    /// Thời gian tạo
    pub created_at: DateTime<Utc>,
}

/// Trạng thái của ví
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalletStatus {
    Active,
    Frozen,
    Closed,
}

impl WalletStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WalletStatus::Active => "active",
            WalletStatus::Frozen => "frozen",
            WalletStatus::Closed => "closed",
        }
    }
}

impl Wallet {
    /// Tạo Wallet mới
    pub fn new(id: String, account_id: String, wallet_type: WalletType) -> Self {
        Self {
            id,
            account_id,
            wallet_type,
            balances: HashMap::new(),
            status: WalletStatus::Active,
            created_at: Utc::now(),
        }
    }

    /// Lấy balance của một currency
    pub fn get_balance(&self, currency_code: &str) -> Option<&Balance> {
        self.balances.get(currency_code)
    }

    /// Lấy hoặc tạo balance cho currency
    pub fn get_or_create_balance(&mut self, currency: Currency) -> &mut Balance {
        let code = currency.code.clone();
        self.balances
            .entry(code)
            .or_insert_with(|| Balance::new(currency))
    }

    /// Credit (cộng tiền) vào wallet
    pub fn credit(&mut self, currency: Currency, amount: Decimal) {
        let balance = self.get_or_create_balance(currency);
        balance.credit(amount);
    }

    /// Debit (trừ tiền) từ wallet
    ///
    /// # Returns
    /// - `Ok(())` nếu thành công
    /// - `Err(amount_needed)` nếu không đủ số dư
    pub fn debit(&mut self, currency_code: &str, amount: Decimal) -> Result<(), Decimal> {
        if let Some(balance) = self.balances.get_mut(currency_code) {
            balance.debit(amount)
        } else {
            Err(amount) // Không có balance = thiếu toàn bộ amount
        }
    }

    /// Kiểm tra ví có active không
    pub fn is_active(&self) -> bool {
        self.status == WalletStatus::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_wallet_type_str() {
        assert_eq!(WalletType::Spot.as_str(), "spot");
        assert_eq!(WalletType::Funding.as_str(), "funding");
        assert_eq!(WalletType::from_str("SPOT"), Some(WalletType::Spot));
        assert_eq!(WalletType::from_str("unknown"), None);
    }

    #[test]
    fn test_balance_operations() {
        let mut balance = Balance::new(Currency::usd());
        assert_eq!(balance.available, dec!(0));

        balance.credit(dec!(100));
        assert_eq!(balance.available, dec!(100));

        assert!(balance.can_spend(dec!(50)));
        assert!(!balance.can_spend(dec!(150)));

        assert!(balance.debit(dec!(30)).is_ok());
        assert_eq!(balance.available, dec!(70));

        assert!(balance.debit(dec!(100)).is_err());
    }

    #[test]
    fn test_wallet_multi_currency() {
        let mut wallet = Wallet::new(
            "WAL_001".to_string(),
            "ACC_001".to_string(),
            WalletType::Spot,
        );

        wallet.credit(Currency::usd(), dec!(100));
        wallet.credit(Currency::btc(), dec!(0.5));
        wallet.credit(Currency::usd(), dec!(50)); // Thêm vào USD có sẵn

        assert_eq!(wallet.get_balance("USD").unwrap().available, dec!(150));
        assert_eq!(wallet.get_balance("BTC").unwrap().available, dec!(0.5));
        assert!(wallet.get_balance("ETH").is_none());
    }

    #[test]
    fn test_wallet_debit() {
        let mut wallet = Wallet::new(
            "WAL_001".to_string(),
            "ACC_001".to_string(),
            WalletType::Funding,
        );

        wallet.credit(Currency::usdt(), dec!(1000));

        assert!(wallet.debit("USDT", dec!(300)).is_ok());
        assert_eq!(wallet.get_balance("USDT").unwrap().available, dec!(700));

        // Không đủ tiền
        let result = wallet.debit("USDT", dec!(1000));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), dec!(300)); // Thiếu 300

        // Currency không tồn tại
        let result = wallet.debit("BTC", dec!(1));
        assert!(result.is_err());
    }
}
