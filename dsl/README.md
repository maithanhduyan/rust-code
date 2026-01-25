# 🏦 Banking DSL - Ngôn ngữ đặc tả miền cho nghiệp vụ ngân hàng

DSL (Domain Specific Language) trong Rust giúp chuyên viên ngân hàng mô tả sản phẩm tiền gửi bằng cú pháp gần với ngôn ngữ tự nhiên.

## 🚀 Cài đặt và Chạy

```bash
# Build project
cargo build

# Chạy demo
cargo run

# Chạy tests
cargo test
```

## 📖 Cách sử dụng DSL

### 1. Mở tài khoản tiết kiệm
```rust
use banking_dsl::*;

let mut tk = tiet_kiem!(tiền_gửi 100.0);
```

### 2. Trừ phí quản lý hàng năm
```rust
tiet_kiem!(trừ_phí 1.0, cho tk);
```

### 3. Cộng lãi suất
```rust
tiet_kiem!(cộng_lãi 0.002, cho tk);  // Lãi suất 0.2%
```

### 4. Gửi thêm / Rút tiền
```rust
tiet_kiem!(gửi_thêm 50.0, vào tk);
tiet_kiem!(rút 30.0, từ tk);
```

### 5. Mô phỏng nhiều năm
```rust
mo_phong_nam!(3, tk, phí: 1.0, lãi: 0.002);
```

## 📋 Ví dụ nghiệp vụ

**Yêu cầu:** Tiền gửi 100 triệu, phí quản lý 1 triệu/năm, lãi suất 0.2%/năm

```rust
let mut tai_khoan = tiet_kiem!(tiền_gửi 100.0);
tiet_kiem!(trừ_phí 1.0, cho tai_khoan);
tiet_kiem!(cộng_lãi 0.002, cho tai_khoan);

println!("Số dư: {:.2}", tiet_kiem!(số_dư tai_khoan));
// Kết quả: 99.20 = (100 - 1) + (99 × 0.002)
```

## 🧩 Cấu trúc dự án

```
dsl/
├── Cargo.toml          # Cấu hình project
├── README.md           # Tài liệu này
└── src/
    ├── lib.rs          # DSL macros (tiet_kiem!, mo_phong_nam!)
    ├── account.rs      # SavingsAccount struct
    └── main.rs         # Demo program
```

## ✅ Ưu điểm DSL

- **Trực quan**: Cú pháp gần với ngôn ngữ tự nhiên
- **An toàn kiểu**: Trình biên dịch Rust kiểm tra lỗi
- **Hiệu năng cao**: Biên dịch xuống mã máy tối ưu
- **Dễ mở rộng**: Thêm quy tắc nghiệp vụ mới dễ dàng