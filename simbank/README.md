# SIMBANK

Ứng dụng ngân hàng đơn giản.

## Mục đích
- Minh họa cách xây dựng DSL trong Rust.
- Minh họa cách sử dụng DSL để mô tả nghiệp vụ ngân hàng.
- DSL trong Rust giúp Business Analyst (BA) và các chuyên gia nghiệp vụ dễ dàng định nghĩa các quy tắc kinh doanh phức tạp mà không cần kiến thức lập trình sâu sắc.

## Hướng dẫn viết DSL
DSL bằng tiếng Anh.
Comment bằng tiếng Việt.

## Thực hiện các nghiệp vụ ngân hàng với DSL.
### Nghiệp vụ ngân hàng cho khách hàng:
- Tạo tài khoản tiền gửi
- Gửi tiết kiệm
- Thu phí thường niên

### Nghiệp vụ ngân hàng cho nhân viên ngân hàng:
- Tài khoản tiền gửi
- Bảo hiểm

### Nghiệp vụ ngân hàng cho quản lý ngân hàng:
- Thưởng

### Nghiệp vụ ngân hàng cho kiểm toán bên thứ 3(The Big 4—Deloitte, PwC, EY (Ernst & Young), and KPMG):
- Kiểm toán

### Nghiệp vụ ngân hàng cho cổ đông:
- Cổ tức


## Công nghệ
- Rust: Ngôn ngữ lõi/ core language
- DSL (Domain Specific Language - Ngôn ngữ đặc tả miền): Định nghĩa các điều kiện phức tạp.
- SQLite: Lưu trữ dữ liệu nhẹ. Dành cho ứng dụng nhỏ và nhúng.

## Kiến trúc dự án
- simbank/: Mã nguồn chính của ứng dụng ngân hàng.
- simbank/crates/core-banking/: Mã nguồn lõi của ứng dụng ngân hàng.
- simbank/crates/business/: Mã nguồn nghiệp vụ ngân hàng.
- simbank/crates/dsl-macros/: Module chứa các macro DSL cho nghiệp vụ ngân hàng.
- simbank/crates/reports/: Module xuất báo cáo.
- simbank/examples/: Ví dụ sử dụng DSL để mô tả nghiệp vụ ngân hàng.
- simbank/tests/: Bộ kiểm thử để đảm bảo tính đúng đắn của DSL và ứng dụng ngân hàng.