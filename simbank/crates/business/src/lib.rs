//! # Simbank Business
//!
//! Business logic layer - Customer, Employee, Auditor operations.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Business Layer                           │
//! │  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐  │
//! │  │ Customer  │ │ Employee  │ │Shareholder│ │  Auditor  │  │
//! │  │  Service  │ │  Service  │ │  Service  │ │  Service  │  │
//! │  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘  │
//! │        │             │             │             │         │
//! │        └─────────────┴─────────────┴─────────────┘         │
//! │                          │                                  │
//! │                  ServiceContext                             │
//! │                    (Pool + Events)                          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use simbank_business::{ServiceContext, CustomerService};
//!
//! let ctx = ServiceContext::new(&db);
//! let customer_svc = CustomerService::new(&ctx);
//!
//! // Deposit 100 USDT to customer's Funding wallet
//! let result = customer_svc.deposit("CUST_001", "ACC_001", dec!(100), "USDT").await?;
//! ```

pub mod auditor;
pub mod customer;
pub mod employee;
pub mod error;
pub mod management;
pub mod services;
pub mod shareholder;

// Re-export commonly used types
pub use auditor::{AmlThresholds, AuditorService};
pub use customer::CustomerService;
pub use employee::EmployeeService;
pub use error::{BusinessError, BusinessResult};
pub use management::ManagementService;
pub use services::{AccountCreationResult, ServiceContext, TransactionResult};
pub use shareholder::ShareholderService;
