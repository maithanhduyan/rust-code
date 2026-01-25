//! Amount - Non-negative decimal wrapper for financial amounts
//!
//! All financial amounts in BiBank MUST be non-negative.
//! This is enforced at the type level.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Errors that can occur when working with amounts
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum AmountError {
    #[error("Amount cannot be negative: {0}")]
    NegativeAmount(Decimal),
}

/// A non-negative decimal amount for financial operations.
///
/// # Invariant
/// The inner value is always >= 0. This is enforced by the constructor.
///
/// # Example
/// ```
/// use bibank_core::Amount;
/// use rust_decimal::Decimal;
///
/// let amount = Amount::new(Decimal::new(100, 0)).unwrap();
/// assert_eq!(amount.value(), Decimal::new(100, 0));
///
/// // Negative amounts are rejected
/// let negative = Amount::new(Decimal::new(-100, 0));
/// assert!(negative.is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "Decimal", into = "Decimal")]
pub struct Amount(Decimal);

impl Amount {
    /// Zero amount constant
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Create a new Amount from a Decimal.
    ///
    /// Returns an error if the value is negative.
    pub fn new(value: Decimal) -> Result<Self, AmountError> {
        if value < Decimal::ZERO {
            Err(AmountError::NegativeAmount(value))
        } else {
            Ok(Self(value))
        }
    }

    /// Create an Amount without validation.
    ///
    /// # Safety
    /// The caller MUST ensure the value is non-negative.
    /// Use only for trusted sources (e.g., deserialization from validated storage).
    #[inline]
    pub const fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    /// Get the inner Decimal value
    #[inline]
    pub const fn value(&self) -> Decimal {
        self.0
    }

    /// Check if the amount is zero
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Saturating addition - returns the sum or panics on overflow
    pub fn checked_add(&self, other: &Amount) -> Option<Amount> {
        self.0.checked_add(other.0).map(Amount)
    }

    /// Saturating subtraction - returns None if result would be negative
    pub fn checked_sub(&self, other: &Amount) -> Option<Amount> {
        let result = self.0.checked_sub(other.0)?;
        if result < Decimal::ZERO {
            None
        } else {
            Some(Amount(result))
        }
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<Decimal> for Amount {
    type Error = AmountError;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Amount> for Decimal {
    fn from(amount: Amount) -> Self {
        amount.0
    }
}

impl Default for Amount {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_positive() {
        let amount = Amount::new(Decimal::new(100, 0)).unwrap();
        assert_eq!(amount.value(), Decimal::new(100, 0));
    }

    #[test]
    fn test_amount_zero() {
        let amount = Amount::new(Decimal::ZERO).unwrap();
        assert!(amount.is_zero());
    }

    #[test]
    fn test_amount_negative_rejected() {
        let result = Amount::new(Decimal::new(-100, 0));
        assert!(matches!(result, Err(AmountError::NegativeAmount(_))));
    }

    #[test]
    fn test_checked_sub_prevents_negative() {
        let a = Amount::new(Decimal::new(50, 0)).unwrap();
        let b = Amount::new(Decimal::new(100, 0)).unwrap();
        assert!(a.checked_sub(&b).is_none());
    }

    #[test]
    fn test_checked_sub_success() {
        let a = Amount::new(Decimal::new(100, 0)).unwrap();
        let b = Amount::new(Decimal::new(30, 0)).unwrap();
        let result = a.checked_sub(&b).unwrap();
        assert_eq!(result.value(), Decimal::new(70, 0));
    }

    #[test]
    fn test_serde_roundtrip() {
        let amount = Amount::new(Decimal::new(12345, 2)).unwrap(); // 123.45
        let json = serde_json::to_string(&amount).unwrap();
        let parsed: Amount = serde_json::from_str(&json).unwrap();
        assert_eq!(amount, parsed);
    }
}
