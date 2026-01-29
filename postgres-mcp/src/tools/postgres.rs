// PostgreSQL MCP Tools
// Implements all PostgreSQL database operations as MCP tools

use crate::mcp_core::{McpResult, McpTool};
use crate::tools::Tool;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio_postgres::{Client, NoTls, Row};

// Access modes for database operations
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AccessMode {
    Restricted,   // Read-only, safe for production
    Unrestricted, // Full access, for development
}

impl Default for AccessMode {
    fn default() -> Self {
        AccessMode::Restricted
    }
}

// Global PostgreSQL connection state
lazy_static::lazy_static! {
    static ref PG_CONNECTION: Arc<Mutex<Option<PgConnection>>> = Arc::new(Mutex::new(None));
    static ref RUNTIME: Runtime = Runtime::new().expect("Failed to create Tokio runtime");
    static ref ACCESS_MODE: Arc<Mutex<AccessMode>> = Arc::new(Mutex::new(AccessMode::Restricted));
}

struct PgConnection {
    client: Client,
}

// Helper function to format query results as text
fn format_result_text(text: &str) -> serde_json::Value {
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": text
        }]
    })
}

// Helper to check if current mode allows write operations
fn is_write_allowed() -> bool {
    let mode = ACCESS_MODE.lock().unwrap();
    *mode == AccessMode::Unrestricted
}

// Helper to get current access mode as string
fn get_access_mode_str() -> String {
    let mode = ACCESS_MODE.lock().unwrap();
    match *mode {
        AccessMode::Restricted => "🔒 Restricted (read-only)".to_string(),
        AccessMode::Unrestricted => "🔓 Unrestricted (full access)".to_string(),
    }
}

// ============================================================================
// SetAccessModeTool
// ============================================================================

#[derive(Deserialize)]
struct SetAccessModeParams {
    mode: String,
}

pub struct SetAccessModeTool;

impl SetAccessModeTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SetAccessModeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for SetAccessModeTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "set_access_mode".to_string(),
            description: "Thiết lập access mode: restricted (production) hoặc unrestricted (development)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "description": "Access mode: 'restricted' (read-only) hoặc 'unrestricted' (full access)",
                        "enum": ["restricted", "unrestricted"]
                    }
                },
                "required": ["mode"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let mode_params: SetAccessModeParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid parameters: {}", e))?;

        let new_mode = match mode_params.mode.to_lowercase().as_str() {
            "restricted" => AccessMode::Restricted,
            "unrestricted" => AccessMode::Unrestricted,
            _ => return Ok(format_result_text("❌ Invalid mode. Use 'restricted' or 'unrestricted'")),
        };

        let mut mode = ACCESS_MODE.lock().unwrap();
        *mode = new_mode;

        let message = match new_mode {
            AccessMode::Restricted => "🔒 Access mode set to RESTRICTED (read-only, safe for production)\n• Only SELECT queries allowed\n• DDL/DML operations blocked",
            AccessMode::Unrestricted => "🔓 Access mode set to UNRESTRICTED (full access, for development)\n• All SQL operations allowed\n• ⚠️ Use with caution!",
        };

        Ok(format_result_text(message))
    }
}

// ============================================================================
// GetAccessModeTool
// ============================================================================

pub struct GetAccessModeTool;

impl GetAccessModeTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GetAccessModeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GetAccessModeTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "get_access_mode".to_string(),
            description: "Hiển thị access mode hiện tại".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn execute(&self, _params: serde_json::Value) -> McpResult<serde_json::Value> {
        let mode_str = get_access_mode_str();
        let info = match *ACCESS_MODE.lock().unwrap() {
            AccessMode::Restricted => format!("{}\n\n📋 Allowed operations:\n• SELECT queries\n• EXPLAIN queries\n• Health checks\n\n🚫 Blocked operations:\n• CREATE/DROP/ALTER\n• INSERT/UPDATE/DELETE\n• TRUNCATE", mode_str),
            AccessMode::Unrestricted => format!("{}\n\n📋 Allowed operations:\n• All SQL operations\n• DDL (CREATE, DROP, ALTER)\n• DML (INSERT, UPDATE, DELETE)\n\n⚠️ Warning: Use only in development!", mode_str),
        };
        Ok(format_result_text(&info))
    }
}

// ============================================================================
// ExecuteSqlTool (replaces execute_query with access mode support)
// ============================================================================

#[derive(Deserialize)]
struct ExecuteSqlParams {
    sql: String,
    limit: Option<i64>,
}

pub struct ExecuteSqlTool;

impl ExecuteSqlTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExecuteSqlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ExecuteSqlTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "execute_sql".to_string(),
            description: "Thực thi SQL (respects access mode: restricted = SELECT only, unrestricted = all)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sql": {
                        "type": "string",
                        "description": "SQL statement to execute"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max rows for SELECT queries",
                        "default": 100
                    }
                },
                "required": ["sql"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let sql_params: ExecuteSqlParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid parameters: {}", e))?;
        let sql = sql_params.sql.trim();
        let limit = sql_params.limit.unwrap_or(100);

        // Parse SQL to determine operation type
        let sql_lower = sql.to_lowercase();
        let first_word = sql_lower.split_whitespace().next().unwrap_or("");

        let is_select = first_word == "select";
        let is_show = first_word == "show";
        let is_explain = first_word == "explain";
        let is_read_only = is_select || is_show || is_explain;

        // Check access mode
        if !is_read_only && !is_write_allowed() {
            return Ok(format_result_text(&format!(
                "🔒 BLOCKED: {} operation not allowed in RESTRICTED mode.\n\n\
                 Current mode: {}\n\n\
                 To enable write operations, run:\n\
                 set_access_mode(mode='unrestricted')",
                first_word.to_uppercase(),
                get_access_mode_str()
            )));
        }

        // Add LIMIT to SELECT if not present
        let final_sql = if is_select && !sql_lower.contains("limit") {
            format!("{} LIMIT {}", sql.trim_end_matches(';'), limit)
        } else {
            sql.to_string()
        };

        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            if is_select || is_show || is_explain {
                // Query that returns rows
                match conn.client.query(&final_sql, &[]).await {
                    Ok(rows) => {
                        if rows.is_empty() {
                            return Ok(format_result_text("📊 Query executed successfully. No rows returned."));
                        }

                        let columns: Vec<String> = rows[0]
                            .columns()
                            .iter()
                            .map(|c| c.name().to_string())
                            .collect();

                        let mut data_text = format!("📊 Query Result ({} rows):\n\n", rows.len());
                        data_text.push_str(&format!("Columns: {}\n", columns.join(" | ")));
                        data_text.push_str(&"-".repeat(80));
                        data_text.push('\n');

                        for row in rows.iter().take(20) {
                            let row_values: Vec<String> = (0..columns.len())
                                .map(|i| format_column_value(row, i))
                                .collect();
                            data_text.push_str(&row_values.join(" | "));
                            data_text.push('\n');
                        }

                        if rows.len() > 20 {
                            data_text.push_str(&format!("\n... và {} dòng khác", rows.len() - 20));
                        }

                        Ok(format_result_text(&data_text))
                    }
                    Err(e) => Ok(format_result_text(&format!("❌ Error: {}", e))),
                }
            } else {
                // DDL/DML that doesn't return rows
                match conn.client.execute(&final_sql, &[]).await {
                    Ok(affected) => {
                        Ok(format_result_text(&format!(
                            "✅ SQL executed successfully!\n\nStatement: {}\nRows affected: {}",
                            first_word.to_uppercase(),
                            affected
                        )))
                    }
                    Err(e) => Ok(format_result_text(&format!("❌ Error: {}", e))),
                }
            }
        });

        result
    }
}

// ============================================================================
// ConnectPostgresTool
// ============================================================================

#[derive(Deserialize)]
struct ConnectParams {
    host: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    password: String,
    database: Option<String>,
}

pub struct ConnectPostgresTool;

impl ConnectPostgresTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConnectPostgresTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ConnectPostgresTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "connect_postgres".to_string(),
            description: "Kết nối đến PostgreSQL server".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "host": {
                        "type": "string",
                        "description": "Địa chỉ server",
                        "default": "localhost"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Cổng kết nối",
                        "default": 5432
                    },
                    "user": {
                        "type": "string",
                        "description": "Tên người dùng",
                        "default": "postgres"
                    },
                    "password": {
                        "type": "string",
                        "description": "Mật khẩu"
                    },
                    "database": {
                        "type": "string",
                        "description": "Tên database",
                        "default": "postgres"
                    }
                },
                "required": ["password"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let connect_params: ConnectParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid parameters: {}", e))?;

        let host = connect_params.host.unwrap_or_else(|| "localhost".to_string());
        let port = connect_params.port.unwrap_or(5432);
        let user = connect_params.user.unwrap_or_else(|| "postgres".to_string());
        let database = connect_params.database.unwrap_or_else(|| "postgres".to_string());
        let password = connect_params.password;

        let connection_string = format!(
            "host={} port={} user={} password={} dbname={}",
            host, port, user, password, database
        );

        let result = RUNTIME.block_on(async {
            match tokio_postgres::connect(&connection_string, NoTls).await {
                Ok((client, connection)) => {
                    // Spawn the connection handler
                    tokio::spawn(async move {
                        if let Err(e) = connection.await {
                            eprintln!("Connection error: {}", e);
                        }
                    });

                    let mut conn_guard = PG_CONNECTION.lock().unwrap();
                    *conn_guard = Some(PgConnection { client });

                    let mode_str = get_access_mode_str();
                    Ok(format_result_text(&format!(
                        "✅ Kết nối thành công!\n\n📊 Database: {}\n🖥️ Host: {}:{}\n👤 User: {}\n🔐 Mode: {}",
                        database, host, port, user, mode_str
                    )))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Kết nối thất bại: {}", e))),
            }
        });

        result
    }
}

// ============================================================================
// CountDatabasesTool
// ============================================================================

pub struct CountDatabasesTool;

impl CountDatabasesTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CountDatabasesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for CountDatabasesTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "count_databases".to_string(),
            description: "Đếm số lượng database trong PostgreSQL".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn execute(&self, _params: serde_json::Value) -> McpResult<serde_json::Value> {
        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            match conn
                .client
                .query_one(
                    "SELECT COUNT(*) FROM pg_database WHERE datistemplate = false",
                    &[],
                )
                .await
            {
                Ok(row) => {
                    let count: i64 = row.get(0);
                    Ok(format_result_text(&format!("📊 Số lượng database: {}", count)))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}

// ============================================================================
// ListDatabasesTool
// ============================================================================

pub struct ListDatabasesTool;

impl ListDatabasesTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListDatabasesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ListDatabasesTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "list_databases".to_string(),
            description: "Liệt kê tất cả database".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn execute(&self, _params: serde_json::Value) -> McpResult<serde_json::Value> {
        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            match conn
                .client
                .query(
                    "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
                    &[],
                )
                .await
            {
                Ok(rows) => {
                    let db_list: Vec<String> = rows
                        .iter()
                        .map(|row| {
                            let name: String = row.get(0);
                            format!("• {}", name)
                        })
                        .collect();
                    Ok(format_result_text(&format!(
                        "📋 Danh sách database:\n{}",
                        db_list.join("\n")
                    )))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}

// ============================================================================
// ListSchemasTool
// ============================================================================

pub struct ListSchemasTool;

impl ListSchemasTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListSchemasTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ListSchemasTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "list_schemas".to_string(),
            description: "Liệt kê tất cả schema trong database hiện tại".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn execute(&self, _params: serde_json::Value) -> McpResult<serde_json::Value> {
        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            match conn
                .client
                .query(
                    "SELECT schema_name FROM information_schema.schemata
                     WHERE schema_name NOT IN ('information_schema', 'pg_catalog', 'pg_toast')
                     ORDER BY schema_name",
                    &[],
                )
                .await
            {
                Ok(rows) => {
                    let schema_list: Vec<String> = rows
                        .iter()
                        .map(|row| {
                            let name: String = row.get(0);
                            format!("• {}", name)
                        })
                        .collect();
                    Ok(format_result_text(&format!(
                        "🗂️ Danh sách schema:\n{}",
                        schema_list.join("\n")
                    )))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}

// ============================================================================
// ListTablesTool
// ============================================================================

#[derive(Deserialize)]
struct ListTablesParams {
    schema_name: Option<String>,
}

pub struct ListTablesTool;

impl ListTablesTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListTablesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ListTablesTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "list_tables".to_string(),
            description: "Liệt kê tất cả table trong schema".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "schema_name": {
                        "type": "string",
                        "description": "Tên schema",
                        "default": "public"
                    }
                }
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let list_params: ListTablesParams = serde_json::from_value(params).unwrap_or(ListTablesParams {
            schema_name: None,
        });
        let schema_name = list_params.schema_name.unwrap_or_else(|| "public".to_string());

        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            match conn
                .client
                .query(
                    "SELECT table_name, table_type FROM information_schema.tables
                     WHERE table_schema = $1 ORDER BY table_name",
                    &[&schema_name],
                )
                .await
            {
                Ok(rows) => {
                    let table_list: Vec<String> = rows
                        .iter()
                        .map(|row| {
                            let name: String = row.get(0);
                            let table_type: String = row.get(1);
                            format!("• {} ({})", name, table_type)
                        })
                        .collect();
                    Ok(format_result_text(&format!(
                        "📋 Tables trong schema '{}':\n{}",
                        schema_name,
                        table_list.join("\n")
                    )))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}

// ============================================================================
// TableStructureTool
// ============================================================================

#[derive(Deserialize)]
struct TableStructureParams {
    table_name: String,
    schema_name: Option<String>,
}

pub struct TableStructureTool;

impl TableStructureTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TableStructureTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TableStructureTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "table_structure".to_string(),
            description: "Lấy cấu trúc chi tiết của table".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "table_name": {
                        "type": "string",
                        "description": "Tên table"
                    },
                    "schema_name": {
                        "type": "string",
                        "description": "Tên schema",
                        "default": "public"
                    }
                },
                "required": ["table_name"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let structure_params: TableStructureParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid parameters: {}", e))?;
        let table_name = structure_params.table_name;
        let schema_name = structure_params.schema_name.unwrap_or_else(|| "public".to_string());

        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            match conn
                .client
                .query(
                    "SELECT
                        column_name,
                        data_type,
                        is_nullable,
                        column_default,
                        character_maximum_length
                     FROM information_schema.columns
                     WHERE table_schema = $1 AND table_name = $2
                     ORDER BY ordinal_position",
                    &[&schema_name, &table_name],
                )
                .await
            {
                Ok(rows) => {
                    let columns_info: Vec<String> = rows
                        .iter()
                        .map(|row| {
                            let col_name: String = row.get(0);
                            let data_type: String = row.get(1);
                            let nullable: String = row.get(2);
                            let default: Option<String> = row.get(3);
                            let max_len: Option<i32> = row.get(4);

                            let mut info = format!("• {}: {}", col_name, data_type);
                            if let Some(len) = max_len {
                                info.push_str(&format!("({})", len));
                            }
                            if nullable == "NO" {
                                info.push_str(" NOT NULL");
                            }
                            if let Some(def) = default {
                                info.push_str(&format!(" DEFAULT {}", def));
                            }
                            info
                        })
                        .collect();
                    Ok(format_result_text(&format!(
                        "🏗️ Cấu trúc table '{}':\n{}",
                        table_name,
                        columns_info.join("\n")
                    )))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}

// ============================================================================
// TableDataTool
// ============================================================================

#[derive(Deserialize)]
struct TableDataParams {
    table_name: String,
    schema_name: Option<String>,
    limit: Option<i64>,
}

pub struct TableDataTool;

impl TableDataTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TableDataTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TableDataTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "table_data".to_string(),
            description: "Lấy dữ liệu từ table".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "table_name": {
                        "type": "string",
                        "description": "Tên table"
                    },
                    "schema_name": {
                        "type": "string",
                        "description": "Tên schema",
                        "default": "public"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Số lượng record tối đa",
                        "default": 10
                    }
                },
                "required": ["table_name"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let data_params: TableDataParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid parameters: {}", e))?;
        let table_name = data_params.table_name;
        let schema_name = data_params.schema_name.unwrap_or_else(|| "public".to_string());
        let limit = data_params.limit.unwrap_or(10);

        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            // First, get column names
            let columns_result = conn
                .client
                .query(
                    "SELECT column_name FROM information_schema.columns
                     WHERE table_schema = $1 AND table_name = $2
                     ORDER BY ordinal_position",
                    &[&schema_name, &table_name],
                )
                .await;

            let columns: Vec<String> = match columns_result {
                Ok(rows) => rows.iter().map(|r| r.get(0)).collect(),
                Err(e) => return Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            };

            // Build and execute the data query
            let query = format!(
                "SELECT * FROM \"{}\".\"{}\" LIMIT {}",
                schema_name, table_name, limit
            );

            match conn.client.query(&query, &[]).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        return Ok(format_result_text(&format!(
                            "📊 Table '{}' không có dữ liệu",
                            table_name
                        )));
                    }

                    let mut data_text = format!(
                        "📊 Dữ liệu từ table '{}' (tối đa {} records):\n\n",
                        table_name, limit
                    );
                    data_text.push_str(&format!("Columns: {}\n", columns.join(" | ")));
                    data_text.push_str(&"-".repeat(80));
                    data_text.push('\n');

                    for row in rows.iter().take(5) {
                        let row_values: Vec<String> = (0..columns.len())
                            .map(|i| format_column_value(row, i))
                            .collect();
                        data_text.push_str(&row_values.join(" | "));
                        data_text.push('\n');
                    }

                    if rows.len() > 5 {
                        data_text.push_str(&format!("\n... và {} dòng khác", rows.len() - 5));
                    }

                    Ok(format_result_text(&data_text))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}

// Helper function to format column value
fn format_column_value(row: &Row, idx: usize) -> String {
    // Try different types
    if let Ok(val) = row.try_get::<_, Option<String>>(idx) {
        return val
            .map(|s| if s.len() > 20 { format!("{}...", &s[..17]) } else { s })
            .unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<i32>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<i64>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<f64>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<bool>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    "???".to_string()
}

// ============================================================================
// ExplainQueryTool
// ============================================================================

#[derive(Deserialize)]
struct ExplainQueryParams {
    query: String,
    analyze: Option<bool>,
    format: Option<String>,
}

pub struct ExplainQueryTool;

impl ExplainQueryTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExplainQueryTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ExplainQueryTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "explain_query".to_string(),
            description: "Hiển thị query plan - phân tích cách PostgreSQL thực thi query".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "SQL query cần phân tích"
                    },
                    "analyze": {
                        "type": "boolean",
                        "description": "Thực thi query thật để lấy thời gian thực (mặc định: false)",
                        "default": false
                    },
                    "format": {
                        "type": "string",
                        "description": "Định dạng output: text, json, yaml, xml",
                        "default": "text",
                        "enum": ["text", "json", "yaml", "xml"]
                    }
                },
                "required": ["query"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let explain_params: ExplainQueryParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid parameters: {}", e))?;
        let query = explain_params.query.trim();
        let analyze = explain_params.analyze.unwrap_or(false);
        let format = explain_params.format.unwrap_or_else(|| "text".to_string());

        // Build EXPLAIN query
        let mut explain_opts = vec!["VERBOSE", "COSTS", "BUFFERS"];
        if analyze {
            explain_opts.insert(0, "ANALYZE");
        }
        let format_upper = format.to_uppercase();
        let explain_query = format!(
            "EXPLAIN ({}, FORMAT {}) {}",
            explain_opts.join(", "),
            format_upper,
            query
        );

        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            match conn.client.query(&explain_query, &[]).await {
                Ok(rows) => {
                    let mut plan_text = "📊 Query Execution Plan:\n\n".to_string();
                    plan_text.push_str(&format!("Query: {}\n", query));
                    plan_text.push_str(&"-".repeat(80));
                    plan_text.push('\n');

                    for row in &rows {
                        let line: String = row.get(0);
                        plan_text.push_str(&line);
                        plan_text.push('\n');
                    }

                    if analyze {
                        plan_text.push_str("\n⚠️ ANALYZE mode: Query đã được thực thi thật!\n");
                    }

                    Ok(format_result_text(&plan_text))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}

// ============================================================================
// GetTopQueriesTool
// ============================================================================

#[derive(Deserialize)]
struct GetTopQueriesParams {
    limit: Option<i64>,
    order_by: Option<String>,
}

pub struct GetTopQueriesTool;

impl GetTopQueriesTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GetTopQueriesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GetTopQueriesTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "get_top_queries".to_string(),
            description: "Tìm các slow queries từ pg_stat_statements - cần extension pg_stat_statements".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Số lượng queries trả về",
                        "default": 10
                    },
                    "order_by": {
                        "type": "string",
                        "description": "Sắp xếp theo: total_time, mean_time, calls",
                        "default": "total_time",
                        "enum": ["total_time", "mean_time", "calls"]
                    }
                }
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let query_params: GetTopQueriesParams = serde_json::from_value(params)
            .unwrap_or(GetTopQueriesParams { limit: None, order_by: None });
        let limit = query_params.limit.unwrap_or(10);
        let order_by = query_params.order_by.unwrap_or_else(|| "total_time".to_string());

        let order_column = match order_by.as_str() {
            "mean_time" => "mean_exec_time",
            "calls" => "calls",
            _ => "total_exec_time",
        };

        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            // Check if pg_stat_statements extension exists
            let check_ext = conn.client.query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'pg_stat_statements')",
                &[],
            ).await;

            match check_ext {
                Ok(row) => {
                    let exists: bool = row.get(0);
                    if !exists {
                        return Ok(format_result_text(
                            "❌ Extension pg_stat_statements chưa được cài đặt.\n\
                             Để cài đặt, chạy: CREATE EXTENSION pg_stat_statements;"
                        ));
                    }
                }
                Err(e) => return Ok(format_result_text(&format!("❌ Lỗi kiểm tra extension: {}", e))),
            }

            // Query pg_stat_statements
            let query = format!(
                "SELECT
                    queryid,
                    LEFT(query, 100) as query_preview,
                    calls,
                    ROUND(total_exec_time::numeric, 2) as total_time_ms,
                    ROUND(mean_exec_time::numeric, 2) as mean_time_ms,
                    ROUND(min_exec_time::numeric, 2) as min_time_ms,
                    ROUND(max_exec_time::numeric, 2) as max_time_ms,
                    rows
                 FROM pg_stat_statements
                 WHERE query NOT LIKE '%pg_stat_statements%'
                 ORDER BY {} DESC
                 LIMIT {}",
                order_column, limit
            );

            match conn.client.query(&query, &[]).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        return Ok(format_result_text("📊 Không có dữ liệu query statistics"));
                    }

                    let mut text = format!("🔍 Top {} Slow Queries (order by {}):\n\n", limit, order_by);

                    for (i, row) in rows.iter().enumerate() {
                        let queryid: i64 = row.get(0);
                        let query_preview: String = row.get(1);
                        let calls: i64 = row.get(2);
                        let total_time: rust_decimal::Decimal = row.get(3);
                        let mean_time: rust_decimal::Decimal = row.get(4);
                        let min_time: rust_decimal::Decimal = row.get(5);
                        let max_time: rust_decimal::Decimal = row.get(6);
                        let rows_count: i64 = row.get(7);

                        text.push_str(&format!(
                            "{}. Query ID: {}\n   Query: {}...\n   Calls: {} | Total: {}ms | Mean: {}ms | Min: {}ms | Max: {}ms | Rows: {}\n\n",
                            i + 1, queryid, query_preview.replace('\n', " "),
                            calls, total_time, mean_time, min_time, max_time, rows_count
                        ));
                    }

                    Ok(format_result_text(&text))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}

// ============================================================================
// AnalyzeDbHealthTool
// ============================================================================

pub struct AnalyzeDbHealthTool;

impl AnalyzeDbHealthTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AnalyzeDbHealthTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for AnalyzeDbHealthTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "analyze_db_health".to_string(),
            description: "Phân tích sức khỏe database: buffer cache, connections, indexes, vacuum, sequences".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn execute(&self, _params: serde_json::Value) -> McpResult<serde_json::Value> {
        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            let mut health_report = "🏥 Database Health Report\n".to_string();
            health_report.push_str(&"=".repeat(80));
            health_report.push('\n');

            // 1. Buffer Cache Hit Rate
            health_report.push_str("\n📊 1. Buffer Cache Hit Rate\n");
            health_report.push_str(&"-".repeat(40));
            health_report.push('\n');

            let cache_query = "SELECT
                ROUND(100.0 * sum(blks_hit) / nullif(sum(blks_hit) + sum(blks_read), 0), 2) as hit_rate
                FROM pg_stat_database";

            match conn.client.query_one(cache_query, &[]).await {
                Ok(row) => {
                    let hit_rate: Option<rust_decimal::Decimal> = row.get(0);
                    match hit_rate {
                        Some(rate) => {
                            let status = if rate >= rust_decimal::Decimal::from(99) { "✅ Excellent" }
                                        else if rate >= rust_decimal::Decimal::from(95) { "⚠️ Good" }
                                        else { "❌ Poor" };
                            health_report.push_str(&format!("   Cache Hit Rate: {}% {}\n", rate, status));
                        }
                        None => health_report.push_str("   Cache Hit Rate: N/A (no data)\n"),
                    }
                }
                Err(e) => health_report.push_str(&format!("   ❌ Error: {}\n", e)),
            }

            // 2. Connection Health
            health_report.push_str("\n📊 2. Connection Health\n");
            health_report.push_str(&"-".repeat(40));
            health_report.push('\n');

            let conn_query = "SELECT
                (SELECT count(*) FROM pg_stat_activity) as total_connections,
                (SELECT setting::int FROM pg_settings WHERE name = 'max_connections') as max_connections,
                (SELECT count(*) FROM pg_stat_activity WHERE state = 'active') as active,
                (SELECT count(*) FROM pg_stat_activity WHERE state = 'idle') as idle,
                (SELECT count(*) FROM pg_stat_activity WHERE wait_event_type = 'Lock') as waiting";

            match conn.client.query_one(conn_query, &[]).await {
                Ok(row) => {
                    let total: i64 = row.get(0);
                    let max: i32 = row.get(1);
                    let active: i64 = row.get(2);
                    let idle: i64 = row.get(3);
                    let waiting: i64 = row.get(4);
                    let usage_pct = (total as f64 / max as f64) * 100.0;

                    let status = if usage_pct < 70.0 { "✅" } else if usage_pct < 90.0 { "⚠️" } else { "❌" };
                    health_report.push_str(&format!("   Total: {}/{} ({:.1}%) {}\n", total, max, usage_pct, status));
                    health_report.push_str(&format!("   Active: {} | Idle: {} | Waiting: {}\n", active, idle, waiting));
                }
                Err(e) => health_report.push_str(&format!("   ❌ Error: {}\n", e)),
            }

            // 3. Unused Indexes
            health_report.push_str("\n📊 3. Unused Indexes\n");
            health_report.push_str(&"-".repeat(40));
            health_report.push('\n');

            let unused_idx_query = "SELECT
                schemaname || '.' || relname as table_name,
                indexrelname as index_name,
                pg_size_pretty(pg_relation_size(i.indexrelid)) as index_size
                FROM pg_stat_user_indexes ui
                JOIN pg_index i ON ui.indexrelid = i.indexrelid
                WHERE NOT i.indisunique AND ui.idx_scan = 0
                AND pg_relation_size(i.indexrelid) > 8192
                ORDER BY pg_relation_size(i.indexrelid) DESC
                LIMIT 5";

            match conn.client.query(unused_idx_query, &[]).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        health_report.push_str("   ✅ No unused indexes found\n");
                    } else {
                        health_report.push_str(&format!("   ⚠️ Found {} unused indexes:\n", rows.len()));
                        for row in &rows {
                            let table: String = row.get(0);
                            let index: String = row.get(1);
                            let size: String = row.get(2);
                            health_report.push_str(&format!("   • {} on {} ({})\n", index, table, size));
                        }
                    }
                }
                Err(e) => health_report.push_str(&format!("   ❌ Error: {}\n", e)),
            }

            // 4. Duplicate Indexes
            health_report.push_str("\n📊 4. Duplicate Indexes\n");
            health_report.push_str(&"-".repeat(40));
            health_report.push('\n');

            let dup_idx_query = "SELECT
                pg_size_pretty(sum(pg_relation_size(idx))::bigint) as size,
                array_agg(idx) as indexes
                FROM (
                    SELECT indexrelid::regclass as idx,
                           (indrelid::text || E'\n' || indclass::text || E'\n' ||
                            indkey::text || E'\n' || coalesce(indexprs::text,'') ||
                            E'\n' || coalesce(indpred::text,'')) as key
                    FROM pg_index
                ) sub
                GROUP BY key HAVING count(*) > 1
                LIMIT 5";

            match conn.client.query(dup_idx_query, &[]).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        health_report.push_str("   ✅ No duplicate indexes found\n");
                    } else {
                        health_report.push_str(&format!("   ⚠️ Found {} duplicate index groups:\n", rows.len()));
                        for row in &rows {
                            let size: String = row.get(0);
                            let indexes: Vec<String> = row.get(1);
                            health_report.push_str(&format!("   • {} - {}\n", indexes.join(", "), size));
                        }
                    }
                }
                Err(e) => health_report.push_str(&format!("   ❌ Error: {}\n", e)),
            }

            // 5. Tables needing vacuum
            health_report.push_str("\n📊 5. Vacuum Health\n");
            health_report.push_str(&"-".repeat(40));
            health_report.push('\n');

            let vacuum_query = "SELECT
                schemaname || '.' || relname as table_name,
                n_dead_tup as dead_tuples,
                ROUND(100.0 * n_dead_tup / nullif(n_live_tup + n_dead_tup, 0), 2) as dead_pct,
                last_autovacuum
                FROM pg_stat_user_tables
                WHERE n_dead_tup > 1000
                ORDER BY n_dead_tup DESC
                LIMIT 5";

            match conn.client.query(vacuum_query, &[]).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        health_report.push_str("   ✅ All tables are well vacuumed\n");
                    } else {
                        health_report.push_str("   ⚠️ Tables needing vacuum:\n");
                        for row in &rows {
                            let table: String = row.get(0);
                            let dead: i64 = row.get(1);
                            let pct: Option<rust_decimal::Decimal> = row.get(2);
                            health_report.push_str(&format!(
                                "   • {} - {} dead tuples ({:.1}%)\n",
                                table, dead, pct.unwrap_or_default()
                            ));
                        }
                    }
                }
                Err(e) => health_report.push_str(&format!("   ❌ Error: {}\n", e)),
            }

            // 6. Sequence Health
            health_report.push_str("\n📊 6. Sequence Health\n");
            health_report.push_str(&"-".repeat(40));
            health_report.push('\n');

            let seq_query = "SELECT
                schemaname || '.' || sequencename as sequence_name,
                last_value,
                max_value,
                ROUND(100.0 * last_value / max_value, 2) as usage_pct
                FROM pg_sequences
                WHERE last_value IS NOT NULL
                AND ROUND(100.0 * last_value / max_value, 2) > 50
                ORDER BY usage_pct DESC
                LIMIT 5";

            match conn.client.query(seq_query, &[]).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        health_report.push_str("   ✅ All sequences are healthy\n");
                    } else {
                        health_report.push_str("   ⚠️ Sequences approaching limit:\n");
                        for row in &rows {
                            let seq: String = row.get(0);
                            let last: i64 = row.get(1);
                            let max: i64 = row.get(2);
                            let pct: rust_decimal::Decimal = row.get(3);
                            health_report.push_str(&format!(
                                "   • {} - {}/{} ({}%)\n",
                                seq, last, max, pct
                            ));
                        }
                    }
                }
                Err(e) => health_report.push_str(&format!("   ❌ Error: {}\n", e)),
            }

            // 7. Database Size
            health_report.push_str("\n📊 7. Database Size\n");
            health_report.push_str(&"-".repeat(40));
            health_report.push('\n');

            let size_query = "SELECT
                current_database() as db_name,
                pg_size_pretty(pg_database_size(current_database())) as db_size";

            match conn.client.query_one(size_query, &[]).await {
                Ok(row) => {
                    let db_name: String = row.get(0);
                    let db_size: String = row.get(1);
                    health_report.push_str(&format!("   Database: {} - Size: {}\n", db_name, db_size));
                }
                Err(e) => health_report.push_str(&format!("   ❌ Error: {}\n", e)),
            }

            health_report.push_str(&"\n");
            health_report.push_str(&"=".repeat(80));
            health_report.push_str("\n✅ Health check completed!\n");

            Ok(format_result_text(&health_report))
        });

        result
    }
}

// ============================================================================
// ExecuteQueryTool
// ============================================================================

#[derive(Deserialize)]
struct ExecuteQueryParams {
    query: String,
    limit: Option<i64>,
}

pub struct ExecuteQueryTool;

impl ExecuteQueryTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExecuteQueryTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ExecuteQueryTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "execute_query".to_string(),
            description: "Thực thi SQL query (chỉ SELECT)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "SQL query để thực thi"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Số lượng record tối đa",
                        "default": 100
                    }
                },
                "required": ["query"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let query_params: ExecuteQueryParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid parameters: {}", e))?;
        let mut query = query_params.query.trim().to_string();
        let limit = query_params.limit.unwrap_or(100);

        // Security check: Only allow SELECT queries
        if !query.to_lowercase().starts_with("select") {
            return Ok(format_result_text(
                "❌ Chỉ cho phép SELECT query để đảm bảo an toàn",
            ));
        }

        // Add LIMIT if not present
        if !query.to_lowercase().contains("limit") {
            query = format!("{} LIMIT {}", query.trim_end_matches(';'), limit);
        }

        let conn_guard = PG_CONNECTION.lock().unwrap();
        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => return Ok(format_result_text("❌ Chưa kết nối đến PostgreSQL")),
        };

        let result = RUNTIME.block_on(async {
            match conn.client.query(&query, &[]).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        return Ok(format_result_text("📊 Query không trả về dữ liệu"));
                    }

                    // Get column names from the first row
                    let columns: Vec<String> = rows[0]
                        .columns()
                        .iter()
                        .map(|c| c.name().to_string())
                        .collect();

                    let mut data_text = "📊 Kết quả query:\n\n".to_string();
                    data_text.push_str(&format!("Columns: {}\n", columns.join(" | ")));
                    data_text.push_str(&"-".repeat(80));
                    data_text.push('\n');

                    for row in rows.iter().take(10) {
                        let row_values: Vec<String> = (0..columns.len())
                            .map(|i| format_column_value(row, i))
                            .collect();
                        data_text.push_str(&row_values.join(" | "));
                        data_text.push('\n');
                    }

                    if rows.len() > 10 {
                        data_text.push_str(&format!("\n... và {} dòng khác", rows.len() - 10));
                    }

                    Ok(format_result_text(&data_text))
                }
                Err(e) => Ok(format_result_text(&format!("❌ Lỗi: {}", e))),
            }
        });

        result
    }
}
