//! # Business Logic
//! 
//! Module chứa các quy tắc nghiệp vụ ngân hàng:
//! - Lãi suất theo cấp số dư (tiered interest)
//! - Thuế thu nhập từ tiền lãi
//! - Phí quản lý tài khoản

pub mod interest;
pub mod tax;
pub mod fee;
pub mod process;

pub use interest::*;
pub use tax::*;
pub use fee::*;
pub use process::*;
