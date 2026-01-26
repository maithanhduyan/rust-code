//! Trade projection - tracks trade history from events

use bibank_ledger::{JournalEntry, TransactionIntent};
use rust_decimal::Decimal;
use sqlx::{Row, SqlitePool};

/// Trade record from projection
#[derive(Debug, Clone)]
pub struct TradeRecord {
    /// Trade ID (sequence number)
    pub trade_id: u64,
    /// Seller user ID
    pub seller: String,
    /// Buyer user ID
    pub buyer: String,
    /// Asset being sold
    pub sell_asset: String,
    /// Amount being sold
    pub sell_amount: Decimal,
    /// Asset being bought
    pub buy_asset: String,
    /// Amount being bought
    pub buy_amount: Decimal,
    /// Trade timestamp
    pub timestamp: String,
    /// Entry hash
    pub hash: String,
}

/// Trade projection - tracks trade history
pub struct TradeProjection {
    pool: SqlitePool,
}

impl TradeProjection {
    /// Create a new trade projection
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the schema
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                trade_id INTEGER PRIMARY KEY,
                seller TEXT NOT NULL,
                buyer TEXT NOT NULL,
                sell_asset TEXT NOT NULL,
                sell_amount TEXT NOT NULL,
                buy_asset TEXT NOT NULL,
                buy_amount TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                hash TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_seller
            ON trades(seller)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_buyer
            ON trades(buyer)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_trades_assets
            ON trades(sell_asset, buy_asset)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Apply a journal entry to update trades
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), sqlx::Error> {
        // Only process Trade entries
        if entry.intent != TransactionIntent::Trade {
            return Ok(());
        }

        // Extract trade info from postings
        // Trade has 4+ postings:
        // - Seller DEBIT (loses sell_asset)
        // - Seller CREDIT (gains buy_asset)
        // - Buyer DEBIT (loses buy_asset)
        // - Buyer CREDIT (gains sell_asset)

        let mut seller: Option<String> = None;
        let mut buyer: Option<String> = None;
        let mut sell_asset: Option<String> = None;
        let mut sell_amount: Option<Decimal> = None;
        let mut buy_asset: Option<String> = None;
        let mut buy_amount: Option<Decimal> = None;

        for posting in &entry.postings {
            // Only look at user LIAB accounts
            if posting.account.segment != "USER" {
                continue;
            }

            let user_id = &posting.account.id;
            let asset = &posting.account.asset;

            // DEBIT on LIAB = user is paying (losing)
            // CREDIT on LIAB = user is receiving (gaining)
            use bibank_ledger::entry::Side;

            match posting.side {
                Side::Debit => {
                    // User is losing this asset
                    if seller.is_none() || seller.as_ref() == Some(user_id) {
                        seller = Some(user_id.clone());
                        sell_asset = Some(asset.clone());
                        sell_amount = Some(posting.amount.value());
                    } else {
                        buyer = Some(user_id.clone());
                        buy_asset = Some(asset.clone());
                        buy_amount = Some(posting.amount.value());
                    }
                }
                Side::Credit => {
                    // User is gaining this asset
                    if buyer.is_none() || buyer.as_ref() == Some(user_id) {
                        buyer = Some(user_id.clone());
                    } else {
                        seller = Some(user_id.clone());
                    }
                }
            }
        }

        // Insert trade record
        if let (
            Some(seller),
            Some(buyer),
            Some(sell_asset),
            Some(sell_amount),
            Some(buy_asset),
            Some(buy_amount),
        ) = (seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount)
        {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO trades
                (trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(entry.sequence as i64)
            .bind(&seller)
            .bind(&buyer)
            .bind(&sell_asset)
            .bind(sell_amount.to_string())
            .bind(&buy_asset)
            .bind(buy_amount.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .bind(&entry.hash)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get all trades for a user (as seller or buyer)
    pub async fn get_user_trades(&self, user_id: &str) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            WHERE seller = ? OR buyer = ?
            ORDER BY trade_id DESC
            "#,
        )
        .bind(user_id.to_uppercase())
        .bind(user_id.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get trades for a trading pair
    pub async fn get_pair_trades(
        &self,
        base_asset: &str,
        quote_asset: &str,
    ) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            WHERE (sell_asset = ? AND buy_asset = ?) OR (sell_asset = ? AND buy_asset = ?)
            ORDER BY trade_id DESC
            "#,
        )
        .bind(base_asset.to_uppercase())
        .bind(quote_asset.to_uppercase())
        .bind(quote_asset.to_uppercase())
        .bind(base_asset.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get recent trades (all)
    pub async fn get_recent_trades(&self, limit: u32) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT trade_id, seller, buyer, sell_asset, sell_amount, buy_asset, buy_amount, timestamp, hash
            FROM trades
            ORDER BY trade_id DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                trade_id: row.get::<i64, _>("trade_id") as u64,
                seller: row.get("seller"),
                buyer: row.get("buyer"),
                sell_asset: row.get("sell_asset"),
                sell_amount: row
                    .get::<String, _>("sell_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                buy_asset: row.get("buy_asset"),
                buy_amount: row
                    .get::<String, _>("buy_amount")
                    .parse()
                    .unwrap_or(Decimal::ZERO),
                timestamp: row.get("timestamp"),
                hash: row.get("hash"),
            })
            .collect())
    }

    /// Get trade count
    pub async fn count(&self) -> Result<u64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM trades")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64, _>("count") as u64)
    }

    /// Clear all trades (for replay)
    pub async fn clear(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM trades")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
