//! Order book structure with price-time priority

use std::collections::{BTreeMap, VecDeque};

use rust_decimal::Decimal;

use crate::error::MatchingError;
use crate::fill::{Fill, MatchResult};
use crate::order::{Order, OrderId, OrderSide, OrderStatus, TradingPair};

/// An order book for a single trading pair
///
/// Uses CLOB (Central Limit Order Book) structure:
/// - Bids (buy orders): sorted by price descending (highest first)
/// - Asks (sell orders): sorted by price ascending (lowest first)
/// - At each price level: FIFO queue (time priority)
#[derive(Debug)]
pub struct OrderBook {
    /// Trading pair
    pair: TradingPair,
    /// Buy orders: price -> orders at that price (price descending for matching)
    bids: BTreeMap<PriceLevel, VecDeque<Order>>,
    /// Sell orders: price -> orders at that price (price ascending for matching)
    asks: BTreeMap<PriceLevel, VecDeque<Order>>,
    /// All orders indexed by ID (for fast lookup and cancellation)
    orders: std::collections::HashMap<OrderId, OrderLocation>,
}

/// Price level wrapper for custom ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PriceLevel(Decimal);

impl PriceLevel {
    fn new(price: Decimal) -> Self {
        Self(price)
    }
}

impl PartialOrd for PriceLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriceLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Use total ordering for Decimal
        self.0.cmp(&other.0)
    }
}

/// Location of an order in the book
#[derive(Debug, Clone)]
struct OrderLocation {
    side: OrderSide,
    price: PriceLevel,
}

impl OrderBook {
    /// Create a new order book for a trading pair
    pub fn new(pair: TradingPair) -> Self {
        Self {
            pair,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: std::collections::HashMap::new(),
        }
    }

    /// Get the trading pair
    pub fn pair(&self) -> &TradingPair {
        &self.pair
    }

    /// Get the best bid price (highest buy price)
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.keys().next_back().map(|p| p.0)
    }

    /// Get the best ask price (lowest sell price)
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.keys().next().map(|p| p.0)
    }

    /// Get the spread (ask - bid)
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Get the mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some((ask + bid) / Decimal::from(2)),
            _ => None,
        }
    }

    /// Count of all active orders
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    /// Total bid volume
    pub fn total_bid_volume(&self) -> Decimal {
        self.bids
            .values()
            .flat_map(|q| q.iter())
            .map(|o| o.remaining())
            .sum()
    }

    /// Total ask volume
    pub fn total_ask_volume(&self) -> Decimal {
        self.asks
            .values()
            .flat_map(|q| q.iter())
            .map(|o| o.remaining())
            .sum()
    }

    /// Add an order to the book (after matching, if any remaining)
    fn add_order(&mut self, order: Order) {
        let price = PriceLevel::new(order.price);
        let side = order.side;
        let order_id = order.id.clone();

        let book = match side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        };

        book.entry(price).or_insert_with(VecDeque::new).push_back(order);
        self.orders.insert(order_id, OrderLocation { side, price });
    }

    /// Remove an order from the book
    fn remove_order(&mut self, order_id: &str) -> Option<Order> {
        let location = self.orders.remove(order_id)?;

        let book = match location.side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        };

        if let Some(queue) = book.get_mut(&location.price) {
            let pos = queue.iter().position(|o| o.id == order_id)?;
            let order = queue.remove(pos)?;

            // Clean up empty price levels
            if queue.is_empty() {
                book.remove(&location.price);
            }

            return Some(order);
        }

        None
    }

    /// Get an order by ID
    pub fn get_order(&self, order_id: &str) -> Option<&Order> {
        let location = self.orders.get(order_id)?;

        let book = match location.side {
            OrderSide::Buy => &self.bids,
            OrderSide::Sell => &self.asks,
        };

        book.get(&location.price)?
            .iter()
            .find(|o| o.id == order_id)
    }

    /// Match an incoming order against the book
    ///
    /// Returns fills and any remaining quantity
    pub fn match_order(&mut self, mut order: Order) -> Result<MatchResult, MatchingError> {
        // Validate order
        if order.quantity <= Decimal::ZERO {
            return Err(MatchingError::InvalidQuantity(order.quantity));
        }
        if order.price <= Decimal::ZERO {
            return Err(MatchingError::InvalidPrice(order.price));
        }
        if order.pair != self.pair {
            return Err(MatchingError::PairNotFound(order.pair.to_string()));
        }

        let mut result = MatchResult::empty(order.quantity);

        // Get opposite side book
        let can_match = match order.side {
            OrderSide::Buy => {
                // Buy order matches against asks (sellers)
                // Match if ask price <= buy price
                |ask_price: Decimal, buy_price: Decimal| ask_price <= buy_price
            }
            OrderSide::Sell => {
                // Sell order matches against bids (buyers)
                // Match if bid price >= sell price
                |bid_price: Decimal, sell_price: Decimal| bid_price >= sell_price
            }
        };

        // Collect matched orders to remove after iteration
        let mut filled_order_ids: Vec<OrderId> = Vec::new();

        loop {
            if order.remaining() <= Decimal::ZERO {
                result.fully_filled = true;
                break;
            }

            // Get best price from opposite side
            let best_price = match order.side {
                OrderSide::Buy => self.best_ask(),
                OrderSide::Sell => self.best_bid(),
            };

            let best_price = match best_price {
                Some(p) if can_match(p, order.price) => p,
                _ => break, // No more matchable orders
            };

            // Get orders at best price
            let opposite_book = match order.side {
                OrderSide::Buy => &mut self.asks,
                OrderSide::Sell => &mut self.bids,
            };

            let price_level = PriceLevel::new(best_price);
            let queue = match opposite_book.get_mut(&price_level) {
                Some(q) => q,
                None => break,
            };

            // Match against orders at this price level (FIFO)
            while let Some(maker_order) = queue.front_mut() {
                if order.remaining() <= Decimal::ZERO {
                    break;
                }

                // Self-trade prevention
                if maker_order.user_id == order.user_id {
                    return Err(MatchingError::SelfTradeNotAllowed);
                }

                let fill_qty = order.remaining().min(maker_order.remaining());

                // Create fill
                let fill = Fill::new(
                    self.pair.clone(),
                    order.id.clone(),
                    maker_order.id.clone(),
                    order.user_id.clone(),
                    maker_order.user_id.clone(),
                    order.side,
                    maker_order.price, // Execute at maker's price
                    fill_qty,
                );

                result.fills.push(fill);

                // Update quantities
                order.fill(fill_qty);
                maker_order.fill(fill_qty);

                // Remove filled maker orders
                if maker_order.is_filled() {
                    let maker_id = maker_order.id.clone();
                    filled_order_ids.push(maker_id);
                    queue.pop_front();
                }
            }

            // Clean up empty price levels
            if queue.is_empty() {
                opposite_book.remove(&price_level);
            }
        }

        // Remove filled orders from index
        for order_id in filled_order_ids {
            self.orders.remove(&order_id);
        }

        result.remaining_quantity = order.remaining();

        // If order has remaining quantity, add it to the book
        if order.remaining() > Decimal::ZERO && order.is_active() {
            self.add_order(order);
        }

        Ok(result)
    }

    /// Cancel an order
    pub fn cancel_order(&mut self, order_id: &str) -> Result<Order, MatchingError> {
        let mut order = self.remove_order(order_id)
            .ok_or_else(|| MatchingError::OrderNotFound(order_id.to_string()))?;

        if order.status == OrderStatus::Cancelled {
            return Err(MatchingError::OrderAlreadyCancelled(order_id.to_string()));
        }
        if order.status == OrderStatus::Filled {
            return Err(MatchingError::OrderAlreadyFilled(order_id.to_string()));
        }

        order.cancel();
        Ok(order)
    }

    /// Get all bids as (price, total_quantity) tuples, sorted by price descending
    pub fn get_bids(&self, depth: usize) -> Vec<(Decimal, Decimal)> {
        self.bids
            .iter()
            .rev() // Descending by price
            .take(depth)
            .map(|(price, orders)| {
                let total_qty: Decimal = orders.iter().map(|o| o.remaining()).sum();
                (price.0, total_qty)
            })
            .collect()
    }

    /// Get all asks as (price, total_quantity) tuples, sorted by price ascending
    pub fn get_asks(&self, depth: usize) -> Vec<(Decimal, Decimal)> {
        self.asks
            .iter()
            .take(depth)
            .map(|(price, orders)| {
                let total_qty: Decimal = orders.iter().map(|o| o.remaining()).sum();
                (price.0, total_qty)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_book() -> OrderBook {
        OrderBook::new(TradingPair::btc_usdt())
    }

    #[test]
    fn test_empty_orderbook() {
        let book = create_test_book();
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.spread(), None);
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_add_buy_order_no_match() {
        let mut book = create_test_book();

        let order = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );

        let result = book.match_order(order).unwrap();
        assert!(result.fills.is_empty());
        assert_eq!(result.remaining_quantity, dec!(1));
        assert!(!result.fully_filled);
        assert_eq!(book.best_bid(), Some(dec!(50000)));
        assert_eq!(book.order_count(), 1);
    }

    #[test]
    fn test_add_sell_order_no_match() {
        let mut book = create_test_book();

        let order = Order::new(
            "BOB",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(51000),
            dec!(2),
        );

        let result = book.match_order(order).unwrap();
        assert!(result.fills.is_empty());
        assert_eq!(book.best_ask(), Some(dec!(51000)));
    }

    #[test]
    fn test_full_match() {
        let mut book = create_test_book();

        // Add sell order
        let sell = Order::new(
            "BOB",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(1),
        );
        book.match_order(sell).unwrap();

        // Add matching buy order
        let buy = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        let result = book.match_order(buy).unwrap();

        assert_eq!(result.fills.len(), 1);
        assert!(result.fully_filled);
        assert_eq!(result.fills[0].quantity, dec!(1));
        assert_eq!(result.fills[0].price, dec!(50000));
        assert_eq!(book.order_count(), 0); // Both orders filled
    }

    #[test]
    fn test_partial_match() {
        let mut book = create_test_book();

        // Add small sell order
        let sell = Order::new(
            "BOB",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(0.5),
        );
        book.match_order(sell).unwrap();

        // Add larger buy order
        let buy = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        let result = book.match_order(buy).unwrap();

        assert_eq!(result.fills.len(), 1);
        assert!(!result.fully_filled);
        assert_eq!(result.remaining_quantity, dec!(0.5));
        assert_eq!(book.best_bid(), Some(dec!(50000))); // Remaining buy order
        assert_eq!(book.best_ask(), None); // Sell order fully filled
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = create_test_book();

        // Add two sell orders at same price
        let sell1 = Order::with_id(
            "sell-1",
            "BOB",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(1),
        );
        book.match_order(sell1).unwrap();

        let sell2 = Order::with_id(
            "sell-2",
            "CAROL",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(1),
        );
        book.match_order(sell2).unwrap();

        // Buy should match with first seller (BOB)
        let buy = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        let result = book.match_order(buy).unwrap();

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].maker_order_id, "sell-1");
        assert_eq!(result.fills[0].maker_user_id, "BOB");
    }

    #[test]
    fn test_cancel_order() {
        let mut book = create_test_book();

        let order = Order::with_id(
            "order-1",
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        book.match_order(order).unwrap();

        let cancelled = book.cancel_order("order-1").unwrap();
        assert_eq!(cancelled.status, OrderStatus::Cancelled);
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_cancel_nonexistent_order() {
        let mut book = create_test_book();
        let result = book.cancel_order("nonexistent");
        assert!(matches!(result, Err(MatchingError::OrderNotFound(_))));
    }

    #[test]
    fn test_self_trade_prevention() {
        let mut book = create_test_book();

        let sell = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Sell,
            dec!(50000),
            dec!(1),
        );
        book.match_order(sell).unwrap();

        // Same user tries to buy
        let buy = Order::new(
            "ALICE",
            TradingPair::btc_usdt(),
            OrderSide::Buy,
            dec!(50000),
            dec!(1),
        );
        let result = book.match_order(buy);
        assert!(matches!(result, Err(MatchingError::SelfTradeNotAllowed)));
    }

    #[test]
    fn test_depth() {
        let mut book = create_test_book();

        // Add bids at different prices
        for price in [49000, 49500, 50000].iter() {
            let order = Order::new(
                "ALICE",
                TradingPair::btc_usdt(),
                OrderSide::Buy,
                Decimal::from(*price),
                dec!(1),
            );
            book.match_order(order).unwrap();
        }

        let bids = book.get_bids(10);
        assert_eq!(bids.len(), 3);
        // Sorted descending
        assert_eq!(bids[0].0, dec!(50000));
        assert_eq!(bids[1].0, dec!(49500));
        assert_eq!(bids[2].0, dec!(49000));
    }
}
