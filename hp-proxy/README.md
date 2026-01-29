# HP Proxy - High-Performance Blocking Reverse Proxy

Ultra-fast HTTP/1.1 reverse proxy written in pure Rust.

## Features

- **SO_REUSEPORT**: Kernel load-balances connections across workers (Linux)
- **Zero-copy parsing**: No intermediate allocations
- **Connection pooling**: Reuses backend connections
- **Cache-line aligned stats**: No false sharing between workers

## Performance Target

| Configuration | Expected RPS |
|---------------|--------------|
| Single worker | 20-30k |
| 8 workers (SO_REUSEPORT) | 120-160k |
| With io_uring (future) | 500k+ |

## Quick Start

```bash
# Build
cargo build --release

# Start echo server (backend)
./target/release/hp-echo &

# Start proxy
./target/release/hp-proxy &

# Benchmark
./target/release/hp-bench 127.0.0.1:8080 100 10
```

## Configuration

Edit constants in `src/main.rs`:

```rust
const BACKEND_ADDR: &str = "127.0.0.1:9001";
const LISTEN_ADDR: &str = "0.0.0.0:8080";
const NUM_WORKERS: usize = 8;
const POOL_SIZE_PER_WORKER: usize = 20;
```

## Benchmarking

### With included benchmark tool

```bash
# 100 concurrent connections, 10 seconds
./target/release/hp-bench 127.0.0.1:8080 100 10

# Direct to echo (baseline)
./target/release/hp-bench 127.0.0.1:9001 100 10
```

### With wrk (recommended)

```bash
wrk -t8 -c100 -d10s --latency http://127.0.0.1:8080/
```

### With hey

```bash
hey -n 100000 -c 100 http://127.0.0.1:8080/
```

## Architecture

```
                    ┌──────────────────┐
                    │   Linux Kernel   │
                    │  (SO_REUSEPORT)  │
                    └────────┬─────────┘
                             │ load balance
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│    Worker 0     │ │    Worker 1     │ │    Worker N     │
│  ┌───────────┐  │ │  ┌───────────┐  │ │  ┌───────────┐  │
│  │ Listener  │  │ │  │ Listener  │  │ │  │ Listener  │  │
│  └─────┬─────┘  │ │  └─────┬─────┘  │ │  └─────┬─────┘  │
│        │        │ │        │        │ │        │        │
│  ┌─────▼─────┐  │ │  ┌─────▼─────┐  │ │  ┌─────▼─────┐  │
│  │   Pool    │  │ │  │   Pool    │  │ │  │   Pool    │  │
│  │ (backend) │  │ │  │ (backend) │  │ │  │ (backend) │  │
│  └───────────┘  │ │  └───────────┘  │ │  └───────────┘  │
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

## Platform Support

| Platform | SO_REUSEPORT | Expected Performance |
|----------|--------------|---------------------|
| Linux 3.9+ | ✅ | Full |
| macOS | ✅ | Full |
| Windows | ❌ (fallback) | Reduced (~50%) |

## License

MIT
