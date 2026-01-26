//! Matching engine for multiple trading pairs

use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::error::MatchingError;
use crate::fill::MatchResult;
use crate::order::{Order, OrderId, OrderSide, TradingPair};
use crate::orderbook::OrderBook;

/// Central matching engine managing multiple order books
#[derive(Debug)]
pub struct MatchingEngine {
    /// Order books indexed by trading pair symbol (e.g., "BTC/USDT")
    books: HashMap<String, OrderBook>,
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchingEngine {
    /// Create a new matching engine
    pub fn new() -> Self {
        Self {
            books: HashMap::new(),
        }
    }

    /// Create a matching engine with pre-configured trading pairs
    pub fn with_pairs(pairs: Vec<TradingPair>) -> Self {
        let mut engine = Self::new();
        for pair in pairs {
            engine.add_pair(pair);
        }
        engine
    }

    /// Add a new trading pair
    pub fn add_pair(&mut self, pair: TradingPair) {
        let key = pair.to_string();
        self.books.entry(key).or_insert_with(|| OrderBook::new(pair));
    }

    /// Check if a trading pair exists
    pub fn has_pair(&self, pair: &TradingPair) -> bool {
        self.books.contains_key(&pair.to_string())
    }

    /// Get a reference to an order book
    pub fn get_book(&self, pair: &TradingPair) -> Option<&OrderBook> {
        self.books.get(&pair.to_string())
    }

    /// Get a mutable reference to an order book
    fn get_book_mut(&mut self, pair: &TradingPair) -> Option<&mut OrderBook> {
        self.books.get_mut(&pair.to_string())
    }

    /// Place a new order
    ///
    /// The order will be matched against the opposite side of the book.
    /// Any remaining quantity will be added to the book as a resting order.
    pub fn place_order(&mut self, order: Order) -> Result<MatchResult, MatchingError> {
        let pair = order.pair.clone();

        let book = self
            .get_book_mut(&pair)
            .ok_or_else(|| MatchingError::PairNotFound(pair.to_string()))?;

        book.match_order(order)
    }

    /// Place a new limit order with parameters
    pub fn place_limit_order(
        &mut self,
        user_id: impl Into<String>,
        pair: TradingPair,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
    ) -> Result<(Order, MatchResult), MatchingError> {
        let order = Order::new(user_id, pair, side, price, quantity);
        let order_clone = order.clone();
        let result = self.place_order(order)?;
        Ok((order_clone, result))
    }

    /// Cancel an order
    pub fn cancel_order(&mut self, pair: &TradingPair, order_id: &str) -> Result<Order, MatchingError> {
        let book = self
            .get_book_mut(pair)
            .ok_or_else(|| MatchingError::PairNotFound(pair.to_string()))?;

        book.cancel_order(order_id)
    }

    /// Get an order by ID
    pub fn get_order(&self, pair: &TradingPair, order_id: &str) -> Option<&Order> {
        self.get_book(pair)?.get_order(order_id)
    }

    /// Get the best bid price for a pair
    pub fn best_bid(&self, pair: &TradingPair) -> Option<Decimal> {
        self.get_book(pair)?.best_bid()
    }

    /// Get the best ask price for a pair
    pub fn best_ask(&self, pair: &TradingPair) -> Option<Decimal> {
        self.get_book(pair)?.best_ask()
    }

    /// Get the spread for a pair
    pub fn spread(&self, pair: &TradingPair) -> Option<Decimal> {
        self.get_book(pair)?.spread()
    }

    /// Get the mid price for a pair
    pub fn mid_price(&self, pair: &TradingPair) -> Option<Decimal> {
        self.get_book(pair)?.mid_price()
    }

    /// Get order book depth
    pub fn get_depth(
        &self,
        pair: &TradingPair,
        levels: usize,
    ) -> Option<OrderBookDepth> {
        let book = self.get_book(pair)?;
        Some(OrderBookDepth {
            pair: pair.clone(),
            bids: book.get_bids(levels),
            asks: book.get_asks(levels),
        })
    }

    /// Get all trading pairs
    pub fn pairs(&self) -> Vec<TradingPair> {
        self.books.values().map(|b| b.pair().clone()).collect()
    }

    /// Total order count across all books
    pub fn total_order_count(&self) -> usize {
        self.books.values().map(|b| b.order_count()).sum()
    }
}

/// Order book depth snapshot
#[derive(Debug, Clone)]
pub struct OrderBookDepth {
    pub pair: TradingPair,
    /// Bids: (price, quantity) sorted by price descending
    pub bids: Vec<(Decimal, Decimal)>,
    /// Asks: (price, quantity) sorted by price ascending
    pub asks: Vec<(Decimal, Decimal)>,
}

impl OrderBookDepth {
    /// Get the best bid price
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.first().map(|(p, _)| *p)
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.first().map(|(p, _)| *p)
    }

    /// Get the spread
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }
}

/// Builder for placing orders with validation
pub struct OrderBuilder {
    user_id: Option<String>,
    pair: Option<TradingPair>,
    side: Option<OrderSide>,
    price: Option<Decimal>,
    quantity: Option<Decimal>,
}

impl OrderBuilder {
    pub fn new() -> Self {
        Self {
            user_id: None,
            pair: None,
            side: None,
            price: None,
            quantity: None,
        }
    }

    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn pair(mut self, pair: TradingPair) -> Self {
        self.pair = Some(pair);
        self
    }

    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    pub fn buy(self) -> Self {
        self.side(OrderSide::Buy)
    }

    pub fn sell(self) -> Self {
        self.side(OrderSide::Sell)
    }

    pub fn price(mut self, price: Decimal) -> Self {
        self.price = Some(price);
        self
    }

    pub fn quantity(mut self, quantity: Decimal) -> Self {
        self.quantity = Some(quantity);
        self
    }

    pub fn build(self) -> Result<Order, MatchingError> {
        let user_id = self.user_id.ok_or_else(|| {
            MatchingError::InvalidQuantity(Decimal::ZERO) // TODO: Better error
        })?;
        let pair = self.pair.ok_or_else(|| {
            MatchingError::PairNotFound("unspecified".to_string())
        })?;
        let side = self.side.ok_or_else(|| {
            MatchingError::InvalidQuantity(Decimal::ZERO) // TODO: Better error
        })?;
        let price = self.price.ok_or_else(|| {
            MatchingError::InvalidPrice(Decimal::ZERO)
        })?;
        let quantity = self.quantity.ok_or_else(|| {
            MatchingError::InvalidQuantity(Decimal::ZERO)
        })?;

        if price <= Decimal::ZERO {
            return Err(MatchingError::InvalidPrice(price));
        }
        if quantity <= Decimal::ZERO {
            return Err(MatchingError::InvalidQuantity(quantity));
        }

        Ok(Order::new(user_id, pair, side, price, quantity))
    }
}

impl Default for OrderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_engine() -> MatchingEngine {
        MatchingEngine::with_pairs(vec![
            TradingPair::btc_usdt(),
            TradingPair::eth_usdt(),
        ])
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_engine();
        assert!(engine.has_pair(&TradingPair::btc_usdt()));
        assert!(engine.has_pair(&TradingPair::eth_usdt()));
        assert!(!engine.has_pair(&TradingPair::new("SOL", "USDT")));
    }

    #[test]
    fn test_place_order() {
        let mut engine = create_engine();

        let order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );

        let result = engine.place_order(order).unwrap();
        assert!(result.fills.is_empty());
        assert_eq!(engine.best_bid(&TradingPair::btc_usdt()), Some(dec!(50000)));
    }

    #[test]
    fn test_place_limit_order() {
        let mut engine = create_engine();

        let (order, result) = engine
            .place_limit_order("ALICE", TradingPair::btc_usdt(), OrderSide::Sell, dec!(51000), dec!(2))
            .unwrap();

        assert!(result.fills.is_empty());
        assert_eq!(order.quantity, dec!(2));
        assert_eq!(engine.best_ask(&TradingPair::btc_usdt()), Some(dec!(51000)));
    }

    #[test]
    fn test_matching() {
        let mut engine = create_engine();

        // Place sell order
        engine
            .place_limit_order("BOB", TradingPair::btc_usdt(), OrderSide::Sell, dec!(50000), dec!(1))
            .unwrap();

        // Place matching buy order
        let (_, result) = engine
            .place_limit_order("ALICE", TradingPair::btc_usdt(), OrderSide::Buy, dec!(50000), dec!(1))
            .unwrap();

        assert_eq!(result.fills.len(), 1);
        assert!(result.fully_filled);
        assert_eq!(result.fills[0].quantity, dec!(1));
        assert_eq!(engine.total_order_count(), 0);
    }

    #[test]
    fn test_cancel_order() {
        let mut engine = create_engine();

        let order = Order::with_id(
            "order-1",
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        engine.place_order(order).unwrap();

        let cancelled = engine.cancel_order(&TradingPair::btc_usdt(), "order-1").unwrap();
        assert_eq!(cancelled.id, "order-1");
        assert_eq!(engine.total_order_count(), 0);
    }

    #[test]
    fn test_pair_not_found() {
        let mut engine = create_engine();

        let order = Order::new(
            "ALICE",
            TradingPair::new("SOL", "USDT"),
            OrderSide::Buy,
            dec!(100),
            dec!(10),
        );

        let result = engine.place_order(order);
        assert!(matches!(result, Err(MatchingError::PairNotFound(_))));
    }

    #[test]
    fn test_order_book_depth() {
        let mut engine = create_engine();

        // Add some orders
        engine
            .place_limit_order("ALICE", TradingPair::btc_usdt(), OrderSide::Buy, dec!(50000), dec!(1))
            .unwrap();
        engine
            .place_limit_order("BOB", TradingPair::btc_usdt(), OrderSide::Buy, dec!(49900), dec!(2))
            .unwrap();
        engine
            .place_limit_order("CAROL", TradingPair::btc_usdt(), OrderSide::Sell, dec!(50100), dec!(1))
            .unwrap();

        let depth = engine.get_depth(&TradingPair::btc_usdt(), 10).unwrap();

        assert_eq!(depth.bids.len(), 2);
        assert_eq!(depth.asks.len(), 1);
        assert_eq!(depth.best_bid(), Some(dec!(50000)));
        assert_eq!(depth.best_ask(), Some(dec!(50100)));
        assert_eq!(depth.spread(), Some(dec!(100)));
    }

    #[test]
    fn test_order_builder() {
        let order = OrderBuilder::new()
            .user_id("ALICE")
            .pair(TradingPair::btc_usdt())
            .buy()
            .price(dec!(50000))
            .quantity(dec!(1))
            .build()
            .unwrap();

        assert_eq!(order.user_id, "ALICE");
        assert_eq!(order.side, OrderSide::Buy);
    }

    #[test]
    fn test_order_builder_invalid() {
        let result = OrderBuilder::new()
            .user_id("ALICE")
            .pair(TradingPair::btc_usdt())
            .buy()
            .price(dec!(-100)) // Invalid
            .quantity(dec!(1))
            .build();

        assert!(matches!(result, Err(MatchingError::InvalidPrice(_))));
    }

    #[test]
    fn test_multiple_pairs() {
        let mut engine = create_engine();

        // Place orders on different pairs
        engine
            .place_limit_order("ALICE", TradingPair::btc_usdt(), OrderSide::Buy, dec!(50000), dec!(1))
            .unwrap();
        engine
            .place_limit_order("BOB", TradingPair::eth_usdt(), OrderSide::Sell, dec!(3000), dec!(10))
            .unwrap();

        assert_eq!(engine.best_bid(&TradingPair::btc_usdt()), Some(dec!(50000)));
        assert_eq!(engine.best_ask(&TradingPair::eth_usdt()), Some(dec!(3000)));
        assert_eq!(engine.total_order_count(), 2);
    }
}
