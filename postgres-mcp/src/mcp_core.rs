// MCP core protocol, server, trait, error
// JSON-RPC 2.0 implementation for MCP

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

pub type McpResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Deserialize, Debug, Clone)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub result: serde_json::Value,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcError {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub error: ErrorObject,
}

#[derive(Serialize, Debug)]
pub struct ErrorObject {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Serialize, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

pub struct McpServer {
    reader: BufReader<io::Stdin>,
    writer: BufWriter<io::Stdout>,
    tools: HashMap<String, Box<dyn crate::tools::Tool>>,
    server_info: ServerInfo,
}

#[derive(Serialize, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            name: "postgres-mcp".to_string(),
            version: "1.0.0".to_string(),
            description: Some("PostgreSQL MCP Server".to_string()),
        }
    }
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(io::stdin()),
            writer: BufWriter::new(io::stdout()),
            tools: HashMap::new(),
            server_info: ServerInfo::default(),
        }
    }

    pub fn with_info(server_info: ServerInfo) -> Self {
        Self {
            reader: BufReader::new(io::stdin()),
            writer: BufWriter::new(io::stdout()),
            tools: HashMap::new(),
            server_info,
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn crate::tools::Tool>) -> &mut Self {
        let name = tool.definition().name.clone();
        self.tools.insert(name, tool);
        self
    }

    pub fn run(&mut self) -> McpResult<()> {
        let mut line = String::new();
        while self.reader.read_line(&mut line)? > 0 {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                self.handle_request(trimmed)?;
            }
            line.clear();
        }
        Ok(())
    }

    fn handle_request(&mut self, request_str: &str) -> McpResult<()> {
        let request: JsonRpcRequest = match serde_json::from_str(request_str) {
            Ok(req) => req,
            Err(e) => {
                self.send_error_response(
                    serde_json::Value::Null,
                    -32700,
                    "Parse error",
                    Some(serde_json::json!({"details": e.to_string()})),
                )?;
                return Ok(());
            }
        };
        if request.jsonrpc != "2.0" {
            self.send_error_response(
                request.id.unwrap_or(serde_json::Value::Null),
                -32600,
                "Invalid Request: jsonrpc must be '2.0'",
                None,
            )?;
            return Ok(());
        }
        let request_id = request.id.unwrap_or(serde_json::Value::Null);
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request_id),
            "tools/list" => self.handle_tools_list(request_id),
            "tools/call" => self.handle_tool_call(request_id, request.params),
            _ => self.send_error_response(request_id, -32601, "Method not found", None),
        }
    }

    fn handle_initialize(&mut self, id: serde_json::Value) -> McpResult<()> {
        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": self.server_info.name,
                "version": self.server_info.version,
                "description": self.server_info.description
            }
        });
        self.send_success_response(id, result)
    }

    fn handle_tools_list(&mut self, id: serde_json::Value) -> McpResult<()> {
        let tools: Vec<McpTool> = self.tools.values().map(|tool| tool.definition()).collect();
        let result = serde_json::json!({ "tools": tools });
        self.send_success_response(id, result)
    }

    fn handle_tool_call(
        &mut self,
        id: serde_json::Value,
        params: Option<serde_json::Value>,
    ) -> McpResult<()> {
        let params = params.ok_or("Missing parameters")?;
        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing tool name")?;
        let tool = match self.tools.get(tool_name) {
            Some(tool) => tool,
            None => {
                self.send_error_response(
                    id,
                    -32602,
                    "Unknown tool",
                    Some(serde_json::json!({"tool": tool_name})),
                )?;
                return Ok(());
            }
        };
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
        match tool.execute(arguments) {
            Ok(result) => self.send_success_response(id, result),
            Err(e) => self.send_error_response(
                id,
                -32603,
                "Tool execution failed",
                Some(serde_json::json!({"error": e.to_string()})),
            ),
        }
    }

    fn send_success_response(
        &mut self,
        id: serde_json::Value,
        result: serde_json::Value,
    ) -> McpResult<()> {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result,
        };
        let response_str = serde_json::to_string(&response)?;
        writeln!(self.writer, "{}", response_str)?;
        self.writer.flush()?;
        Ok(())
    }

    fn send_error_response(
        &mut self,
        id: serde_json::Value,
        code: i32,
        message: &str,
        data: Option<serde_json::Value>,
    ) -> McpResult<()> {
        let error_response = JsonRpcError {
            jsonrpc: "2.0".to_string(),
            id,
            error: ErrorObject {
                code,
                message: message.to_string(),
                data,
            },
        };
        let response_str = serde_json::to_string(&error_response)?;
        writeln!(self.writer, "{}", response_str)?;
        self.writer.flush()?;
        Ok(())
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
