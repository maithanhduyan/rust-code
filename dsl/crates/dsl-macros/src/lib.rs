//! # DSL Macros
//! 
//! Module chứa các macro DSL cho nghiệp vụ ngân hàng.
//! Cung cấp cú pháp thân thiện gần với ngôn ngữ tự nhiên.

// Re-export dependencies để người dùng không cần import riêng
pub use core_banking;
pub use business;

pub use core_banking::{Account, VND, Percentage, AccountType};
pub use business::{
    TieredInterestTable, InterestTier,
    TaxTable, TaxRule, TaxBracket,
    FeeSchedule, FeeRule, FeeType,
    YearlyProcess, ProcessBuilder,
};

/// Macro tạo tài khoản tiết kiệm
/// 
/// # Cú pháp
/// - `tài_khoản!(tiết_kiệm "ID", số_dư)` - Tạo tài khoản tiết kiệm
/// - `tài_khoản!(thanh_toán "ID", số_dư)` - Tạo tài khoản thanh toán
#[macro_export]
macro_rules! tài_khoản {
    (tiết_kiệm $id:expr, $balance:expr) => {
        $crate::Account::savings($id, $balance)
    };
    (thanh_toán $id:expr, $balance:expr) => {
        $crate::Account::checking($id, $balance)
    };
}

/// Macro định nghĩa bảng lãi suất bậc thang
/// 
/// # Cú pháp
/// ```ignore
/// lãi_suất! {
///     tên: "Bảng lãi suất",
///     cấp: [
///         (0, 1000): 0.1% => "Cấp cơ bản",
///         (1000, 10000): 0.2% => "Cấp trung",
///         (10000, MAX): 0.15% => "Cấp cao",
///     ]
/// }
/// ```
#[macro_export]
macro_rules! lãi_suất {
    {
        tên: $name:expr,
        cấp: [
            $(
                ($min:expr, $max:tt): $rate:tt% => $desc:expr
            ),* $(,)?
        ]
    } => {{
        let mut table = $crate::TieredInterestTable::new($name);
        $(
            table = table.tier(
                $min as f64,
                $crate::__parse_max!($max),
                $rate,
                $desc
            );
        )*
        table
    }};
}

/// Helper macro để parse max value
#[macro_export]
#[doc(hidden)]
macro_rules! __parse_max {
    (MAX) => { None };
    ($val:expr) => { Some($val as f64) };
}

/// Macro định nghĩa bảng thuế
/// 
/// # Cú pháp
/// ```ignore
/// thuế! {
///     tên: "Bảng thuế",
///     quy_tắc: [
///         lãi_dưới 100 => Miễn,
///         lãi_dưới 500 => Thấp,
///     ],
///     mặc_định: Trung_bình
/// }
/// ```
#[macro_export]
macro_rules! thuế {
    {
        tên: $name:expr,
        quy_tắc: [
            $(lãi_dưới $threshold:expr => $bracket:ident),* $(,)?
        ],
        mặc_định: $default:ident
    } => {{
        let mut table = $crate::TaxTable::new($name);
        $(
            table = table.rule(
                $threshold as f64,
                $crate::__tax_bracket!($bracket),
                format!("Lãi < {} VND", $threshold)
            );
        )*
        table.default($crate::__tax_bracket!($default))
    }};
}

/// Helper macro để chuyển đổi tên thuế tiếng Việt sang enum
#[macro_export]
#[doc(hidden)]
macro_rules! __tax_bracket {
    (Miễn) => { $crate::TaxBracket::Exempt };
    (Thấp) => { $crate::TaxBracket::Low };
    (Trung_bình) => { $crate::TaxBracket::Medium };
    (Cao) => { $crate::TaxBracket::High };
}

/// Macro định nghĩa bảng phí
/// 
/// # Cú pháp
/// ```ignore
/// phí! {
///     tên: "Bảng phí",
///     tiết_kiệm: 1.0,
///     thanh_toán: 2.0,
///     vip: 0.0
/// }
/// ```
#[macro_export]
macro_rules! phí {
    {
        tên: $name:expr
        $(, tiết_kiệm: $savings:expr)?
        $(, thanh_toán: $checking:expr)?
        $(, vip: $premium:expr)?
    } => {{
        let mut schedule = $crate::FeeSchedule::new($name);
        $(
            schedule = schedule.for_account_type(
                $crate::AccountType::Savings,
                $crate::FeeRule::fixed(
                    $crate::FeeType::AnnualMaintenance,
                    $savings,
                    "Phí quản lý tiết kiệm"
                )
            );
        )?
        $(
            schedule = schedule.for_account_type(
                $crate::AccountType::Checking,
                $crate::FeeRule::fixed(
                    $crate::FeeType::AnnualMaintenance,
                    $checking,
                    "Phí quản lý thanh toán"
                )
            );
        )?
        $(
            schedule = schedule.for_account_type(
                $crate::AccountType::Premium,
                $crate::FeeRule::fixed(
                    $crate::FeeType::AnnualMaintenance,
                    $premium,
                    "Phí VIP"
                )
            );
        )?
        schedule
    }};
}

/// Macro mô phỏng năm tài chính
/// 
/// # Cú pháp
/// ```ignore
/// mô_phỏng! {
///     tài_khoản: tk,
///     số_năm: 3,
///     lãi_suất: interest_table,
///     thuế: tax_table,
///     phí: fee_schedule
/// }
/// ```
#[macro_export]
macro_rules! mô_phỏng {
    {
        tài_khoản: $account:ident,
        số_năm: $years:expr,
        lãi_suất: $interest:expr,
        thuế: $tax:expr,
        phí: $fee:expr
    } => {{
        let process = $crate::YearlyProcess::new($interest, $tax, $fee);
        process.simulate_years(&mut $account, $years)
    }};
    
    // Phiên bản đơn giản với cấu hình mặc định
    {
        tài_khoản: $account:ident,
        số_năm: $years:expr
    } => {{
        let process = $crate::ProcessBuilder::new().build();
        process.simulate_years(&mut $account, $years)
    }};
}

/// Macro tạo quy trình nghiệp vụ hoàn chỉnh
/// 
/// # Cú pháp
/// ```ignore
/// nghiệp_vụ! {
///     // Định nghĩa tài khoản
///     let tk = tiết_kiệm("TK001", 5000.0);
///     
///     // Định nghĩa quy tắc
///     lãi_suất: {
///         (0 -> 1000): 0.1%,
///         (1000 -> 10000): 0.2%,
///         (từ 10000): 0.15%
///     },
///     thuế: {
///         lãi_dưới 100 => Miễn,
///         lãi_dưới 500 => Thấp,
///         mặc_định => Trung_bình
///     },
///     phí: 1.0,
///     
///     // Thực thi
///     mô_phỏng: 3
/// }
/// ```
#[macro_export]
macro_rules! nghiệp_vụ {
    {
        tài_khoản: $account_type:ident($id:expr, $balance:expr),
        lãi_suất: {
            $(($min:expr, $max:tt): $rate:tt%),* $(,)?
        },
        thuế: {
            $(lãi_dưới $threshold:expr => $bracket:ident),* $(,)?
            mặc_định => $default:ident
        },
        phí: $fee:expr,
        mô_phỏng: $years:expr
    } => {{
        println!("╔═══════════════════════════════════════════════════════════╗");
        println!("║        🏦 MÔ PHỎNG NGHIỆP VỤ NGÂN HÀNG 🏦                 ║");
        println!("╚═══════════════════════════════════════════════════════════╝\n");
        
        // Tạo tài khoản
        let mut account = $crate::tài_khoản!($account_type $id, $balance);
        
        // Tạo bảng lãi suất
        let interest_table = $crate::lãi_suất! {
            tên: "Lãi suất bậc thang",
            cấp: [
                $(($min, $max): $rate% => concat!("Cấp ", stringify!($min))),*
            ]
        };
        
        // Tạo bảng thuế
        let tax_table = $crate::thuế! {
            tên: "Thuế thu nhập từ lãi",
            quy_tắc: [
                $(lãi_dưới $threshold => $bracket),*
            ],
            mặc_định: $default
        };
        
        // Tạo bảng phí
        let fee_schedule = $crate::phí! {
            tên: "Phí quản lý",
            tiết_kiệm: $fee
        };
        
        // Thực thi mô phỏng
        let results = $crate::mô_phỏng! {
            tài_khoản: account,
            số_năm: $years,
            lãi_suất: interest_table,
            thuế: tax_table,
            phí: fee_schedule
        };
        
        println!("\n╔═══════════════════════════════════════════════════════════╗");
        println!("║                   🎉 HOÀN TẤT 🎉                           ║");
        println!("╚═══════════════════════════════════════════════════════════╝");
        
        (account, results)
    }};
}
