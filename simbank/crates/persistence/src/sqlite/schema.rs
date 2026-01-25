//! Database schema definitions
//!
//! Row types cho sqlx mapping từ SQLite tables.
//! Schema được định nghĩa trong migrations/20260125_init.sql

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Row type cho bảng `wallet_types`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct WalletTypeRow {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

/// Row type cho bảng `currencies`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct CurrencyRow {
    pub code: String,
    pub name: String,
    pub decimals: i32,
    pub symbol: Option<String>,
}

/// Row type cho bảng `persons`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PersonRow {
    pub id: String,
    pub person_type: String,
    pub name: String,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Row type cho bảng `accounts`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct AccountRow {
    pub id: String,
    pub person_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Row type cho bảng `wallets`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct WalletRow {
    pub id: String,
    pub account_id: String,
    pub wallet_type: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Row type cho bảng `balances`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct BalanceRow {
    pub wallet_id: String,
    pub currency_code: String,
    pub available: String, // Decimal stored as TEXT
    pub locked: String,    // Decimal stored as TEXT
    pub updated_at: DateTime<Utc>,
}

/// Row type cho bảng `transactions`
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct TransactionRow {
    pub id: String,
    pub account_id: String,
    pub wallet_id: String,
    pub tx_type: String,
    pub amount: String, // Decimal stored as TEXT
    pub currency_code: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

// === Conversion implementations ===

impl From<&simbank_core::Currency> for CurrencyRow {
    fn from(currency: &simbank_core::Currency) -> Self {
        Self {
            code: currency.code.clone(),
            name: currency.name.clone(),
            decimals: currency.decimals as i32,
            symbol: Some(currency.symbol.clone()),
        }
    }
}

impl From<CurrencyRow> for simbank_core::Currency {
    fn from(row: CurrencyRow) -> Self {
        simbank_core::Currency::new(
            &row.code,
            &row.name,
            row.decimals as u8,
            row.symbol.as_deref().unwrap_or(""),
        )
    }
}

impl From<&simbank_core::Person> for PersonRow {
    fn from(person: &simbank_core::Person) -> Self {
        Self {
            id: person.id.clone(),
            person_type: person.person_type.as_str().to_string(),
            name: person.name.clone(),
            email: person.email.clone(),
            created_at: person.created_at,
        }
    }
}

impl From<&simbank_core::wallet::WalletType> for WalletTypeRow {
    fn from(wt: &simbank_core::wallet::WalletType) -> Self {
        let (name, desc) = match wt {
            simbank_core::wallet::WalletType::Spot => ("Spot Wallet", "For trading"),
            simbank_core::wallet::WalletType::Funding => ("Funding Wallet", "For deposit/withdraw"),
            simbank_core::wallet::WalletType::Margin => ("Margin Wallet", "For margin trading"),
            simbank_core::wallet::WalletType::Futures => ("Futures Wallet", "For futures contracts"),
            simbank_core::wallet::WalletType::Earn => ("Earn Wallet", "For staking/savings"),
        };
        Self {
            code: wt.as_str().to_string(),
            name: name.to_string(),
            description: Some(desc.to_string()),
        }
    }
}
