//! Order types and structures

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique order identifier
pub type OrderId = String;

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    /// Get the opposite side
    pub fn opposite(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        }
    }
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
        }
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Order is open and waiting for fills
    Open,
    /// Order is partially filled
    PartiallyFilled,
    /// Order is completely filled
    Filled,
    /// Order was cancelled
    Cancelled,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Open => write!(f, "open"),
            OrderStatus::PartiallyFilled => write!(f, "partially_filled"),
            OrderStatus::Filled => write!(f, "filled"),
            OrderStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Trading pair (e.g., BTC/USDT)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradingPair {
    /// Base asset (e.g., BTC)
    pub base: String,
    /// Quote asset (e.g., USDT)
    pub quote: String,
}

impl TradingPair {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into().to_uppercase(),
            quote: quote.into().to_uppercase(),
        }
    }

    pub fn btc_usdt() -> Self {
        Self::new("BTC", "USDT")
    }

    pub fn eth_usdt() -> Self {
        Self::new("ETH", "USDT")
    }
}

impl std::fmt::Display for TradingPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// A limit order in the order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Unique order ID
    pub id: OrderId,
    /// User who placed the order
    pub user_id: String,
    /// Trading pair
    pub pair: TradingPair,
    /// Buy or Sell
    pub side: OrderSide,
    /// Limit price
    pub price: Decimal,
    /// Original quantity
    pub quantity: Decimal,
    /// Filled quantity
    pub filled: Decimal,
    /// Current status
    pub status: OrderStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Order {
    /// Create a new order
    pub fn new(
        user_id: impl Into<String>,
        pair: TradingPair,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.into(),
            pair,
            side,
            price,
            quantity,
            filled: Decimal::ZERO,
            status: OrderStatus::Open,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create order with specific ID (for testing)
    pub fn with_id(
        id: impl Into<String>,
        user_id: impl Into<String>,
        pair: TradingPair,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            user_id: user_id.into(),
            pair,
            side,
            price,
            quantity,
            filled: Decimal::ZERO,
            status: OrderStatus::Open,
            created_at: now,
            updated_at: now,
        }
    }

    /// Remaining unfilled quantity
    pub fn remaining(&self) -> Decimal {
        self.quantity - self.filled
    }

    /// Check if order is fully filled
    pub fn is_filled(&self) -> bool {
        self.remaining() <= Decimal::ZERO
    }

    /// Check if order is active (can be matched)
    pub fn is_active(&self) -> bool {
        matches!(self.status, OrderStatus::Open | OrderStatus::PartiallyFilled)
    }

    /// Fill the order by a given quantity
    pub fn fill(&mut self, fill_quantity: Decimal) {
        self.filled += fill_quantity;
        self.updated_at = Utc::now();

        if self.is_filled() {
            self.status = OrderStatus::Filled;
        } else {
            self.status = OrderStatus::PartiallyFilled;
        }
    }

    /// Cancel the order
    pub fn cancel(&mut self) {
        self.status = OrderStatus::Cancelled;
        self.updated_at = Utc::now();
    }

    /// Notional value (price * quantity)
    pub fn notional_value(&self) -> Decimal {
        self.price * self.quantity
    }

    /// Remaining notional value
    pub fn remaining_notional(&self) -> Decimal {
        self.price * self.remaining()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_creation() {
        let order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );

        assert_eq!(order.user_id, "ALICE");
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.price, Decimal::from(50000));
        assert_eq!(order.quantity, Decimal::from(1));
        assert_eq!(order.filled, Decimal::ZERO);
        assert_eq!(order.status, OrderStatus::Open);
    }

    #[test]
    fn test_order_fill() {
        let mut order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );

        // Partial fill
        order.fill(Decimal::from_str_exact("0.3").unwrap());
        assert_eq!(order.filled, Decimal::from_str_exact("0.3").unwrap());
        assert_eq!(order.remaining(), Decimal::from_str_exact("0.7").unwrap());
        assert_eq!(order.status, OrderStatus::PartiallyFilled);

        // Complete fill
        order.fill(Decimal::from_str_exact("0.7").unwrap());
        assert_eq!(order.filled, Decimal::from(1));
        assert!(order.is_filled());
        assert_eq!(order.status, OrderStatus::Filled);
    }

    #[test]
    fn test_order_cancel() {
        let mut order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );

        order.cancel();
        assert_eq!(order.status, OrderStatus::Cancelled);
        assert!(!order.is_active());
    }

    #[test]
    fn test_trading_pair_display() {
        let pair = TradingPair::btc_usdt();
        assert_eq!(pair.to_string(), "BTC/USDT");
    }

    #[test]
    fn test_order_side_opposite() {
        assert_eq!(OrderSide::Buy.opposite(), OrderSide::Sell);
        assert_eq!(OrderSide::Sell.opposite(), OrderSide::Buy);
    }
}
