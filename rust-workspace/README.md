# Rust Workspace Project

Dự án Rust với cấu trúc workspace gồm nhiều crates.

## Cấu trúc dự án

```
rust-workspace/
├── Cargo.toml          # Workspace configuration
├── crates/
│   ├── core/           # Core library - business logic
│   ├── utils/          # Utility functions and helpers
│   ├── cli/            # Command-line interface
│   └── api/            # API server
└── README.md
```

## Các Crates

- **core**: Chứa business logic chính của ứng dụng
- **utils**: Các hàm tiện ích dùng chung
- **cli**: Giao diện dòng lệnh
- **api**: REST API server

## Build

```bash
# Build tất cả crates
cargo build

# Build một crate cụ thể
cargo build -p core
cargo build -p cli

# Run CLI
cargo run -p cli

# Run API server
cargo run -p api

# Run tests
cargo test

# Run tests cho một crate
cargo test -p core
```

## Development

```bash
# Check code
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```
