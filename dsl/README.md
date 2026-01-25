# 🏦 Banking DSL - Rust Workspace

Hệ thống DSL (Domain Specific Language) modular cho nghiệp vụ ngân hàng, được thiết kế theo kiến trúc phân lớp.

## 📁 Kiến trúc

```
dsl/
├── Cargo.toml                 # Workspace configuration
├── crates/
│   ├── core-banking/         # 🔧 Core types, traits & abstractions
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs      # VND, Percentage, AccountType
│   │       ├── account.rs    # Account struct
│   │       ├── transaction.rs# Transaction types
│   │       └── traits.rs     # InterestCalculator, TaxCalculator, etc.
│   │
│   ├── business/             # 💼 Business logic
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── interest.rs   # Tiered interest rates
│   │       ├── tax.rs        # Tax brackets & rules
│   │       ├── fee.rs        # Fee schedules
│   │       └── process.rs    # Yearly simulation process
│   │
│   ├── dsl-macros/           # 🎯 DSL Macros
│   │   └── src/lib.rs        # tài_khoản!, lãi_suất!, thuế!, phí!, mô_phỏng!, nghiệp_vụ!
│   │
│   └── reports/              # 📊 Reporting & Export
│       └── src/
│           ├── lib.rs
│           ├── summary.rs    # Account summary report
│           ├── yearly.rs     # Yearly report
│           └── export.rs     # CSV, JSON, Markdown exporters
│
└── examples/
    ├── basic/                # Ví dụ cơ bản
    └── advanced/             # Mô hình nghiệp vụ nâng cao
```

## 🚀 Cài đặt và Chạy

```bash
# Build toàn bộ workspace
cargo build --workspace

# Chạy tests
cargo test --workspace

# Chạy ví dụ cơ bản
cargo run -p example-basic

# Chạy ví dụ nâng cao (lãi suất bậc thang, thuế)
cargo run -p example-advanced
```

## 📖 Sử dụng DSL

### 1. Tạo tài khoản
```rust
use dsl_macros::*;

let mut tk = tài_khoản!(tiết_kiệm "TK001", 5000.0);
```

### 2. Định nghĩa lãi suất bậc thang
```rust
let interest = lãi_suất! {
    tên: "Lãi suất tiết kiệm",
    cấp: [
        (0, 1000): 0.1% => "Cấp cơ bản",
        (1000, 10000): 0.2% => "Cấp trung",
        (10000, MAX): 0.15% => "Cấp cao",
    ]
};
```

### 3. Định nghĩa thuế
```rust
let tax = thuế! {
    tên: "Thuế TNCN từ lãi",
    quy_tắc: [
        lãi_dưới 100 => Miễn,
        lãi_dưới 500 => Thấp,
    ],
    mặc_định: Trung_bình
};
```

### 4. Mô phỏng nhiều năm
```rust
let results = mô_phỏng! {
    tài_khoản: tk,
    số_năm: 3,
    lãi_suất: interest,
    thuế: tax,
    phí: fee_schedule
};
```

### 5. DSL tổng hợp (all-in-one)
```rust
let (account, results) = nghiệp_vụ! {
    tài_khoản: tiết_kiệm("TK001", 10000.0),
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
```

## 📊 Báo cáo

```rust
use reports::{AccountSummary, YearlyReport, CsvExporter, ReportExporter};

// Báo cáo tổng hợp tài khoản
let summary = AccountSummary::from_account(&account);
summary.display();

// Xuất CSV
let csv = CsvExporter.export(&results);
```

## ✅ Ưu điểm kiến trúc

| Layer | Responsibility |
|-------|----------------|
| **core-banking** | Types cơ bản, không phụ thuộc business logic |
| **business** | Quy tắc nghiệp vụ có thể thay đổi theo chính sách |
| **dsl-macros** | Cú pháp thân thiện cho người dùng cuối |
| **reports** | Xuất báo cáo đa định dạng |

## 📈 Mở rộng

- Thêm quy tắc lãi suất mới: Sửa `business/src/interest.rs`
- Thêm loại thuế: Sửa `business/src/tax.rs`  
- Thêm cú pháp DSL: Sửa `dsl-macros/src/lib.rs`
- Thêm format báo cáo: Implement `ReportExporter` trait