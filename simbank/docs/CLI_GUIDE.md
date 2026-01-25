# Hướng dẫn sử dụng CLI 🖥️

> Tài liệu hướng dẫn sử dụng giao diện dòng lệnh (CLI) của SIMBANK

---

## 1. Giới thiệu

SIMBANK CLI là công cụ dòng lệnh để quản lý và thao tác với hệ thống ngân hàng.

### Cài đặt

```powershell
# Build CLI
cd simbank
cargo build --release -p simbank-cli

# Hoặc chạy trực tiếp
cargo run -p simbank-cli -- <command>
```

### Xem trợ giúp

```powershell
# Trợ giúp chung
simbank --help

# Trợ giúp cho command cụ thể
simbank account --help
simbank deposit --help
```

---

## 2. Các lệnh cơ bản

### 2.1 init - Khởi tạo hệ thống

Tạo database SQLite và thư mục events.

```powershell
# Khởi tạo với đường dẫn mặc định
simbank init

# Khởi tạo với đường dẫn tùy chỉnh
simbank init --data-dir ./my-data

# Force khởi tạo lại (xóa data cũ)
simbank init --force
```

**Kết quả:**
```
✅ Database initialized at data/simbank.db
✅ Events directory created at data/events/
✅ Migration completed: 20260125_init.sql
```

### 2.2 status - Xem trạng thái

Hiển thị thông tin tổng quan về hệ thống.

```powershell
simbank status
```

**Kết quả:**
```
╔══════════════════════════════════════════╗
║           SIMBANK STATUS                 ║
╚══════════════════════════════════════════╝

📊 Database: data/simbank.db (125 KB)
📁 Events:   data/events/ (3 files, 45 KB)

📈 Statistics:
   Persons:      15
   Accounts:     12
   Wallets:      24
   Transactions: 156
   Events:       203

🕐 Last activity: 2025-01-25 10:30:45 UTC
```

---

## 3. Quản lý tài khoản

### 3.1 account create - Tạo tài khoản

```powershell
# Tạo tài khoản khách hàng
simbank account create --name "Nguyễn Văn A" --type customer

# Tạo tài khoản nhân viên
simbank account create --name "Trần Thị B" --type employee

# Tạo với email
simbank account create --name "Lê Văn C" --type customer --email "levanc@email.com"

# Các loại tài khoản
# --type: customer, employee, shareholder, manager, auditor
```

**Kết quả:**
```
✅ Account created successfully!
   Person ID:  PERS_a1b2c3d4
   Account ID: ACC_e5f6g7h8
   Name:       Nguyễn Văn A
   Type:       Customer
   Wallets:    Funding, Spot
```

### 3.2 account list - Liệt kê tài khoản

```powershell
# Liệt kê tất cả
simbank account list

# Lọc theo loại
simbank account list --type customer
simbank account list --type employee

# Lọc theo trạng thái
simbank account list --status active
simbank account list --status suspended

# Giới hạn số lượng
simbank account list --limit 10
```

**Kết quả:**
```
┌─────────────────┬──────────────────────┬──────────┬─────────┐
│ ACCOUNT ID      │ NAME                 │ TYPE     │ STATUS  │
├─────────────────┼──────────────────────┼──────────┼─────────┤
│ ACC_e5f6g7h8    │ Nguyễn Văn A         │ Customer │ Active  │
│ ACC_i9j0k1l2    │ Trần Thị B           │ Employee │ Active  │
│ ACC_m3n4o5p6    │ Lê Văn C             │ Customer │ Active  │
└─────────────────┴──────────────────────┴──────────┴─────────┘
Total: 3 accounts
```

### 3.3 account show - Xem chi tiết

```powershell
simbank account show ACC_e5f6g7h8
```

**Kết quả:**
```
╔══════════════════════════════════════════╗
║           ACCOUNT DETAILS                ║
╚══════════════════════════════════════════╝

Account ID:  ACC_e5f6g7h8
Person ID:   PERS_a1b2c3d4
Name:        Nguyễn Văn A
Type:        Customer
Email:       nguyenvana@email.com
Status:      Active
Created:     2025-01-20 08:00:00 UTC

📱 Wallets:
┌─────────────────┬──────────┬─────────────────┐
│ WALLET ID       │ TYPE     │ CURRENCIES      │
├─────────────────┼──────────┼─────────────────┤
│ WAL_q7r8s9t0    │ Funding  │ USD, VND        │
│ WAL_u1v2w3x4    │ Spot     │ USDT, BTC, ETH  │
└─────────────────┴──────────┴─────────────────┘
```

### 3.4 account balance - Xem số dư

```powershell
# Xem tất cả số dư
simbank account balance ACC_e5f6g7h8

# Xem số dư một loại ví
simbank account balance ACC_e5f6g7h8 --wallet funding

# Xem số dư một loại tiền
simbank account balance ACC_e5f6g7h8 --currency USD
```

**Kết quả:**
```
╔══════════════════════════════════════════╗
║           ACCOUNT BALANCE                ║
╚══════════════════════════════════════════╝

Account: ACC_e5f6g7h8 (Nguyễn Văn A)

💰 Funding Wallet (WAL_q7r8s9t0):
   USD:     $  15,000.00  (Available: $15,000.00 | Locked: $0.00)
   VND:  ₫ 350,000,000    (Available: ₫350,000,000 | Locked: ₫0)

💰 Spot Wallet (WAL_u1v2w3x4):
   USDT:    $   5,000.00
   BTC:     ₿      0.5000
   ETH:     Ξ      2.0000

📊 Total (USD equivalent): $25,500.00
```

---

## 4. Giao dịch

### 4.1 deposit - Gửi tiền

```powershell
# Gửi tiền cơ bản
simbank deposit ACC_e5f6g7h8 10000 USD

# Gửi vào ví cụ thể
simbank deposit ACC_e5f6g7h8 10000 USD --wallet funding

# Gửi với ghi chú
simbank deposit ACC_e5f6g7h8 10000 USD --note "Initial deposit"

# Gửi nhiều loại tiền
simbank deposit ACC_e5f6g7h8 5000000000 VND
simbank deposit ACC_e5f6g7h8 0.5 BTC --wallet spot
```

**Kết quả:**
```
✅ Deposit successful!

Transaction Details:
   ID:       TXN_y5z6a7b8
   Account:  ACC_e5f6g7h8
   Amount:   $10,000.00 USD
   Wallet:   Funding
   Time:     2025-01-25 10:30:45 UTC

📊 New Balance:
   Funding USD: $25,000.00
```

### 4.2 withdraw - Rút tiền

```powershell
# Rút tiền cơ bản
simbank withdraw ACC_e5f6g7h8 5000 USD

# Rút từ ví cụ thể
simbank withdraw ACC_e5f6g7h8 5000 USD --wallet funding

# Rút với ghi chú
simbank withdraw ACC_e5f6g7h8 5000 USD --note "ATM withdrawal"
```

**Kết quả:**
```
✅ Withdrawal successful!

Transaction Details:
   ID:       TXN_c9d0e1f2
   Account:  ACC_e5f6g7h8
   Amount:   $5,000.00 USD
   Wallet:   Funding
   Time:     2025-01-25 10:35:00 UTC

📊 New Balance:
   Funding USD: $20,000.00

⚠️ AML Note: Transaction flagged as near_threshold
```

### 4.3 transfer - Chuyển khoản

```powershell
# Chuyển giữa các tài khoản
simbank transfer ACC_e5f6g7h8 ACC_i9j0k1l2 3000 USD

# Chuyển giữa các ví trong cùng tài khoản
simbank transfer ACC_e5f6g7h8 ACC_e5f6g7h8 2000 USDT --from-wallet funding --to-wallet spot

# Chuyển với ghi chú
simbank transfer ACC_e5f6g7h8 ACC_i9j0k1l2 3000 USD --note "Payment for services"
```

**Kết quả:**
```
✅ Transfer successful!

Transaction Details:
   ID:        TXN_g3h4i5j6
   From:      ACC_e5f6g7h8 (Nguyễn Văn A)
   To:        ACC_i9j0k1l2 (Trần Thị B)
   Amount:    $3,000.00 USD
   Time:      2025-01-25 10:40:00 UTC

📊 Balances:
   Sender:   $17,000.00 USD (Funding)
   Receiver: $3,000.00 USD (Funding)
```

---

## 5. Kiểm toán và báo cáo

### 5.1 audit - Kiểm toán giao dịch

```powershell
# Kiểm toán toàn bộ
simbank audit

# Kiểm toán theo khoảng thời gian
simbank audit --from 2025-01-01 --to 2025-01-31

# Kiểm toán với AML flags
simbank audit --flags large_amount,near_threshold

# Kiểm toán cho tài khoản cụ thể
simbank audit --account ACC_e5f6g7h8

# Kiểm toán với chi tiết
simbank audit --verbose
```

**Kết quả:**
```
╔══════════════════════════════════════════╗
║           AML AUDIT REPORT               ║
╚══════════════════════════════════════════╝

Period: 2025-01-01 to 2025-01-31
Transactions Scanned: 156
Flagged Transactions: 12

🔍 Risk Assessment:
   Overall Risk Level: 🟡 Medium
   Risk Score:         35.5/100

📊 Flag Breakdown:
   large_amount:       5 transactions
   near_threshold:     4 transactions
   unusual_pattern:    2 transactions
   high_risk_country:  1 transaction

⚠️ Flagged Transactions:
┌────────────────┬──────────────────┬──────────┬─────────────────────┐
│ TX ID          │ ACCOUNT          │ AMOUNT   │ FLAGS               │
├────────────────┼──────────────────┼──────────┼─────────────────────┤
│ TXN_a1b2c3d4   │ ACC_e5f6g7h8     │ $15,000  │ large_amount        │
│ TXN_e5f6g7h8   │ ACC_i9j0k1l2     │ $9,500   │ near_threshold      │
│ TXN_i9j0k1l2   │ ACC_m3n4o5p6     │ $25,000  │ large_amount        │
└────────────────┴──────────────────┴──────────┴─────────────────────┘
```

### 5.2 report - Xuất báo cáo

```powershell
# Báo cáo giao dịch
simbank report transactions

# Báo cáo AML
simbank report aml

# Báo cáo tài khoản
simbank report accounts

# Chọn format
simbank report transactions --format csv
simbank report transactions --format json
simbank report transactions --format markdown

# Xuất ra file
simbank report transactions --format csv --output ./reports/transactions.csv

# Lọc theo thời gian
simbank report transactions --from 2025-01-01 --to 2025-01-31

# Lọc theo tài khoản
simbank report transactions --account ACC_e5f6g7h8
```

**Kết quả (CSV):**
```csv
id,account_id,wallet_id,tx_type,amount,currency,description,created_at
TXN_a1b2c3d4,ACC_e5f6g7h8,WAL_q7r8s9t0,deposit,15000,USD,Initial deposit,2025-01-20T08:00:00Z
TXN_e5f6g7h8,ACC_e5f6g7h8,WAL_q7r8s9t0,withdrawal,5000,USD,ATM withdrawal,2025-01-21T10:30:00Z
```

**Kết quả (Markdown):**
```markdown
# Transaction Report

## Summary
- Period: 2025-01-01 to 2025-01-31
- Total Transactions: 156
- Total Volume: $1,250,000.00

## Transactions

| ID | Account | Type | Amount | Currency | Date |
|----|---------|------|--------|----------|------|
| TXN_a1b2c3d4 | ACC_e5f6g7h8 | Deposit | $15,000 | USD | 2025-01-20 |
```

---

## 6. Cấu hình

### 6.1 Environment Variables

```powershell
# Đường dẫn data
$env:SIMBANK_DATA_DIR = "C:\simbank\data"

# Database file
$env:SIMBANK_DB_PATH = "C:\simbank\data\simbank.db"

# Events directory
$env:SIMBANK_EVENTS_DIR = "C:\simbank\data\events"

# Log level
$env:SIMBANK_LOG_LEVEL = "info"  # debug, info, warn, error
```

### 6.2 Config File

```toml
# simbank.toml
[database]
path = "data/simbank.db"

[events]
directory = "data/events"
rotation = "daily"

[aml]
large_amount_threshold = 10000
near_threshold_range = [9000, 10000]
high_risk_countries = ["IR", "KP", "SY", "CU"]

[logging]
level = "info"
format = "pretty"  # pretty, json
```

---

## 7. Exit Codes

| Code | Ý nghĩa |
|------|---------|
| 0 | Thành công |
| 1 | Lỗi chung |
| 2 | Lỗi tham số |
| 3 | Lỗi database |
| 4 | Lỗi nghiệp vụ (insufficient funds, etc.) |
| 5 | Lỗi AML (transaction blocked) |

---

## 8. Troubleshooting

### 8.1 Database không tồn tại

```
Error: Database not found at data/simbank.db
```

**Giải pháp:**
```powershell
simbank init
```

### 8.2 Không đủ số dư

```
Error: Insufficient balance
  Available: $5,000.00 USD
  Required:  $10,000.00 USD
```

**Giải pháp:** Kiểm tra số dư trước khi giao dịch
```powershell
simbank account balance ACC_e5f6g7h8
```

### 8.3 Tài khoản không tồn tại

```
Error: Account not found: ACC_invalid
```

**Giải pháp:** Kiểm tra danh sách tài khoản
```powershell
simbank account list
```

### 8.4 Giao dịch bị chặn

```
Error: Transaction blocked by AML rules
  Reason: high_risk_country
  Location: KP (North Korea)
```

**Giải pháp:** Liên hệ compliance team

---

## 9. Ví dụ workflow hoàn chỉnh

```powershell
# 1. Khởi tạo hệ thống
simbank init

# 2. Tạo tài khoản khách hàng
simbank account create --name "Nguyễn Văn A" --type customer --email "nva@email.com"
# Output: ACC_e5f6g7h8

# 3. Gửi tiền
simbank deposit ACC_e5f6g7h8 100000 USD --note "Account opening"

# 4. Xem số dư
simbank account balance ACC_e5f6g7h8

# 5. Chuyển tiền nội bộ
simbank transfer ACC_e5f6g7h8 ACC_e5f6g7h8 50000 USD --from-wallet funding --to-wallet spot

# 6. Tạo tài khoản thứ hai
simbank account create --name "Trần Thị B" --type customer
# Output: ACC_i9j0k1l2

# 7. Chuyển tiền cho tài khoản khác
simbank transfer ACC_e5f6g7h8 ACC_i9j0k1l2 10000 USD

# 8. Kiểm toán
simbank audit --from 2025-01-01

# 9. Xuất báo cáo
simbank report transactions --format csv --output ./reports/jan2025.csv

# 10. Xem trạng thái
simbank status
```

---

## 10. Tham khảo

| Lệnh | Mô tả |
|------|-------|
| `simbank --help` | Trợ giúp chung |
| `simbank <command> --help` | Trợ giúp lệnh cụ thể |
| `simbank --version` | Phiên bản CLI |
