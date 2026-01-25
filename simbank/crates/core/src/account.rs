//! # Account Module
//!
//! Định nghĩa Account - đại diện cho tài khoản của người dùng.
//! Mỗi Account có quan hệ 1:1 với Person và chứa nhiều Wallets.

use crate::person::Person;
use crate::wallet::{Wallet, WalletType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Trạng thái của Account
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    /// Tài khoản hoạt động bình thường
    Active,
    /// Tài khoản bị đóng băng (nghi ngờ gian lận, vi phạm)
    Frozen,
    /// Tài khoản đã đóng
    Closed,
}

impl AccountStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountStatus::Active => "active",
            AccountStatus::Frozen => "frozen",
            AccountStatus::Closed => "closed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(AccountStatus::Active),
            "frozen" => Some(AccountStatus::Frozen),
            "closed" => Some(AccountStatus::Closed),
            _ => None,
        }
    }
}

impl fmt::Display for AccountStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tài khoản của người dùng.
///
/// Mỗi Account:
/// - Thuộc về một Person (1:1)
/// - Chứa nhiều Wallets (Spot, Funding, ...)
/// - Có trạng thái (Active, Frozen, Closed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// ID của account (ACC_001, ACC_002, ...)
    pub id: String,
    /// ID của person sở hữu account này
    pub person_id: String,
    /// Trạng thái account
    pub status: AccountStatus,
    /// Map từ WalletType -> Wallet
    pub wallets: HashMap<WalletType, Wallet>,
    /// Thời gian tạo
    pub created_at: DateTime<Utc>,
}

impl Account {
    /// Tạo Account mới với các wallets mặc định (Phase 1: Spot + Funding)
    pub fn new(id: String, person_id: String) -> Self {
        let mut account = Self {
            id: id.clone(),
            person_id,
            status: AccountStatus::Active,
            wallets: HashMap::new(),
            created_at: Utc::now(),
        };

        // Eager creation: Tạo sẵn các wallets cho Phase 1
        account.create_default_wallets();
        account
    }

    /// Tạo Account từ Person
    pub fn from_person(account_id: &str, person: &Person) -> Self {
        Self::new(account_id.to_string(), person.id.clone())
    }

    /// Tạo các wallets mặc định (Phase 1: Spot + Funding)
    fn create_default_wallets(&mut self) {
        let mut wallet_counter = 1;
        for wallet_type in WalletType::phase1_types() {
            let wallet_id = format!("WAL_{:03}", wallet_counter);
            let wallet = Wallet::new(wallet_id, self.id.clone(), wallet_type);
            self.wallets.insert(wallet_type, wallet);
            wallet_counter += 1;
        }
    }

    /// Lấy wallet theo loại
    pub fn get_wallet(&self, wallet_type: WalletType) -> Option<&Wallet> {
        self.wallets.get(&wallet_type)
    }

    /// Lấy mutable wallet theo loại
    pub fn get_wallet_mut(&mut self, wallet_type: WalletType) -> Option<&mut Wallet> {
        self.wallets.get_mut(&wallet_type)
    }

    /// Lấy Spot wallet
    pub fn spot(&self) -> Option<&Wallet> {
        self.get_wallet(WalletType::Spot)
    }

    /// Lấy Funding wallet
    pub fn funding(&self) -> Option<&Wallet> {
        self.get_wallet(WalletType::Funding)
    }

    /// Lấy mutable Spot wallet
    pub fn spot_mut(&mut self) -> Option<&mut Wallet> {
        self.get_wallet_mut(WalletType::Spot)
    }

    /// Lấy mutable Funding wallet
    pub fn funding_mut(&mut self) -> Option<&mut Wallet> {
        self.get_wallet_mut(WalletType::Funding)
    }

    /// Kiểm tra account có active không
    pub fn is_active(&self) -> bool {
        self.status == AccountStatus::Active
    }

    /// Freeze account
    pub fn freeze(&mut self) {
        self.status = AccountStatus::Frozen;
    }

    /// Activate account
    pub fn activate(&mut self) {
        self.status = AccountStatus::Active;
    }

    /// Close account
    pub fn close(&mut self) {
        self.status = AccountStatus::Closed;
    }

    /// Generate ID cho account mới
    pub fn generate_id(counter: u32) -> String {
        format!("ACC_{:03}", counter)
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Account {} (owner: {}, status: {}, wallets: {})",
            self.id,
            self.person_id,
            self.status,
            self.wallets.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::Currency;
    use rust_decimal_macros::dec;

    #[test]
    fn test_account_creation() {
        let account = Account::new("ACC_001".to_string(), "CUST_001".to_string());

        assert_eq!(account.id, "ACC_001");
        assert_eq!(account.person_id, "CUST_001");
        assert_eq!(account.status, AccountStatus::Active);
        assert!(account.is_active());

        // Should have 2 wallets (Spot + Funding) by default
        assert_eq!(account.wallets.len(), 2);
        assert!(account.spot().is_some());
        assert!(account.funding().is_some());
    }

    #[test]
    fn test_account_from_person() {
        let alice = Person::customer("CUST_001", "Alice");
        let account = Account::from_person("ACC_001", &alice);

        assert_eq!(account.person_id, "CUST_001");
    }

    #[test]
    fn test_account_wallet_operations() {
        let mut account = Account::new("ACC_001".to_string(), "CUST_001".to_string());

        // Credit to funding wallet
        if let Some(funding) = account.funding_mut() {
            funding.credit(Currency::usdt(), dec!(1000));
        }

        // Verify balance
        let balance = account
            .funding()
            .and_then(|w| w.get_balance("USDT"))
            .map(|b| b.available);

        assert_eq!(balance, Some(dec!(1000)));
    }

    #[test]
    fn test_account_status_transitions() {
        let mut account = Account::new("ACC_001".to_string(), "CUST_001".to_string());

        assert!(account.is_active());

        account.freeze();
        assert_eq!(account.status, AccountStatus::Frozen);
        assert!(!account.is_active());

        account.activate();
        assert!(account.is_active());

        account.close();
        assert_eq!(account.status, AccountStatus::Closed);
    }

    #[test]
    fn test_account_id_generation() {
        assert_eq!(Account::generate_id(1), "ACC_001");
        assert_eq!(Account::generate_id(42), "ACC_042");
        assert_eq!(Account::generate_id(999), "ACC_999");
    }
}
