//! # Event Module
//!
//! Định nghĩa Event, EventType, và EventMetadata cho Event Sourcing.
//! Events được ghi vào JSONL files để phục vụ AML compliance và audit.

use crate::person::PersonType;
use crate::wallet::WalletType;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Loại sự kiện trong hệ thống.
///
/// Mỗi event type đại diện cho một hành động đã xảy ra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // === Account events ===
    /// Tạo account mới
    AccountCreated,
    /// Đóng băng account
    AccountFrozen,
    /// Mở khóa account
    AccountActivated,
    /// Đóng account
    AccountClosed,

    // === Transaction events ===
    /// Nạp tiền vào (external -> Funding)
    Deposit,
    /// Rút tiền ra (Funding -> external)
    Withdrawal,
    /// Chuyển tiền nội bộ giữa các wallets
    InternalTransfer,
    /// Giao dịch trade (Spot)
    Trade,

    // === Business events ===
    /// Thu phí (annual fee, transaction fee)
    Fee,
    /// Trả lương cho employee
    SalaryPayment,
    /// Mua bảo hiểm
    InsurancePurchase,
    /// Chi trả cổ tức
    DividendPayment,
    /// Thưởng (bonus)
    BonusPayment,

    // === Audit events ===
    /// Kiểm toán viên truy cập dữ liệu
    AuditAccess,
    /// Tạo báo cáo audit
    AuditReportGenerated,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::AccountCreated => "account_created",
            EventType::AccountFrozen => "account_frozen",
            EventType::AccountActivated => "account_activated",
            EventType::AccountClosed => "account_closed",
            EventType::Deposit => "deposit",
            EventType::Withdrawal => "withdrawal",
            EventType::InternalTransfer => "internal_transfer",
            EventType::Trade => "trade",
            EventType::Fee => "fee",
            EventType::SalaryPayment => "salary_payment",
            EventType::InsurancePurchase => "insurance_purchase",
            EventType::DividendPayment => "dividend_payment",
            EventType::BonusPayment => "bonus_payment",
            EventType::AuditAccess => "audit_access",
            EventType::AuditReportGenerated => "audit_report_generated",
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// AML (Anti-Money Laundering) flags.
///
/// Các flag được gắn vào event để đánh dấu giao dịch đáng ngờ.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmlFlag {
    /// Giao dịch lớn (> threshold)
    LargeAmount,
    /// Gần ngưỡng báo cáo (có thể là smurfing)
    NearThreshold,
    /// Pattern bất thường (nhiều giao dịch nhỏ liên tiếp)
    UnusualPattern,
    /// Giao dịch xuyên biên giới
    CrossBorder,
    /// Từ/đến quốc gia rủi ro cao
    HighRiskCountry,
    /// Tài khoản mới với giao dịch lớn
    NewAccountLargeTx,
    /// Rút tiền nhanh sau khi nạp
    RapidWithdrawal,
}

impl AmlFlag {
    pub fn as_str(&self) -> &'static str {
        match self {
            AmlFlag::LargeAmount => "large_amount",
            AmlFlag::NearThreshold => "near_threshold",
            AmlFlag::UnusualPattern => "unusual_pattern",
            AmlFlag::CrossBorder => "cross_border",
            AmlFlag::HighRiskCountry => "high_risk_country",
            AmlFlag::NewAccountLargeTx => "new_account_large_tx",
            AmlFlag::RapidWithdrawal => "rapid_withdrawal",
        }
    }
}

/// Metadata bổ sung cho event, phục vụ truy vết và AML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventMetadata {
    /// IP address của người thực hiện
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    /// Mã quốc gia ISO (VN, US, ...)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// ID thiết bị
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// User agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Dữ liệu tùy chỉnh (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

impl EventMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ip(mut self, ip: &str) -> Self {
        self.ip_address = Some(ip.to_string());
        self
    }

    pub fn with_location(mut self, location: &str) -> Self {
        self.location = Some(location.to_string());
        self
    }

    pub fn with_device(mut self, device_id: &str) -> Self {
        self.device_id = Some(device_id.to_string());
        self
    }
}

/// Event chính - đại diện cho một sự kiện đã xảy ra trong hệ thống.
///
/// Events là immutable, append-only, và được lưu vào JSONL files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// ID unique của event (EVT_001, EVT_002, ...)
    pub event_id: String,
    /// Thời điểm xảy ra
    pub timestamp: DateTime<Utc>,
    /// Loại event
    pub event_type: EventType,

    // === Actor ===
    /// ID của người thực hiện (CUST_001, EMP_001, ...)
    pub actor_id: String,
    /// Loại actor
    pub actor_role: PersonType,

    // === Target ===
    /// ID của account liên quan
    pub account_id: String,
    /// Wallet nguồn (None nếu deposit từ external)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_wallet: Option<WalletType>,
    /// Wallet đích (None nếu withdrawal ra external)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_wallet: Option<WalletType>,

    // === Amount ===
    /// Số tiền (dạng string để đảm bảo precision)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<Decimal>,
    /// Mã tiền tệ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    // === Description ===
    /// Mô tả giao dịch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // === AML ===
    /// Các flag AML
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aml_flags: Vec<AmlFlag>,

    // === Metadata ===
    /// Thông tin bổ sung
    #[serde(default)]
    pub metadata: EventMetadata,
}

impl Event {
    /// Tạo Event mới với thông tin cơ bản
    pub fn new(
        event_id: String,
        event_type: EventType,
        actor_id: String,
        actor_role: PersonType,
        account_id: String,
    ) -> Self {
        Self {
            event_id,
            timestamp: Utc::now(),
            event_type,
            actor_id,
            actor_role,
            account_id,
            from_wallet: None,
            to_wallet: None,
            amount: None,
            currency: None,
            description: None,
            aml_flags: Vec::new(),
            metadata: EventMetadata::default(),
        }
    }

    // === Builder methods ===

    pub fn with_from_wallet(mut self, wallet: WalletType) -> Self {
        self.from_wallet = Some(wallet);
        self
    }

    pub fn with_to_wallet(mut self, wallet: WalletType) -> Self {
        self.to_wallet = Some(wallet);
        self
    }

    pub fn with_amount(mut self, amount: Decimal, currency: &str) -> Self {
        self.amount = Some(amount);
        self.currency = Some(currency.to_string());
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_aml_flag(mut self, flag: AmlFlag) -> Self {
        self.aml_flags.push(flag);
        self
    }

    pub fn with_aml_flags(mut self, flags: Vec<AmlFlag>) -> Self {
        self.aml_flags = flags;
        self
    }

    pub fn with_metadata(mut self, metadata: EventMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    // === Factory methods ===

    /// Tạo Deposit event
    pub fn deposit(
        event_id: &str,
        actor_id: &str,
        account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> Self {
        Self::new(
            event_id.to_string(),
            EventType::Deposit,
            actor_id.to_string(),
            PersonType::Customer,
            account_id.to_string(),
        )
        .with_to_wallet(WalletType::Funding)
        .with_amount(amount, currency)
    }

    /// Tạo Withdrawal event
    pub fn withdrawal(
        event_id: &str,
        actor_id: &str,
        account_id: &str,
        amount: Decimal,
        currency: &str,
    ) -> Self {
        Self::new(
            event_id.to_string(),
            EventType::Withdrawal,
            actor_id.to_string(),
            PersonType::Customer,
            account_id.to_string(),
        )
        .with_from_wallet(WalletType::Funding)
        .with_amount(amount, currency)
    }

    /// Tạo InternalTransfer event
    pub fn internal_transfer(
        event_id: &str,
        actor_id: &str,
        account_id: &str,
        from: WalletType,
        to: WalletType,
        amount: Decimal,
        currency: &str,
    ) -> Self {
        Self::new(
            event_id.to_string(),
            EventType::InternalTransfer,
            actor_id.to_string(),
            PersonType::Customer,
            account_id.to_string(),
        )
        .with_from_wallet(from)
        .with_to_wallet(to)
        .with_amount(amount, currency)
    }

    /// Generate ID cho event mới
    pub fn generate_id(counter: u64) -> String {
        format!("EVT_{:06}", counter)
    }

    /// Kiểm tra event có AML flags không
    pub fn has_aml_flags(&self) -> bool {
        !self.aml_flags.is_empty()
    }

    /// Serialize event thành JSON string (cho JSONL)
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} by {} on {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.event_type,
            self.actor_id,
            self.account_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_event_deposit() {
        let event = Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(1000), "USDT");

        assert_eq!(event.event_id, "EVT_001");
        assert_eq!(event.event_type, EventType::Deposit);
        assert_eq!(event.to_wallet, Some(WalletType::Funding));
        assert_eq!(event.amount, Some(dec!(1000)));
        assert_eq!(event.currency, Some("USDT".to_string()));
    }

    #[test]
    fn test_event_internal_transfer() {
        let event = Event::internal_transfer(
            "EVT_002",
            "CUST_001",
            "ACC_001",
            WalletType::Funding,
            WalletType::Spot,
            dec!(500),
            "USDT",
        );

        assert_eq!(event.event_type, EventType::InternalTransfer);
        assert_eq!(event.from_wallet, Some(WalletType::Funding));
        assert_eq!(event.to_wallet, Some(WalletType::Spot));
    }

    #[test]
    fn test_event_with_aml_flags() {
        let event = Event::deposit("EVT_003", "CUST_001", "ACC_001", dec!(15000), "USD")
            .with_aml_flag(AmlFlag::LargeAmount)
            .with_description("Large deposit");

        assert!(event.has_aml_flags());
        assert_eq!(event.aml_flags.len(), 1);
        assert_eq!(event.aml_flags[0], AmlFlag::LargeAmount);
    }

    #[test]
    fn test_event_with_metadata() {
        let metadata = EventMetadata::new()
            .with_ip("192.168.1.1")
            .with_location("VN")
            .with_device("mobile_ios");

        let event = Event::deposit("EVT_004", "CUST_001", "ACC_001", dec!(100), "USDT")
            .with_metadata(metadata);

        assert_eq!(event.metadata.ip_address, Some("192.168.1.1".to_string()));
        assert_eq!(event.metadata.location, Some("VN".to_string()));
    }

    #[test]
    fn test_event_to_json() {
        let event = Event::deposit("EVT_005", "CUST_001", "ACC_001", dec!(100), "USDT");
        let json = event.to_json().unwrap();

        assert!(json.contains("EVT_005"));
        assert!(json.contains("deposit"));
        assert!(json.contains("USDT"));
    }

    #[test]
    fn test_event_id_generation() {
        assert_eq!(Event::generate_id(1), "EVT_000001");
        assert_eq!(Event::generate_id(999999), "EVT_999999");
    }
}
