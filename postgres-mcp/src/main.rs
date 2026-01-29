use postgres_mcp::prelude::*;
use postgres_mcp::tools::{
    AnalyzeDbHealthTool, ConnectPostgresTool, CountDatabasesTool, ExecuteQueryTool,
    ExplainQueryTool, GetTopQueriesTool, ListDatabasesTool, ListSchemasTool, ListTablesTool,
    TableDataTool, TableStructureTool,
};

fn main() -> McpResult<()> {
    let server_info = ServerInfo {
        name: "postgres-mcp".to_string(),
        version: "1.0.0".to_string(),
        description: Some("PostgreSQL MCP Server - Database operations via MCP".to_string()),
    };
    let mut server = McpServer::with_info(server_info);

    // Register all PostgreSQL tools
    server.register_tool(Box::new(ConnectPostgresTool::new())); // 1
    server.register_tool(Box::new(CountDatabasesTool::new())); // 2
    server.register_tool(Box::new(ListDatabasesTool::new())); // 3
    server.register_tool(Box::new(ListSchemasTool::new())); // 4
    server.register_tool(Box::new(ListTablesTool::new())); // 5
    server.register_tool(Box::new(TableStructureTool::new())); // 6
    server.register_tool(Box::new(TableDataTool::new())); // 7
    server.register_tool(Box::new(ExecuteQueryTool::new())); // 8
    server.register_tool(Box::new(ExplainQueryTool::new())); // 9
    server.register_tool(Box::new(GetTopQueriesTool::new())); // 10
    server.register_tool(Box::new(AnalyzeDbHealthTool::new())); // 11

    server.run()
}
