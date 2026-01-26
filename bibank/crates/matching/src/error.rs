//! Matching engine errors

use rust_decimal::Decimal;
use thiserror::Error;

/// Matching engine errors
#[derive(Debug, Error)]
pub enum MatchingError {
    /// Order not found
    #[error("Order not found: {0}")]
    OrderNotFound(String),

    /// Order already exists
    #[error("Order already exists: {0}")]
    OrderAlreadyExists(String),

    /// Invalid order quantity
    #[error("Invalid order quantity: {0}")]
    InvalidQuantity(Decimal),

    /// Invalid order price
    #[error("Invalid order price: {0}")]
    InvalidPrice(Decimal),

    /// Order already cancelled
    #[error("Order already cancelled: {0}")]
    OrderAlreadyCancelled(String),

    /// Order already filled
    #[error("Order already filled: {0}")]
    OrderAlreadyFilled(String),

    /// Trading pair not found
    #[error("Trading pair not found: {0}")]
    PairNotFound(String),

    /// Self-trade prevention
    #[error("Self-trade not allowed")]
    SelfTradeNotAllowed,
}
