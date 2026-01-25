//! # Person Module
//!
//! Định nghĩa PersonType và Person cho các vai trò trong hệ thống.
//! - Customer: Khách hàng với đầy đủ wallets
//! - Employee: Nhân viên với Funding wallet
//! - Shareholder: Cổ đông nhận cổ tức
//! - Manager: Quản lý phê duyệt operations
//! - Auditor: Kiểm toán viên (Big 4) read-only

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Loại người dùng trong hệ thống.
///
/// Mỗi loại có quyền hạn và wallets khác nhau:
/// - Customer: Full wallets (Spot, Funding)
/// - Employee: Funding only (lương, bảo hiểm)
/// - Shareholder: Funding only (cổ tức)
/// - Manager: Không có wallet, chỉ có permissions
/// - Auditor: Không có wallet, read-only access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonType {
    /// Khách hàng - có đầy đủ wallets, có thể trade
    Customer,
    /// Nhân viên ngân hàng - nhận lương, mua bảo hiểm
    Employee,
    /// Cổ đông - nhận cổ tức
    Shareholder,
    /// Quản lý - phê duyệt bonus, xem reports
    Manager,
    /// Kiểm toán viên (Deloitte, PwC, EY, KPMG) - read-only
    Auditor,
}

impl PersonType {
    /// Trả về code string cho DB
    pub fn as_str(&self) -> &'static str {
        match self {
            PersonType::Customer => "customer",
            PersonType::Employee => "employee",
            PersonType::Shareholder => "shareholder",
            PersonType::Manager => "manager",
            PersonType::Auditor => "auditor",
        }
    }

    /// Parse từ string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "customer" => Some(PersonType::Customer),
            "employee" => Some(PersonType::Employee),
            "shareholder" => Some(PersonType::Shareholder),
            "manager" => Some(PersonType::Manager),
            "auditor" => Some(PersonType::Auditor),
            _ => None,
        }
    }

    /// Kiểm tra PersonType có cần account/wallets không
    pub fn has_account(&self) -> bool {
        matches!(
            self,
            PersonType::Customer | PersonType::Employee | PersonType::Shareholder
        )
    }

    /// Kiểm tra có quyền phê duyệt operations không
    pub fn can_approve(&self) -> bool {
        matches!(self, PersonType::Manager)
    }

    /// Kiểm tra có quyền audit/read events không
    pub fn can_audit(&self) -> bool {
        matches!(self, PersonType::Auditor | PersonType::Manager)
    }
}

impl fmt::Display for PersonType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Thông tin người dùng trong hệ thống.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    /// ID của person (CUST_001, EMP_001, AUDIT_001, ...)
    pub id: String,
    /// Loại người dùng
    pub person_type: PersonType,
    /// Tên đầy đủ
    pub name: String,
    /// Email (optional)
    pub email: Option<String>,
    /// Thời gian tạo
    pub created_at: DateTime<Utc>,
}

impl Person {
    /// Tạo Person mới
    pub fn new(id: String, person_type: PersonType, name: String) -> Self {
        Self {
            id,
            person_type,
            name,
            email: None,
            created_at: Utc::now(),
        }
    }

    /// Tạo Person với email
    pub fn with_email(mut self, email: String) -> Self {
        self.email = Some(email);
        self
    }

    /// Tạo Customer
    pub fn customer(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Customer, name.to_string())
    }

    /// Tạo Employee
    pub fn employee(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Employee, name.to_string())
    }

    /// Tạo Shareholder
    pub fn shareholder(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Shareholder, name.to_string())
    }

    /// Tạo Manager
    pub fn manager(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Manager, name.to_string())
    }

    /// Tạo Auditor (Big 4)
    pub fn auditor(id: &str, name: &str) -> Self {
        Self::new(id.to_string(), PersonType::Auditor, name.to_string())
    }

    /// Kiểm tra có cần tạo account không
    pub fn needs_account(&self) -> bool {
        self.person_type.has_account()
    }

    /// Generate prefix cho ID dựa trên PersonType
    pub fn id_prefix(person_type: PersonType) -> &'static str {
        match person_type {
            PersonType::Customer => "CUST",
            PersonType::Employee => "EMP",
            PersonType::Shareholder => "SH",
            PersonType::Manager => "MGR",
            PersonType::Auditor => "AUDIT",
        }
    }
}

impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} - {})", self.name, self.id, self.person_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person_type_str() {
        assert_eq!(PersonType::Customer.as_str(), "customer");
        assert_eq!(PersonType::Auditor.as_str(), "auditor");
        assert_eq!(PersonType::from_str("CUSTOMER"), Some(PersonType::Customer));
        assert_eq!(PersonType::from_str("unknown"), None);
    }

    #[test]
    fn test_person_type_permissions() {
        assert!(PersonType::Customer.has_account());
        assert!(PersonType::Employee.has_account());
        assert!(PersonType::Shareholder.has_account());
        assert!(!PersonType::Manager.has_account());
        assert!(!PersonType::Auditor.has_account());

        assert!(PersonType::Manager.can_approve());
        assert!(!PersonType::Customer.can_approve());

        assert!(PersonType::Auditor.can_audit());
        assert!(PersonType::Manager.can_audit());
        assert!(!PersonType::Customer.can_audit());
    }

    #[test]
    fn test_person_creation() {
        let alice = Person::customer("CUST_001", "Alice");
        assert_eq!(alice.id, "CUST_001");
        assert_eq!(alice.person_type, PersonType::Customer);
        assert!(alice.needs_account());

        let deloitte = Person::auditor("AUDIT_001", "Deloitte");
        assert_eq!(deloitte.person_type, PersonType::Auditor);
        assert!(!deloitte.needs_account());
    }

    #[test]
    fn test_person_with_email() {
        let bob = Person::employee("EMP_001", "Bob").with_email("bob@simbank.com".to_string());

        assert_eq!(bob.email, Some("bob@simbank.com".to_string()));
    }

    #[test]
    fn test_person_display() {
        let person = Person::customer("CUST_001", "Alice");
        assert_eq!(format!("{}", person), "Alice (CUST_001 - customer)");
    }
}
