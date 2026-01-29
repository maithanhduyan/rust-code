use clap::Parser;
use postgres_mcp::mcp_core::{McpSseServer, TransportMode};
use postgres_mcp::prelude::*;
use postgres_mcp::tools::{
    AnalyzeDbHealthTool, ConnectPostgresTool, CountDatabasesTool, ExecuteQueryTool,
    ExecuteSqlTool, ExplainQueryTool, GetAccessModeTool, GetTopQueriesTool, ListDatabasesTool,
    ListSchemasTool, ListTablesTool, SetAccessModeTool, TableDataTool, TableStructureTool,
};

/// PostgreSQL MCP Server - Database operations with access modes
#[derive(Parser, Debug)]
#[command(name = "postgres-mcp")]
#[command(version = "1.2.0")]
#[command(about = "PostgreSQL MCP Server with STDIO and SSE transport support")]
struct Args {
    /// Transport mode: stdio or sse
    #[arg(short, long, default_value = "stdio")]
    transport: String,

    /// Port for SSE server (only used with --transport sse)
    #[arg(short, long, default_value = "8000")]
    port: u16,
}

fn get_transport_mode(mode: &str) -> TransportMode {
    match mode.to_lowercase().as_str() {
        "sse" | "http" => TransportMode::Sse,
        _ => TransportMode::Stdio,
    }
}

fn main() -> McpResult<()> {
    let args = Args::parse();
    let transport_mode = get_transport_mode(&args.transport);

    let server_info = ServerInfo {
        name: "postgres-mcp".to_string(),
        version: "1.2.0".to_string(),
        description: Some("PostgreSQL MCP Server - Database operations with access modes".to_string()),
    };

    match transport_mode {
        TransportMode::Stdio => {
            // STDIO transport (original behavior)
            let mut server = McpServer::with_info(server_info);
            register_tools_stdio(&mut server);
            server.run()
        }
        TransportMode::Sse => {
            // SSE transport (HTTP server)
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let mut server = McpSseServer::with_info(server_info, args.port);
                register_tools_sse(&mut server);
                server.run().await
            })
        }
    }
}

fn register_tools_stdio(server: &mut McpServer) {
    server.register_tool(Box::new(ConnectPostgresTool::new())); // 1
    server.register_tool(Box::new(CountDatabasesTool::new())); // 2
    server.register_tool(Box::new(ListDatabasesTool::new())); // 3
    server.register_tool(Box::new(ListSchemasTool::new())); // 4
    server.register_tool(Box::new(ListTablesTool::new())); // 5
    server.register_tool(Box::new(TableStructureTool::new())); // 6
    server.register_tool(Box::new(TableDataTool::new())); // 7
    server.register_tool(Box::new(ExecuteQueryTool::new())); // 8 - Legacy (SELECT only)
    server.register_tool(Box::new(ExplainQueryTool::new())); // 9
    server.register_tool(Box::new(GetTopQueriesTool::new())); // 10
    server.register_tool(Box::new(AnalyzeDbHealthTool::new())); // 11
    server.register_tool(Box::new(SetAccessModeTool::new())); // 12 - Access mode control
    server.register_tool(Box::new(GetAccessModeTool::new())); // 13 - Get current mode
    server.register_tool(Box::new(ExecuteSqlTool::new())); // 14 - Full SQL with access mode
}

fn register_tools_sse(server: &mut McpSseServer) {
    server.register_tool(Box::new(ConnectPostgresTool::new())); // 1
    server.register_tool(Box::new(CountDatabasesTool::new())); // 2
    server.register_tool(Box::new(ListDatabasesTool::new())); // 3
    server.register_tool(Box::new(ListSchemasTool::new())); // 4
    server.register_tool(Box::new(ListTablesTool::new())); // 5
    server.register_tool(Box::new(TableStructureTool::new())); // 6
    server.register_tool(Box::new(TableDataTool::new())); // 7
    server.register_tool(Box::new(ExecuteQueryTool::new())); // 8 - Legacy (SELECT only)
    server.register_tool(Box::new(ExplainQueryTool::new())); // 9
    server.register_tool(Box::new(GetTopQueriesTool::new())); // 10
    server.register_tool(Box::new(AnalyzeDbHealthTool::new())); // 11
    server.register_tool(Box::new(SetAccessModeTool::new())); // 12 - Access mode control
    server.register_tool(Box::new(GetAccessModeTool::new())); // 13 - Get current mode
    server.register_tool(Box::new(ExecuteSqlTool::new())); // 14 - Full SQL with access mode
}
