//! Mock Oracle for testing
//!
//! Provides configurable fixed prices for testing margin calculations.

use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::error::OracleError;
use crate::types::{Price, PriceOracle, TradingPair};

/// Mock Price Oracle for testing
///
/// Stores fixed prices that can be updated programmatically.
/// Useful for unit tests and integration tests.
pub struct MockOracle {
    /// Stored prices (pair -> price)
    prices: RwLock<HashMap<String, Price>>,
}

impl MockOracle {
    /// Create a new empty mock oracle
    pub fn new() -> Self {
        Self {
            prices: RwLock::new(HashMap::new()),
        }
    }

    /// Create a mock oracle with default trading pairs
    pub fn with_defaults() -> Self {
        let oracle = Self::new();

        // Set some default prices
        oracle.set_price(TradingPair::btc_usdt(), Decimal::from(50000));
        oracle.set_price(TradingPair::eth_usdt(), Decimal::from(3000));
        oracle.set_price(TradingPair::new("SOL", "USDT"), Decimal::from(100));
        oracle.set_price(TradingPair::new("BNB", "USDT"), Decimal::from(300));

        oracle
    }

    /// Set a fixed price for a trading pair
    pub fn set_price(&self, pair: TradingPair, price: Decimal) {
        let price_obj = Price::simple(pair.clone(), price);
        let mut prices = self.prices.write().unwrap();
        prices.insert(pair.to_string(), price_obj);
    }

    /// Set a price with bid/ask spread
    pub fn set_price_with_spread(&self, pair: TradingPair, bid: Decimal, ask: Decimal) {
        let last = (bid + ask) / Decimal::from(2);
        let price_obj = Price::new(pair.clone(), bid, ask, last);
        let mut prices = self.prices.write().unwrap();
        prices.insert(pair.to_string(), price_obj);
    }

    /// Remove a price (for testing pair not found error)
    pub fn remove_price(&self, pair: &TradingPair) {
        let mut prices = self.prices.write().unwrap();
        prices.remove(&pair.to_string());
    }

    /// Get number of configured pairs
    pub fn pair_count(&self) -> usize {
        self.prices.read().unwrap().len()
    }
}

impl Default for MockOracle {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[async_trait]
impl PriceOracle for MockOracle {
    async fn get_price(&self, pair: &TradingPair) -> Result<Price, OracleError> {
        let prices = self.prices.read().unwrap();
        prices
            .get(&pair.to_string())
            .cloned()
            .ok_or_else(|| OracleError::PairNotFound {
                pair: pair.to_string(),
            })
    }

    async fn supported_pairs(&self) -> Vec<TradingPair> {
        let prices = self.prices.read().unwrap();
        prices.values().map(|p| p.pair.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_oracle_default_prices() {
        let oracle = MockOracle::with_defaults();

        let btc_price = oracle.get_price(&TradingPair::btc_usdt()).await.unwrap();
        assert_eq!(btc_price.last, Decimal::from(50000));

        let eth_price = oracle.get_price(&TradingPair::eth_usdt()).await.unwrap();
        assert_eq!(eth_price.last, Decimal::from(3000));
    }

    #[tokio::test]
    async fn test_mock_oracle_set_price() {
        let oracle = MockOracle::new();
        let pair = TradingPair::new("DOGE", "USDT");

        // Initially not set
        assert!(oracle.get_price(&pair).await.is_err());

        // Set price
        oracle.set_price(pair.clone(), Decimal::from_str_exact("0.08").unwrap());

        // Now available
        let price = oracle.get_price(&pair).await.unwrap();
        assert_eq!(price.last, Decimal::from_str_exact("0.08").unwrap());
    }

    #[tokio::test]
    async fn test_mock_oracle_pair_not_found() {
        let oracle = MockOracle::new();
        let pair = TradingPair::new("UNKNOWN", "USDT");

        let result = oracle.get_price(&pair).await;
        assert!(matches!(result, Err(OracleError::PairNotFound { .. })));
    }

    #[tokio::test]
    async fn test_mock_oracle_supported_pairs() {
        let oracle = MockOracle::with_defaults();
        let pairs = oracle.supported_pairs().await;

        assert!(pairs.len() >= 4);
        assert!(pairs.contains(&TradingPair::btc_usdt()));
        assert!(pairs.contains(&TradingPair::eth_usdt()));
    }

    #[tokio::test]
    async fn test_mock_oracle_with_spread() {
        let oracle = MockOracle::new();
        let pair = TradingPair::btc_usdt();

        oracle.set_price_with_spread(
            pair.clone(),
            Decimal::from(49900),
            Decimal::from(50100),
        );

        let price = oracle.get_price(&pair).await.unwrap();
        assert_eq!(price.bid, Decimal::from(49900));
        assert_eq!(price.ask, Decimal::from(50100));
        assert_eq!(price.mid(), Decimal::from(50000));
    }
}
