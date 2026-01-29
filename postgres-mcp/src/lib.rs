/// PostgreSQL MCP Server Library
/// A library for creating MCP servers for PostgreSQL operations
pub mod mcp_core;
pub mod tools;

pub use mcp_core::*;
pub use tools::*;

/// Re-export commonly used types for convenience
pub mod prelude {
    pub use crate::mcp_core::{
        ErrorObject, JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpResult, McpServer, McpTool,
        ServerInfo,
    };
    pub use crate::tools::Tool;
}
