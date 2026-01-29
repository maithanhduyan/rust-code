# PostgreSQL MCP Server

A Model Context Protocol (MCP) server for PostgreSQL database operations, written in Rust.

## Features

### Core Tools
- **connect_postgres**: Kết nối đến PostgreSQL server
- **count_databases**: Đếm số lượng database
- **list_databases**: Liệt kê tất cả database
- **list_schemas**: Liệt kê tất cả schema
- **list_tables**: Liệt kê tất cả table trong schema
- **table_structure**: Lấy cấu trúc chi tiết của table
- **table_data**: Lấy dữ liệu từ table
- **execute_query**: Thực thi SELECT queries (read-only)

### Advanced Tools
- **explain_query**: Phân tích query plan
- **get_top_queries**: Tìm slow queries (cần pg_stat_statements)
- **analyze_db_health**: Phân tích sức khỏe database

### Access Control Tools
- **set_access_mode**: Thiết lập access mode (restricted/unrestricted)
- **get_access_mode**: Xem access mode hiện tại
- **execute_sql**: Thực thi bất kỳ SQL nào (chỉ unrestricted mode)

## Access Modes

- **Restricted (default)**: Read-only, an toàn cho production
  - Chỉ cho phép SELECT queries
  - DDL/DML bị block

- **Unrestricted**: Full access, dành cho development
  - Cho phép tất cả SQL operations
  - ⚠️ Sử dụng cẩn thận!

## Building

```bash
cargo build --release
```

## Transport Modes

### STDIO Transport (mặc định)
Dành cho single client, tích hợp trực tiếp với VS Code/Claude Desktop.

```bash
./postgres-mcp.exe --transport stdio
```

### SSE Transport
Dành cho nhiều clients dùng chung một server.

```bash
./postgres-mcp.exe --transport sse --port 8000
```

**Endpoints:**
- `GET /` - Server info
- `GET /sse` - SSE stream cho responses
- `POST /message` - Gửi JSON-RPC requests

## Usage

### VS Code MCP Configuration (STDIO)

Add to your `mcp.json`:

```json
{
  "servers": {
    "postgres-mcp": {
      "command": "${workspaceFolder}/.tools/postgres-mcp/target/release/postgres-mcp.exe",
      "args": ["--transport", "stdio"]
    }
  }
}
```

### SSE Client Example (PowerShell)

```powershell
# Start server
Start-Process .\postgres-mcp.exe -ArgumentList "--transport","sse","--port","8000"

# Test connection
Invoke-RestMethod -Uri http://localhost:8000/

# Call tool
$body = '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_databases","arguments":{}}}'
Invoke-RestMethod -Uri http://localhost:8000/message -Method POST -ContentType "application/json" -Body $body
```

## Protocol

This server implements the Model Context Protocol (MCP) with JSON-RPC 2.0.

### Available Methods

- `initialize`: Initialize the server
- `tools/list`: List available tools
- `tools/call`: Call a specific tool

## License

MIT
