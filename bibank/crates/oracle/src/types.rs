//! Core oracle types

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::OracleError;

/// A trading pair (e.g., BTC/USDT)
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

    /// Common pair constructors
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

/// A price quote with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    /// The trading pair
    pub pair: TradingPair,
    /// Bid price (highest buy order)
    pub bid: Decimal,
    /// Ask price (lowest sell order)
    pub ask: Decimal,
    /// Last traded price (or mid-price if no trades)
    pub last: Decimal,
    /// Timestamp when this price was fetched
    pub timestamp: DateTime<Utc>,
    /// Source of the price (e.g., "mock", "binance", "chainlink")
    pub source: String,
}

impl Price {
    /// Create a new price with bid/ask/last
    pub fn new(pair: TradingPair, bid: Decimal, ask: Decimal, last: Decimal) -> Self {
        Self {
            pair,
            bid,
            ask,
            last,
            timestamp: Utc::now(),
            source: "unknown".to_string(),
        }
    }

    /// Create a simple price with same bid/ask/last (for mocking)
    pub fn simple(pair: TradingPair, price: Decimal) -> Self {
        Self {
            pair,
            bid: price,
            ask: price,
            last: price,
            timestamp: Utc::now(),
            source: "mock".to_string(),
        }
    }

    /// Get mid-price (average of bid and ask)
    pub fn mid(&self) -> Decimal {
        (self.bid + self.ask) / Decimal::from(2)
    }

    /// Get spread (ask - bid)
    pub fn spread(&self) -> Decimal {
        self.ask - self.bid
    }

    /// Check if price is stale (older than threshold)
    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        let age = Utc::now().signed_duration_since(self.timestamp);
        age.num_seconds() > max_age_secs as i64
    }
}

/// Price Oracle trait - interface for price feeds
///
/// Implementations can be:
/// - MockOracle: For testing with fixed prices
/// - BinanceOracle: Real-time prices from Binance API
/// - ChainlinkOracle: On-chain prices from Chainlink
#[async_trait]
pub trait PriceOracle: Send + Sync {
    /// Get the current price for a trading pair
    async fn get_price(&self, pair: &TradingPair) -> Result<Price, OracleError>;

    /// Get prices for multiple pairs at once
    async fn get_prices(&self, pairs: &[TradingPair]) -> Vec<Result<Price, OracleError>> {
        let mut results = Vec::new();
        for pair in pairs {
            results.push(self.get_price(pair).await);
        }
        results
    }

    /// Get a list of all supported trading pairs
    async fn supported_pairs(&self) -> Vec<TradingPair>;

    /// Check if a trading pair is supported
    async fn is_supported(&self, pair: &TradingPair) -> bool {
        self.supported_pairs().await.contains(pair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trading_pair_display() {
        let pair = TradingPair::btc_usdt();
        assert_eq!(pair.to_string(), "BTC/USDT");
    }

    #[test]
    fn test_price_mid() {
        let pair = TradingPair::btc_usdt();
        let price = Price::new(
            pair,
            Decimal::from(99),
            Decimal::from(101),
            Decimal::from(100),
        );
        assert_eq!(price.mid(), Decimal::from(100));
    }

    #[test]
    fn test_price_spread() {
        let pair = TradingPair::btc_usdt();
        let price = Price::new(
            pair,
            Decimal::from(99),
            Decimal::from(101),
            Decimal::from(100),
        );
        assert_eq!(price.spread(), Decimal::from(2));
    }
}
