# Apex

**High-performance reverse proxy written in Rust**

> Fast by design. Safe by default.

## Features

- 🚀 **High Performance**: Target 100k+ RPS, P99 < 500μs
- 🔒 **Memory Safe**: Written in Rust, no buffer overflows
- ⚡ **Lock-free**: No mutex contention in hot path
- 🔄 **Hot Reload**: Configuration changes without restart
- 📦 **Simple**: Single binary, TOML configuration

## Quick Start

### Build

```bash
cargo build --release
```

### Configure

Create `apex.toml`:

```toml
[server]
listen = "0.0.0.0:8080"

[[routes]]
name = "my-api"
host = "*"
path_prefix = "/"
backends = [
    { url = "http://127.0.0.1:9001" },
    { url = "http://127.0.0.1:9002" },
]
```

### Run

```bash
./target/release/apex --config apex.toml
```

## Configuration

### Server Options

| Option | Default | Description |
|--------|---------|-------------|
| `listen` | `0.0.0.0:8080` | Server listen address |
| `workers` | `0` (auto) | Number of worker threads |
| `timeout_secs` | `30` | Request timeout |
| `access_log` | `true` | Enable access logging |

### Route Options

| Option | Default | Description |
|--------|---------|-------------|
| `name` | required | Route name for logging |
| `host` | `*` | Host pattern to match |
| `path_prefix` | `/` | Path prefix to match |
| `strip_prefix` | `false` | Remove prefix before forwarding |
| `load_balancing` | `round_robin` | Load balancing strategy |
| `backends` | required | List of backend servers |

### Load Balancing Strategies

- `round_robin` - Distribute requests evenly
- `least_connections` - Send to backend with fewest active connections
- `random` - Random selection

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                         Client                            │
└──────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────┐
│                    Apex Proxy                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│  │   Router    │──│   Handler   │──│   Client    │       │
│  │ (lock-free) │  │   (hyper)   │  │  (pooled)   │       │
│  └─────────────┘  └─────────────┘  └─────────────┘       │
└──────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐    ┌──────────┐    ┌──────────┐
        │ Backend1 │    │ Backend2 │    │ Backend3 │
        └──────────┘    └──────────┘    └──────────┘
```

## Benchmarks

Run benchmarks:

```bash
# Start backends
cargo run --release --bin echo-server -- 9001 &
cargo run --release --bin echo-server -- 9002 &

# Start proxy
cargo run --release -- --config apex.toml

# Benchmark
wrk -t4 -c100 -d30s http://localhost:8080/
```

## Development

### Project Structure

```
crates/
├── apex/          # Binary - CLI and main entry
├── core/          # Core - Lock-free routing, backend pool
├── config/        # Config - TOML parsing, hot reload
└── server/        # Server - HTTP server, proxy logic
```

### Build Commands

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Check config
cargo run -- --config apex.toml --check
```

## License

MIT OR Apache-2.0
