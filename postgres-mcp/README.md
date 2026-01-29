# PostgreSQL MCP Server

A Model Context Protocol (MCP) server for PostgreSQL database operations, written in Rust.

## Features

- **connect_postgres**: Connect to a PostgreSQL server
- **count_databases**: Count the number of databases
- **list_databases**: List all databases
- **list_schemas**: List all schemas in the current database
- **list_tables**: List all tables in a schema
- **table_structure**: Get detailed structure of a table
- **table_data**: Get data from a table
- **execute_query**: Execute SELECT queries safely

## Building

```bash
cargo build --release
```

## Usage

The server communicates via JSON-RPC 2.0 over stdin/stdout.

### Configuration for VS Code MCP

Add to your `mcp.json`:

```json
{
  "servers": {
    "postgres-mcp": {
      "command": "${workspaceFolder}/.tools/postgres-mcp/target/release/postgres-mcp.exe",
      "args": []
    }
  }
}
```

## Protocol

This server implements the Model Context Protocol (MCP) with JSON-RPC 2.0.

### Available Methods

- `initialize`: Initialize the server
- `tools/list`: List available tools
- `tools/call`: Call a specific tool

## License

MIT
