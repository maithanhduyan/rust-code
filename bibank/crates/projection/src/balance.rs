//! Balance projection - tracks account balances from events

use bibank_ledger::JournalEntry;
use rust_decimal::Decimal;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

/// Balance projection - tracks account balances
pub struct BalanceProjection {
    pool: SqlitePool,
}

impl BalanceProjection {
    /// Create a new balance projection
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the schema
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS balances (
                account_key TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                segment TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                asset TEXT NOT NULL,
                sub_account TEXT NOT NULL,
                balance TEXT NOT NULL DEFAULT '0',
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_balances_entity
            ON balances(entity_id, asset)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Apply a journal entry to update balances
    pub async fn apply(&self, entry: &JournalEntry) -> Result<(), sqlx::Error> {
        for posting in &entry.postings {
            let key = posting.account.to_string();
            let normal_side = posting.account.category.normal_balance();

            let delta = if posting.side == normal_side {
                posting.amount.value()
            } else {
                -posting.amount.value()
            };

            // Upsert balance
            sqlx::query(
                r#"
                INSERT INTO balances (account_key, category, segment, entity_id, asset, sub_account, balance, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(account_key) DO UPDATE SET
                    balance = CAST((CAST(balance AS REAL) + CAST(? AS REAL)) AS TEXT),
                    updated_at = ?
                "#,
            )
            .bind(&key)
            .bind(posting.account.category.code())
            .bind(&posting.account.segment)
            .bind(&posting.account.id)
            .bind(&posting.account.asset)
            .bind(&posting.account.sub_account)
            .bind(delta.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .bind(delta.to_string())
            .bind(entry.timestamp.to_rfc3339())
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get balance for a specific account
    pub async fn get_balance(&self, account_key: &str) -> Result<Decimal, sqlx::Error> {
        let row = sqlx::query("SELECT balance FROM balances WHERE account_key = ?")
            .bind(account_key)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let balance_str: String = row.get("balance");
                Ok(balance_str.parse().unwrap_or(Decimal::ZERO))
            }
            None => Ok(Decimal::ZERO),
        }
    }

    /// Get all balances for a user
    pub async fn get_user_balances(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, Decimal>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT asset, balance
            FROM balances
            WHERE segment = 'USER' AND entity_id = ? AND sub_account = 'AVAILABLE'
            "#,
        )
        .bind(user_id.to_uppercase())
        .fetch_all(&self.pool)
        .await?;

        let mut balances = HashMap::new();
        for row in rows {
            let asset: String = row.get("asset");
            let balance_str: String = row.get("balance");
            let balance: Decimal = balance_str.parse().unwrap_or(Decimal::ZERO);
            balances.insert(asset, balance);
        }

        Ok(balances)
    }

    /// Clear all balances (for replay)
    pub async fn clear(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM balances")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
