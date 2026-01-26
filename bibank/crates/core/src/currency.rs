//! Currency - Type-safe currency/asset codes
//!
//! Instead of raw strings, we use an enum for common currencies
//! and a fallback for custom tokens.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur when parsing currencies
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CurrencyError {
    #[error("Empty currency code")]
    EmptyCode,

    #[error("Currency code too long (max 10 chars): {0}")]
    TooLong(String),

    #[error("Invalid currency code format: {0}")]
    InvalidFormat(String),
}

/// Currency/Asset codes
///
/// Common currencies are pre-defined for type safety and performance.
/// Custom tokens use the `Other` variant.
///
/// # Examples
/// ```
/// use bibank_core::Currency;
///
/// let usdt: Currency = "USDT".parse().unwrap();
/// assert_eq!(usdt, Currency::Usdt);
///
/// let btc = Currency::Btc;
/// assert_eq!(btc.to_string(), "BTC");
///
/// // Custom token
/// let custom: Currency = "MYTOKEN".parse().unwrap();
/// assert!(matches!(custom, Currency::Other(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Currency {
    // === Stablecoins ===
    /// Tether USD
    Usdt,
    /// USD Coin
    Usdc,
    /// Binance USD
    Busd,
    /// Dai
    Dai,

    // === Major Crypto ===
    /// Bitcoin
    Btc,
    /// Ethereum
    Eth,
    /// Binance Coin
    Bnb,
    /// Solana
    Sol,
    /// XRP
    Xrp,
    /// Cardano
    Ada,
    /// Dogecoin
    Doge,
    /// Polygon
    Matic,
    /// Litecoin
    Ltc,

    // === Fiat ===
    /// US Dollar
    Usd,
    /// Euro
    Eur,
    /// British Pound
    Gbp,
    /// Japanese Yen
    Jpy,
    /// Vietnamese Dong
    Vnd,

    // === Custom tokens ===
    /// Any other token/currency
    Other(String),
}

impl Currency {
    /// Returns the currency code as a string slice
    pub fn code(&self) -> &str {
        match self {
            // Stablecoins
            Currency::Usdt => "USDT",
            Currency::Usdc => "USDC",
            Currency::Busd => "BUSD",
            Currency::Dai => "DAI",

            // Crypto
            Currency::Btc => "BTC",
            Currency::Eth => "ETH",
            Currency::Bnb => "BNB",
            Currency::Sol => "SOL",
            Currency::Xrp => "XRP",
            Currency::Ada => "ADA",
            Currency::Doge => "DOGE",
            Currency::Matic => "MATIC",
            Currency::Ltc => "LTC",

            // Fiat
            Currency::Usd => "USD",
            Currency::Eur => "EUR",
            Currency::Gbp => "GBP",
            Currency::Jpy => "JPY",
            Currency::Vnd => "VND",

            // Other
            Currency::Other(s) => s.as_str(),
        }
    }

    /// Returns true if this is a stablecoin
    pub fn is_stablecoin(&self) -> bool {
        matches!(
            self,
            Currency::Usdt | Currency::Usdc | Currency::Busd | Currency::Dai
        )
    }

    /// Returns true if this is fiat currency
    pub fn is_fiat(&self) -> bool {
        matches!(
            self,
            Currency::Usd | Currency::Eur | Currency::Gbp | Currency::Jpy | Currency::Vnd
        )
    }

    /// Returns true if this is a major cryptocurrency
    pub fn is_crypto(&self) -> bool {
        matches!(
            self,
            Currency::Btc
                | Currency::Eth
                | Currency::Bnb
                | Currency::Sol
                | Currency::Xrp
                | Currency::Ada
                | Currency::Doge
                | Currency::Matic
                | Currency::Ltc
        )
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl FromStr for Currency {
    type Err = CurrencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_uppercase();

        if s.is_empty() {
            return Err(CurrencyError::EmptyCode);
        }

        if s.len() > 10 {
            return Err(CurrencyError::TooLong(s));
        }

        // Validate: only alphanumeric
        if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(CurrencyError::InvalidFormat(s));
        }

        Ok(match s.as_str() {
            // Stablecoins
            "USDT" => Currency::Usdt,
            "USDC" => Currency::Usdc,
            "BUSD" => Currency::Busd,
            "DAI" => Currency::Dai,

            // Crypto
            "BTC" => Currency::Btc,
            "ETH" => Currency::Eth,
            "BNB" => Currency::Bnb,
            "SOL" => Currency::Sol,
            "XRP" => Currency::Xrp,
            "ADA" => Currency::Ada,
            "DOGE" => Currency::Doge,
            "MATIC" => Currency::Matic,
            "LTC" => Currency::Ltc,

            // Fiat
            "USD" => Currency::Usd,
            "EUR" => Currency::Eur,
            "GBP" => Currency::Gbp,
            "JPY" => Currency::Jpy,
            "VND" => Currency::Vnd,

            // Other
            _ => Currency::Other(s),
        })
    }
}

impl TryFrom<String> for Currency {
    type Error = CurrencyError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Currency> for String {
    fn from(c: Currency) -> Self {
        c.code().to_string()
    }
}

impl From<&str> for Currency {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or_else(|_| Currency::Other(s.to_uppercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_known_currencies() {
        assert_eq!("USDT".parse::<Currency>().unwrap(), Currency::Usdt);
        assert_eq!("btc".parse::<Currency>().unwrap(), Currency::Btc);
        assert_eq!("ETH".parse::<Currency>().unwrap(), Currency::Eth);
        assert_eq!("usd".parse::<Currency>().unwrap(), Currency::Usd);
    }

    #[test]
    fn test_parse_custom_token() {
        let custom: Currency = "MYTOKEN".parse().unwrap();
        assert_eq!(custom, Currency::Other("MYTOKEN".to_string()));
        assert_eq!(custom.to_string(), "MYTOKEN");
    }

    #[test]
    fn test_display() {
        assert_eq!(Currency::Usdt.to_string(), "USDT");
        assert_eq!(Currency::Btc.to_string(), "BTC");
        assert_eq!(Currency::Other("XYZ".to_string()).to_string(), "XYZ");
    }

    #[test]
    fn test_is_stablecoin() {
        assert!(Currency::Usdt.is_stablecoin());
        assert!(Currency::Usdc.is_stablecoin());
        assert!(!Currency::Btc.is_stablecoin());
        assert!(!Currency::Usd.is_stablecoin());
    }

    #[test]
    fn test_is_fiat() {
        assert!(Currency::Usd.is_fiat());
        assert!(Currency::Vnd.is_fiat());
        assert!(!Currency::Usdt.is_fiat());
        assert!(!Currency::Btc.is_fiat());
    }

    #[test]
    fn test_is_crypto() {
        assert!(Currency::Btc.is_crypto());
        assert!(Currency::Eth.is_crypto());
        assert!(!Currency::Usdt.is_crypto());
        assert!(!Currency::Usd.is_crypto());
    }

    #[test]
    fn test_empty_code_error() {
        let result: Result<Currency, _> = "".parse();
        assert!(matches!(result, Err(CurrencyError::EmptyCode)));
    }

    #[test]
    fn test_too_long_error() {
        let result: Result<Currency, _> = "VERYLONGCURRENCYNAME".parse();
        assert!(matches!(result, Err(CurrencyError::TooLong(_))));
    }

    #[test]
    fn test_invalid_format_error() {
        let result: Result<Currency, _> = "BTC-USD".parse();
        assert!(matches!(result, Err(CurrencyError::InvalidFormat(_))));
    }

    #[test]
    fn test_serde_roundtrip() {
        let currencies = vec![
            Currency::Usdt,
            Currency::Btc,
            Currency::Usd,
            Currency::Other("XYZ".to_string()),
        ];

        for currency in currencies {
            let json = serde_json::to_string(&currency).unwrap();
            let parsed: Currency = serde_json::from_str(&json).unwrap();
            assert_eq!(currency, parsed);
        }
    }

    #[test]
    fn test_from_str_trait() {
        let currency: Currency = "ETH".into();
        assert_eq!(currency, Currency::Eth);
    }
}
