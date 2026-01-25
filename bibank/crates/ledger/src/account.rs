//! Ledger Account - Hierarchical account identifiers
//!
//! Format: CATEGORY:SEGMENT:ID:ASSET:SUB_ACCOUNT
//! Example: LIAB:USER:ALICE:USDT:AVAILABLE

use crate::entry::Side;
use crate::error::LedgerError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use strum_macros::{Display, EnumString};

/// Account category following standard accounting principles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountCategory {
    /// Assets - Resources owned by the system (Cash, Crypto vault)
    #[strum(serialize = "ASSET")]
    Asset,

    /// Liabilities - Obligations to users (User balances)
    #[strum(serialize = "LIAB")]
    Liability,

    /// Equity - Owner's stake in the system
    #[strum(serialize = "EQUITY")]
    Equity,

    /// Revenue - Income earned (Fees collected)
    #[strum(serialize = "REV")]
    Revenue,

    /// Expenses - Costs incurred
    #[strum(serialize = "EXP")]
    Expense,
}

impl AccountCategory {
    /// Returns the normal balance side for this category.
    ///
    /// - Assets and Expenses increase on Debit
    /// - Liabilities, Equity, and Revenue increase on Credit
    pub fn normal_balance(&self) -> Side {
        match self {
            AccountCategory::Asset | AccountCategory::Expense => Side::Debit,
            AccountCategory::Liability | AccountCategory::Equity | AccountCategory::Revenue => {
                Side::Credit
            }
        }
    }

    /// Short code for serialization
    pub fn code(&self) -> &'static str {
        match self {
            AccountCategory::Asset => "ASSET",
            AccountCategory::Liability => "LIAB",
            AccountCategory::Equity => "EQUITY",
            AccountCategory::Revenue => "REV",
            AccountCategory::Expense => "EXP",
        }
    }
}

/// Hierarchical ledger account key
///
/// Format: `CATEGORY:SEGMENT:ID:ASSET:SUB_ACCOUNT`
///
/// # Examples
/// - `ASSET:SYSTEM:VAULT:USDT:MAIN` - System USDT vault
/// - `LIAB:USER:ALICE:USDT:AVAILABLE` - Alice's available USDT balance
/// - `REV:SYSTEM:FEE:USDT:REVENUE` - USDT fee revenue
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountKey {
    /// Accounting category (ASSET, LIAB, EQUITY, REV, EXP)
    pub category: AccountCategory,

    /// Segment (USER, SYSTEM)
    pub segment: String,

    /// Entity identifier (ALICE, VAULT, FEE)
    pub id: String,

    /// Asset/currency code (USDT, BTC, USD)
    pub asset: String,

    /// Sub-account type (AVAILABLE, LOCKED, MAIN, VAULT, REVENUE)
    pub sub_account: String,
}

impl AccountKey {
    /// Create a new AccountKey
    pub fn new(
        category: AccountCategory,
        segment: impl Into<String>,
        id: impl Into<String>,
        asset: impl Into<String>,
        sub_account: impl Into<String>,
    ) -> Self {
        Self {
            category,
            segment: segment.into().to_uppercase(),
            id: id.into().to_uppercase(),
            asset: asset.into().to_uppercase(),
            sub_account: sub_account.into().to_uppercase(),
        }
    }

    /// Create a user available balance account
    pub fn user_available(user_id: impl Into<String>, asset: impl Into<String>) -> Self {
        Self::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "AVAILABLE",
        )
    }

    /// Create a user locked balance account
    pub fn user_locked(user_id: impl Into<String>, asset: impl Into<String>) -> Self {
        Self::new(
            AccountCategory::Liability,
            "USER",
            user_id,
            asset,
            "LOCKED",
        )
    }

    /// Create a system vault account
    pub fn system_vault(asset: impl Into<String>) -> Self {
        Self::new(AccountCategory::Asset, "SYSTEM", "VAULT", asset, "MAIN")
    }

    /// Create a fee revenue account
    pub fn fee_revenue(asset: impl Into<String>) -> Self {
        Self::new(AccountCategory::Revenue, "SYSTEM", "FEE", asset, "REVENUE")
    }
}

impl fmt::Display for AccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.category.code(),
            self.segment,
            self.id,
            self.asset,
            self.sub_account
        )
    }
}

impl FromStr for AccountKey {
    type Err = LedgerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 5 {
            return Err(LedgerError::InvalidAccountFormat(format!(
                "Expected 5 parts separated by ':', got {}: {}",
                parts.len(),
                s
            )));
        }

        let category = match parts[0].to_uppercase().as_str() {
            "ASSET" => AccountCategory::Asset,
            "LIAB" => AccountCategory::Liability,
            "EQUITY" => AccountCategory::Equity,
            "REV" => AccountCategory::Revenue,
            "EXP" => AccountCategory::Expense,
            other => return Err(LedgerError::UnknownCategory(other.to_string())),
        };

        Ok(AccountKey {
            category,
            segment: parts[1].to_uppercase(),
            id: parts[2].to_uppercase(),
            asset: parts[3].to_uppercase(),
            sub_account: parts[4].to_uppercase(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_account_key() {
        let key: AccountKey = "LIAB:USER:ALICE:USDT:AVAILABLE".parse().unwrap();
        assert_eq!(key.category, AccountCategory::Liability);
        assert_eq!(key.segment, "USER");
        assert_eq!(key.id, "ALICE");
        assert_eq!(key.asset, "USDT");
        assert_eq!(key.sub_account, "AVAILABLE");
    }

    #[test]
    fn test_account_key_display() {
        let key = AccountKey::user_available("ALICE", "USDT");
        assert_eq!(key.to_string(), "LIAB:USER:ALICE:USDT:AVAILABLE");
    }

    #[test]
    fn test_account_key_roundtrip() {
        let original = "ASSET:SYSTEM:VAULT:BTC:MAIN";
        let key: AccountKey = original.parse().unwrap();
        assert_eq!(key.to_string(), original);
    }

    #[test]
    fn test_normal_balance() {
        assert_eq!(AccountCategory::Asset.normal_balance(), Side::Debit);
        assert_eq!(AccountCategory::Liability.normal_balance(), Side::Credit);
        assert_eq!(AccountCategory::Revenue.normal_balance(), Side::Credit);
        assert_eq!(AccountCategory::Expense.normal_balance(), Side::Debit);
    }

    #[test]
    fn test_invalid_format() {
        let result: Result<AccountKey, _> = "LIAB:USER:ALICE".parse();
        assert!(matches!(result, Err(LedgerError::InvalidAccountFormat(_))));
    }
}
