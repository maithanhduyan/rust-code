Đây là bản phân tích và quyết định cuối cùng dựa trên các phương án bạn đưa ra.

Tôi hoàn toàn đồng ý và khuyên dùng **Option C: Hybrid (Recommended)**. Đây là kiến trúc cân bằng nhất, tuân thủ nguyên lý **Clean Architecture** (tách biệt Domain và Infrastructure) mà không tạo ra quá nhiều boilerplate code như Option B.

Dưới đây là quyết định chi tiết cho các vấn đề thảo luận:

### 1. Chọn kiến trúc: Option C (Hybrid)

Tại sao lại chọn **Option C**?
*   **Tách biệt rõ ràng (Separation of Concerns):**
    *   `core`: Chứa "Domain Entities" (Money, Person, Account). Đây là trái tim của hệ thống, không phụ thuộc vào database hay file system.
    *   `persistence`: Đóng vai trò là "Infrastructure Layer". Nó gom tất cả logic I/O (SQLite cho state hiện tại + JSONL cho lịch sử). Việc gộp này hợp lý vì cả hai đều là chi tiết kỹ thuật về cách lưu trữ dữ liệu.
    *   `business`: Chứa "Use Cases" (Logic nghiệp vụ). Nó chỉ gọi các trait được định nghĩa trong `persistence`, không quan tâm bên dưới là SQL hay File.
*   **Dễ mở rộng:** Nếu sau này bạn muốn thêm Redis cache hoặc đổi sang PostgreSQL, bạn chỉ cần sửa crate `persistence`.
*   **Độ phức tạp vừa phải:** 5 crates là con số lý tưởng để quản lý trong một workspace Rust.

### 2. Xử lý thư mục `data/`

**Quyết định:** Không commit dữ liệu thực tế, chỉ commit cấu trúc thư mục.

*   Thêm vào `.gitignore`:
    ```gitignore
    # Ignore runtime data
    data/simbank.db
    data/simbank.db-shm
    data/simbank.db-wal
    data/events/*.jsonl

    # Keep the folder structure
    !data/.gitkeep
    !data/events/.gitkeep
    ```
*   **Lý do:** Dữ liệu này thay đổi liên tục khi chạy test/app. Việc commit binary file (như `.db`) vào git sẽ làm phình repo và gây conflict không cần thiết.

### 3. Vị trí thư mục `migrations/`

**Quyết định:** Đặt tại **Workspace Root**.

*   Cấu trúc:
    ```
    simbank/
    ├── Cargo.toml
    ├── migrations/
    │   └── 202601250001_create_initial_schema.sql
    └── ...
    ```
*   **Lý do:**
    *   Công cụ CLI `sqlx` thường chạy tốt nhất từ root.
    *   Schema Database là tài sản chung của toàn bộ dự án, không nên giấu quá sâu trong một sub-crate.
    *   Dễ dàng chạy lệnh `sqlx migrate run` từ thư mục gốc mà không cần `cd` vào trong `crates/persistence`.

---

### Cấu trúc file `Cargo.toml` (Workspace) dự kiến

Đây là bước đầu tiên chúng ta cần thực hiện. Nội dung file `simbank/Cargo.toml` sẽ trông như thế này:

```toml
[workspace]
members = [
    "crates/core",
    "crates/persistence",
    "crates/business",
    "crates/dsl",
    "crates/reports",
]
resolver = "2"

# Định nghĩa dependencies chung để đồng bộ version giữa các crate
[workspace.dependencies]
tokio = { version = "1.36", features = ["full"] }
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite"] } # Lưu ý check version mới nhất
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
chrono = { version = "0.4", features = ["serde"] }

# Internal dependencies
simbank-core = { path = "./crates/core" }
simbank-persistence = { path = "./crates/persistence" }
simbank-business = { path = "./crates/business" }
```

### Next Steps (Hành động ngay)

1.  **Tạo thư mục:** Xóa (hoặc backup) các thư mục cũ nếu cần, và tạo cấu trúc thư mục theo **Option C**.
2.  **Khởi tạo Crate:** Chạy `cargo new --lib crates/core`, `crates/persistence`, v.v.
3.  **Setup DB:**
    *   Cài đặt `sqlx-cli`.
    *   Tạo file `migrations/20260125_init.sql`.
    *   Chạy `sqlx database create` và `sqlx migrate run`.

Bạn có muốn tôi cung cấp nội dung chi tiết cho file `migrations/20260125_init.sql` (Schema thiết kế cho hỗ trợ đa tiền tệ và các vai trò Customer/Employee...) không?