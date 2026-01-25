//! # Money Module
//!
//! Định nghĩa Currency và Money với rust_decimal để đảm bảo độ chính xác
//! cho cả fiat (VND, USD) và crypto (BTC, ETH với 8-18 decimals).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Đại diện cho một loại tiền tệ với số decimal động.
///
/// # Examples
/// ```
/// use simbank_core::Currency;
///
/// let usd = Currency::new("USD", "US Dollar", 2, "$");
/// let btc = Currency::new("BTC", "Bitcoin", 8, "₿");
/// let eth = Currency::new("ETH", "Ethereum", 18, "Ξ");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Currency {
    /// Mã tiền tệ (ISO 4217 cho fiat, symbol cho crypto)
    pub code: String,
    /// Tên đầy đủ
    pub name: String,
    /// Số chữ số thập phân (VND=0, USD=2, BTC=8, ETH=18)
    pub decimals: u8,
    /// Ký hiệu hiển thị
    pub symbol: String,
}

impl Currency {
    /// Tạo Currency mới
    pub fn new(code: &str, name: &str, decimals: u8, symbol: &str) -> Self {
        Self {
            code: code.to_uppercase(),
            name: name.to_string(),
            decimals,
            symbol: symbol.to_string(),
        }
    }

    // === Preset currencies ===

    /// Vietnamese Dong (0 decimals)
    pub fn vnd() -> Self {
        Self::new("VND", "Vietnamese Dong", 0, "₫")
    }

    /// US Dollar (2 decimals)
    pub fn usd() -> Self {
        Self::new("USD", "US Dollar", 2, "$")
    }

    /// Tether USDT (6 decimals)
    pub fn usdt() -> Self {
        Self::new("USDT", "Tether", 6, "₮")
    }

    /// USD Coin (6 decimals)
    pub fn usdc() -> Self {
        Self::new("USDC", "USD Coin", 6, "$")
    }

    /// Bitcoin (8 decimals)
    pub fn btc() -> Self {
        Self::new("BTC", "Bitcoin", 8, "₿")
    }

    /// Ethereum (18 decimals)
    pub fn eth() -> Self {
        Self::new("ETH", "Ethereum", 18, "Ξ")
    }

    /// Dogecoin (8 decimals)
    pub fn doge() -> Self {
        Self::new("DOGE", "Dogecoin", 8, "Ð")
    }

    /// Cardano (6 decimals)
    pub fn ada() -> Self {
        Self::new("ADA", "Cardano", 6, "₳")
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)
    }
}

/// Đại diện cho một số tiền với currency và amount.
///
/// Sử dụng `rust_decimal::Decimal` để đảm bảo độ chính xác tuyệt đối
/// cho các phép tính tài chính.
///
/// # Examples
/// ```
/// use simbank_core::{Money, Currency};
/// use rust_decimal_macros::dec;
///
/// let usd = Currency::usd();
/// let amount = Money::new(dec!(100.50), usd);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Số tiền (dạng Decimal, serialize thành String trong JSON)
    pub amount: Decimal,
    /// Loại tiền tệ
    pub currency: Currency,
}

impl Money {
    /// Tạo Money mới
    pub fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Tạo Money với amount = 0
    pub fn zero(currency: Currency) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }

    /// Tạo Money từ f64 (chỉ dùng cho test/demo, production nên dùng Decimal)
    pub fn from_f64(amount: f64, currency: Currency) -> Self {
        Self {
            amount: Decimal::try_from(amount).unwrap_or(Decimal::ZERO),
            currency,
        }
    }

    /// Kiểm tra có phải là số dương
    pub fn is_positive(&self) -> bool {
        self.amount > Decimal::ZERO
    }

    /// Kiểm tra có phải là 0
    pub fn is_zero(&self) -> bool {
        self.amount == Decimal::ZERO
    }

    /// Kiểm tra có phải là số âm
    pub fn is_negative(&self) -> bool {
        self.amount < Decimal::ZERO
    }

    /// Cộng hai Money cùng currency
    ///
    /// # Panics
    /// Panic nếu currency khác nhau
    pub fn add(&self, other: &Money) -> Money {
        assert_eq!(
            self.currency.code, other.currency.code,
            "Cannot add different currencies: {} vs {}",
            self.currency.code, other.currency.code
        );
        Money {
            amount: self.amount + other.amount,
            currency: self.currency.clone(),
        }
    }

    /// Trừ hai Money cùng currency
    ///
    /// # Panics
    /// Panic nếu currency khác nhau
    pub fn sub(&self, other: &Money) -> Money {
        assert_eq!(
            self.currency.code, other.currency.code,
            "Cannot subtract different currencies: {} vs {}",
            self.currency.code, other.currency.code
        );
        Money {
            amount: self.amount - other.amount,
            currency: self.currency.clone(),
        }
    }

    /// Nhân với một số
    pub fn mul(&self, multiplier: Decimal) -> Money {
        Money {
            amount: self.amount * multiplier,
            currency: self.currency.clone(),
        }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency.code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_currency_presets() {
        let usd = Currency::usd();
        assert_eq!(usd.code, "USD");
        assert_eq!(usd.decimals, 2);

        let btc = Currency::btc();
        assert_eq!(btc.code, "BTC");
        assert_eq!(btc.decimals, 8);

        let eth = Currency::eth();
        assert_eq!(eth.code, "ETH");
        assert_eq!(eth.decimals, 18);
    }

    #[test]
    fn test_money_add() {
        let usd = Currency::usd();
        let a = Money::new(dec!(100.50), usd.clone());
        let b = Money::new(dec!(50.25), usd);
        let result = a.add(&b);
        assert_eq!(result.amount, dec!(150.75));
    }

    #[test]
    fn test_money_sub() {
        let usd = Currency::usd();
        let a = Money::new(dec!(100.00), usd.clone());
        let b = Money::new(dec!(30.50), usd);
        let result = a.sub(&b);
        assert_eq!(result.amount, dec!(69.50));
    }

    #[test]
    #[should_panic(expected = "Cannot add different currencies")]
    fn test_money_add_different_currencies_panics() {
        let usd = Money::new(dec!(100), Currency::usd());
        let btc = Money::new(dec!(1), Currency::btc());
        usd.add(&btc);
    }

    #[test]
    fn test_money_display() {
        let money = Money::new(dec!(1234.56), Currency::usd());
        assert_eq!(format!("{}", money), "1234.56 USD");
    }

    #[test]
    fn test_high_precision_eth() {
        // ETH có 18 decimals - test precision
        let eth = Currency::eth();
        let wei = Money::new(dec!(0.000000000000000001), eth);
        assert!(wei.is_positive());
        assert_eq!(wei.amount, dec!(0.000000000000000001));
    }
}
