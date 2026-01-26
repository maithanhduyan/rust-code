//! BiBank Order Matching Engine
//!
//! CLOB (Central Limit Order Book) with price-time priority.
//! Phase 3: Limit GTC orders only.

mod engine;
mod error;
mod fill;
mod order;
mod orderbook;

pub use engine::{MatchingEngine, OrderBookDepth};
pub use error::MatchingError;
pub use fill::{Fill, MatchResult};
pub use order::{Order, OrderId, OrderSide, OrderStatus, TradingPair};
pub use orderbook::OrderBook;
