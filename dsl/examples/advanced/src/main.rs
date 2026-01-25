//! # Ví dụ nâng cao - Mô hình nghiệp vụ phức tạp
//! 
//! Triển khai DSL theo yêu cầu từ DSL_COMPLICATE.md:
//! - Lãi suất theo cấp số dư
//! - Thuế thu nhập từ tiền lãi
//! - Báo cáo tổng hợp

use dsl_macros::*;
use reports::{AccountSummary, YearlyReport, CsvExporter, JsonExporter, MarkdownExporter, ReportExporter};

fn main() {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║       🏦 MÔ HÌNH NGHIỆP VỤ NÂNG CAO - BANKING DSL 🏦      ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 1: Tài khoản 5,000 VND với lãi suất bậc thang
    // ═══════════════════════════════════════════════════════════════════
    example_1_tiered_interest();

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 2: Tài khoản 25,000 VND - VIP
    // ═══════════════════════════════════════════════════════════════════
    example_2_vip_account();

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 3: Sử dụng DSL macro tổng hợp
    // ═══════════════════════════════════════════════════════════════════
    example_3_full_dsl();

    // ═══════════════════════════════════════════════════════════════════
    // VÍ DỤ 4: Xuất báo cáo
    // ═══════════════════════════════════════════════════════════════════
    example_4_reports();

    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║              🎉 HOÀN TẤT MÔ PHỎNG NÂNG CAO 🎉             ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
}

fn example_1_tiered_interest() {
    println!("\n🎯 VÍ DỤ 1: Tài khoản 5,000 VND - Lãi suất bậc thang");
    println!("═══════════════════════════════════════════════════════════════\n");
    
    println!("📋 QUY TẮC NGHIỆP VỤ:");
    println!("   Lãi suất theo cấp số dư:");
    println!("     - Dưới 1,000 VND: 0.1%/năm");
    println!("     - 1,000 - 10,000 VND: 0.2%/năm");
    println!("     - Trên 10,000 VND: 0.15%/năm");
    println!("   Thuế thu nhập từ lãi:");
    println!("     - Lãi < 100: Miễn thuế");
    println!("     - Lãi < 500: 5%");
    println!("     - Lãi >= 500: 10%");
    println!();

    // Tạo tài khoản
    let mut tk = tài_khoản!(tiết_kiệm "TK-5000", 5000.0);

    // Định nghĩa bảng lãi suất bậc thang bằng DSL
    let interest_table = lãi_suất! {
        tên: "Lãi suất tiết kiệm bậc thang",
        cấp: [
            (0, 1000): 0.1% => "Cấp cơ bản",
            (1000, 10000): 0.2% => "Cấp trung bình",
            (10000, MAX): 0.15% => "Cấp cao cấp",
        ]
    };

    // Định nghĩa bảng thuế bằng DSL
    let tax_table = thuế! {
        tên: "Thuế thu nhập cá nhân từ lãi",
        quy_tắc: [
            lãi_dưới 100 => Miễn,
            lãi_dưới 500 => Thấp,
        ],
        mặc_định: Trung_bình
    };

    // Định nghĩa bảng phí
    let fee_schedule = phí! {
        tên: "Phí quản lý tiêu chuẩn",
        tiết_kiệm: 1.0
    };

    // Mô phỏng 3 năm
    let results = mô_phỏng! {
        tài_khoản: tk,
        số_năm: 3,
        lãi_suất: interest_table,
        thuế: tax_table,
        phí: fee_schedule
    };

    // Hiển thị báo cáo
    let summary = AccountSummary::from_account(&tk);
    summary.display();

    let yearly_report = YearlyReport::from_results(results);
    yearly_report.display();
}

fn example_2_vip_account() {
    println!("\n\n🎯 VÍ DỤ 2: Tài khoản VIP 25,000 VND");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut tk_vip = tài_khoản!(tiết_kiệm "TK-VIP-25000", 25000.0);

    // Bảng lãi suất VIP (cao hơn)
    let vip_interest = lãi_suất! {
        tên: "Lãi suất VIP",
        cấp: [
            (0, 5000): 0.15% => "VIP cơ bản",
            (5000, 20000): 0.25% => "VIP trung",
            (20000, MAX): 0.30% => "VIP cao cấp",
        ]
    };

    // Thuế giống nhau
    let tax_table = thuế! {
        tên: "Thuế TNCN",
        quy_tắc: [
            lãi_dưới 100 => Miễn,
            lãi_dưới 500 => Thấp,
        ],
        mặc_định: Trung_bình
    };

    // VIP miễn phí
    let vip_fee = phí! {
        tên: "Phí VIP",
        tiết_kiệm: 0.0
    };

    let results = mô_phỏng! {
        tài_khoản: tk_vip,
        số_năm: 5,
        lãi_suất: vip_interest,
        thuế: tax_table,
        phí: vip_fee
    };

    let yearly_report = YearlyReport::from_results(results);
    yearly_report.display();
}

fn example_3_full_dsl() {
    println!("\n\n🎯 VÍ DỤ 3: DSL Macro tổng hợp");
    println!("═══════════════════════════════════════════════════════════════\n");
    
    println!("Sử dụng macro nghiệp_vụ! để định nghĩa toàn bộ logic trong một block:\n");

    // Sử dụng macro nghiệp_vụ! - cú pháp gần với ngôn ngữ tự nhiên nhất
    let (account, results) = nghiệp_vụ! {
        tài_khoản: tiết_kiệm("TK-FULL-DSL", 10000.0),
        lãi_suất: {
            (0, 1000): 0.1%,
            (1000, 10000): 0.2%,
            (10000, MAX): 0.15%
        },
        thuế: {
            lãi_dưới 100 => Miễn,
            lãi_dưới 500 => Thấp,
            mặc_định => Trung_bình
        },
        phí: 1.0,
        mô_phỏng: 3
    };

    let summary = AccountSummary::from_account(&account);
    summary.display();

    let yearly_report = YearlyReport::from_results(results);
    yearly_report.display();
}

fn example_4_reports() {
    println!("\n\n🎯 VÍ DỤ 4: Xuất báo cáo đa định dạng");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut tk = tài_khoản!(tiết_kiệm "TK-REPORT", 8000.0);

    let process = ProcessBuilder::new().build();
    let results = process.simulate_years(&mut tk, 3);

    // Xuất CSV
    println!("📄 XUẤT CSV:");
    println!("─────────────────────────────────────────────────────────────");
    let csv = CsvExporter.export(&results);
    println!("{}", csv);

    // Xuất JSON
    println!("📄 XUẤT JSON:");
    println!("─────────────────────────────────────────────────────────────");
    let json = JsonExporter.export(&results);
    println!("{}", json);

    // Xuất Markdown
    println!("\n📄 XUẤT MARKDOWN:");
    println!("─────────────────────────────────────────────────────────────");
    let md = MarkdownExporter.export(&results);
    println!("{}", md);
}
