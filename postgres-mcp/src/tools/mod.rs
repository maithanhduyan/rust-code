// PostgreSQL tools module
pub mod postgres;

pub use postgres::*;

use crate::mcp_core::{McpResult, McpTool};

/// Trait for implementing MCP tools
pub trait Tool: Send + Sync {
    /// Returns the tool definition
    fn definition(&self) -> McpTool;
    /// Executes the tool with given parameters
    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value>;
}
