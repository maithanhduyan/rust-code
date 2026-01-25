//! # Banking DSL - Ngôn ngữ đặc tả miền cho nghiệp vụ ngân hàng
//! 
//! DSL này cho phép chuyên viên ngân hàng mô tả sản phẩm tiền gửi
//! bằng cú pháp gần với ngôn ngữ tự nhiên.
//! 
//! ## Ví dụ sử dụng
//! 
//! ```rust
//! use banking_dsl::*;
//! 
//! // Mở tài khoản với 100 triệu
//! let mut tk = tiet_kiem!(tiền_gửi 100.0);
//! 
//! // Trừ phí quản lý 1 triệu
//! tiet_kiem!(trừ_phí 1.0, cho tk);
//! 
//! // Cộng lãi 0.2%
//! tiet_kiem!(cộng_lãi 0.002, cho tk);
//! ```

mod account;

pub use account::SavingsAccount;

/// Macro DSL chính cho nghiệp vụ tiết kiệm
/// 
/// # Cú pháp hỗ trợ
/// 
/// - `tiet_kiem!(tiền_gửi <số tiền>)` - Mở tài khoản mới
/// - `tiet_kiem!(trừ_phí <phí>, cho <tài khoản>)` - Trừ phí quản lý
/// - `tiet_kiem!(cộng_lãi <tỷ lệ>, cho <tài khoản>)` - Cộng lãi suất
/// - `tiet_kiem!(gửi_thêm <số tiền>, vào <tài khoản>)` - Gửi thêm tiền
/// - `tiet_kiem!(rút <số tiền>, từ <tài khoản>)` - Rút tiền
/// - `tiet_kiem!(số_dư <tài khoản>)` - Xem số dư
/// - `tiet_kiem!(hiển_thị <tài khoản>)` - Hiển thị thông tin
#[macro_export]
macro_rules! tiet_kiem {
    // ═══════════════════════════════════════════════════════════════
    // 1. Khởi tạo tài khoản: tiền_gửi <số tiền>
    // ═══════════════════════════════════════════════════════════════
    (tiền_gửi $amount:expr) => {
        $crate::SavingsAccount::new($amount)
    };

    // ═══════════════════════════════════════════════════════════════
    // 2. Trừ phí quản lý: trừ_phí <số tiền>, cho <tài khoản>
    // ═══════════════════════════════════════════════════════════════
    (trừ_phí $fee:expr, cho $account:ident) => {
        $account.subtract_fee($fee)
    };

    // ═══════════════════════════════════════════════════════════════
    // 3. Cộng lãi suất: cộng_lãi <tỷ lệ>, cho <tài khoản>
    // ═══════════════════════════════════════════════════════════════
    (cộng_lãi $rate:expr, cho $account:ident) => {
        $account.add_interest($rate)
    };

    // ═══════════════════════════════════════════════════════════════
    // 4. Gửi thêm tiền: gửi_thêm <số tiền>, vào <tài khoản>
    // ═══════════════════════════════════════════════════════════════
    (gửi_thêm $amount:expr, vào $account:ident) => {
        $account.deposit($amount)
    };

    // ═══════════════════════════════════════════════════════════════
    // 5. Rút tiền: rút <số tiền>, từ <tài khoản>
    // ═══════════════════════════════════════════════════════════════
    (rút $amount:expr, từ $account:ident) => {
        $account.withdraw($amount)
    };

    // ═══════════════════════════════════════════════════════════════
    // 6. Xem số dư: số_dư <tài khoản>
    // ═══════════════════════════════════════════════════════════════
    (số_dư $account:ident) => {
        $account.get_balance()
    };

    // ═══════════════════════════════════════════════════════════════
    // 7. Hiển thị thông tin: hiển_thị <tài khoản>
    // ═══════════════════════════════════════════════════════════════
    (hiển_thị $account:ident) => {
        $account.display()
    };
}

/// Macro mô phỏng nhiều năm
/// 
/// Mô phỏng diễn biến tài khoản qua nhiều năm với phí và lãi suất cố định.
/// 
/// Cú pháp: `mo_phong_nam!(số_năm, tài_khoản, phí: phí_năm, lãi: lãi_suất)`
#[macro_export]
macro_rules! mo_phong_nam {
    ($so_nam:expr, $account:ident, phí: $fee:expr, lãi: $rate:expr) => {
        println!("\n🔄 Bắt đầu mô phỏng {} năm...", $so_nam);
        println!("   - Phí quản lý: {:.2}/năm", $fee);
        println!("   - Lãi suất: {:.2}%/năm", $rate * 100.0);
        println!("───────────────────────────────────");
        
        for nam in 1..=$so_nam {
            println!("\n📅 Năm {}:", nam);
            $crate::tiet_kiem!(trừ_phí $fee, cho $account);
            $crate::tiet_kiem!(cộng_lãi $rate, cho $account);
        }
        
        println!("\n───────────────────────────────────");
        println!("✨ Kết thúc mô phỏng {} năm", $so_nam);
    };
    
    // Phiên bản đơn giản với giá trị mặc định
    ($so_nam:expr, $account:ident) => {
        $crate::mo_phong_nam!($so_nam, $account, phí: 1.0, lãi: 0.002);
    };
}

/// Macro tính lãi suất theo cấp số dư (tiered interest)
/// 
/// Áp dụng lãi suất khác nhau tùy theo mức số dư của tài khoản.
#[macro_export]
macro_rules! lai_suat_bac_thang {
    ($account:ident, [$(($min:expr, $max:expr) => $rate:expr),+ $(,)?]) => {{
        let balance = $account.get_balance();
        let rate = {
            $(
                if balance >= $min && (stringify!($max) == "_" || balance < $max) {
                    $rate
                } else
            )+
            { 0.0 } // Default rate if no tier matches
        };
        println!("📈 Áp dụng lãi suất bậc thang: {:.2}% cho số dư {:.2}", rate * 100.0, balance);
        $account.add_interest(rate);
    }};
}
