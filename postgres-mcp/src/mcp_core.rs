// MCP core protocol, server, trait, error
// JSON-RPC 2.0 implementation for MCP with STDIO and SSE transport support

use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{Any, CorsLayer};

pub type McpResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Deserialize, Debug, Clone)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Serialize, Debug, Clone)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub result: serde_json::Value,
}

#[derive(Serialize, Debug, Clone)]
pub struct JsonRpcError {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub error: ErrorObject,
}

#[derive(Serialize, Debug, Clone)]
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
            version: "1.1.0".to_string(),
            description: Some("PostgreSQL MCP Server with SSE support".to_string()),
        }
    }
}

// Transport mode
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransportMode {
    Stdio,
    Sse,
}

// Shared state for SSE server
pub struct McpServerState {
    pub tools: HashMap<String, Arc<dyn crate::tools::Tool>>,
    pub server_info: ServerInfo,
    pub tx: broadcast::Sender<String>,
}

// STDIO-based MCP Server (original)
pub struct McpServer {
    reader: BufReader<io::Stdin>,
    writer: BufWriter<io::Stdout>,
    tools: HashMap<String, Box<dyn crate::tools::Tool>>,
    server_info: ServerInfo,
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

// ============================================================================
// SSE Transport Implementation
// ============================================================================

pub struct McpSseServer {
    tools: HashMap<String, Arc<dyn crate::tools::Tool>>,
    server_info: ServerInfo,
    port: u16,
}

impl McpSseServer {
    pub fn new(port: u16) -> Self {
        Self {
            tools: HashMap::new(),
            server_info: ServerInfo::default(),
            port,
        }
    }

    pub fn with_info(server_info: ServerInfo, port: u16) -> Self {
        Self {
            tools: HashMap::new(),
            server_info,
            port,
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn crate::tools::Tool>) -> &mut Self {
        let name = tool.definition().name.clone();
        // Convert Box to Arc for sharing across threads
        let arc_tool: Arc<dyn crate::tools::Tool> = Arc::from(tool);
        self.tools.insert(name, arc_tool);
        self
    }

    pub async fn run(self) -> McpResult<()> {
        let (tx, _rx) = broadcast::channel::<String>(100);

        let state = Arc::new(RwLock::new(McpServerState {
            tools: self.tools,
            server_info: self.server_info.clone(),
            tx: tx.clone(),
        }));

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/", get(root_handler))
            .route("/sse", get(sse_handler))
            .route("/message", post(message_handler))
            .with_state(state)
            .layer(cors);

        let addr = format!("0.0.0.0:{}", self.port);
        eprintln!("🚀 PostgreSQL MCP Server (SSE) starting on http://{}", addr);
        eprintln!("   SSE endpoint: http://localhost:{}/sse", self.port);
        eprintln!("   Message endpoint: http://localhost:{}/message", self.port);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

// Root handler - server info
async fn root_handler(State(state): State<Arc<RwLock<McpServerState>>>) -> impl IntoResponse {
    let state = state.read().await;
    Json(serde_json::json!({
        "name": state.server_info.name,
        "version": state.server_info.version,
        "description": state.server_info.description,
        "transport": "sse",
        "endpoints": {
            "sse": "/sse",
            "message": "/message"
        }
    }))
}

// SSE handler - client connects here to receive responses
async fn sse_handler(
    State(state): State<Arc<RwLock<McpServerState>>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let state = state.read().await;
    let rx = state.tx.subscribe();

    let stream = BroadcastStream::new(rx).map(|result| {
        match result {
            Ok(msg) => Ok(Event::default().data(msg)),
            Err(_) => Ok(Event::default().data("{\"error\": \"stream error\"}")),
        }
    });

    Sse::new(stream)
}

// Message handler - client sends JSON-RPC requests here
async fn message_handler(
    State(state): State<Arc<RwLock<McpServerState>>>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        let error = JsonRpcError {
            jsonrpc: "2.0".to_string(),
            id: request.id.unwrap_or(serde_json::Value::Null),
            error: ErrorObject {
                code: -32600,
                message: "Invalid Request: jsonrpc must be '2.0'".to_string(),
                data: None,
            },
        };
        return (StatusCode::BAD_REQUEST, Json(serde_json::to_value(error).unwrap()));
    }

    let request_id = request.id.clone().unwrap_or(serde_json::Value::Null);

    let response = match request.method.as_str() {
        "initialize" => {
            let state = state.read().await;
            let result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": state.server_info.name,
                    "version": state.server_info.version,
                    "description": state.server_info.description
                }
            });
            serde_json::json!(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id,
                result,
            })
        }
        "tools/list" => {
            let state = state.read().await;
            let tools: Vec<McpTool> = state.tools.values().map(|t| t.definition()).collect();
            let result = serde_json::json!({ "tools": tools });
            serde_json::json!(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id,
                result,
            })
        }
        "tools/call" => {
            let params = request.params.clone().unwrap_or(serde_json::Value::Null);
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

            // Get tool from state
            let tool_opt = {
                let state = state.read().await;
                state.tools.get(&tool_name).cloned()
            };

            if let Some(tool) = tool_opt {
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                // Spawn blocking task for tool execution (tools may use their own runtime)
                let id_clone = request_id.clone();
                let result = tokio::task::spawn_blocking(move || {
                    tool.execute(arguments)
                }).await;

                match result {
                    Ok(Ok(tool_result)) => {
                        serde_json::json!(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: id_clone,
                            result: tool_result,
                        })
                    }
                    Ok(Err(e)) => {
                        serde_json::json!(JsonRpcError {
                            jsonrpc: "2.0".to_string(),
                            id: id_clone,
                            error: ErrorObject {
                                code: -32603,
                                message: "Tool execution failed".to_string(),
                                data: Some(serde_json::json!({"error": e.to_string()})),
                            },
                        })
                    }
                    Err(e) => {
                        serde_json::json!(JsonRpcError {
                            jsonrpc: "2.0".to_string(),
                            id: id_clone,
                            error: ErrorObject {
                                code: -32603,
                                message: "Tool execution panicked".to_string(),
                                data: Some(serde_json::json!({"error": e.to_string()})),
                            },
                        })
                    }
                }
            } else {
                serde_json::json!(JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    id: request_id,
                    error: ErrorObject {
                        code: -32602,
                        message: "Unknown tool".to_string(),
                        data: Some(serde_json::json!({"tool": tool_name})),
                    },
                })
            }
        }
        _ => {
            serde_json::json!(JsonRpcError {
                jsonrpc: "2.0".to_string(),
                id: request_id,
                error: ErrorObject {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: None,
                },
            })
        }
    };

    // Also broadcast to SSE clients
    let state = state.read().await;
    let response_str = serde_json::to_string(&response).unwrap_or_default();
    let _ = state.tx.send(response_str);

    (StatusCode::OK, Json(response))
}
