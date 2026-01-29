# Apex Benchmark Scripts

Benchmark scripts để so sánh Apex với Traefik.

## Prerequisites

- [wrk](https://github.com/wg/wrk) hoặc [wrk2](https://github.com/giltene/wrk2)
- [hey](https://github.com/rakyll/hey) (optional)
- Traefik binary

## Quick Start

### 1. Build Echo Server

```bash
cd benches
cargo build --release
```

### 2. Start Backend Servers

```bash
# Terminal 1
./target/release/echo-server 9001

# Terminal 2
./target/release/echo-server 9002
```

### 3. Start Apex

```bash
# Terminal 3
cargo run --release -- --config apex.toml
```

### 4. Run Benchmark

```bash
# Basic benchmark
wrk -t4 -c100 -d30s http://localhost:8080/

# With latency stats
wrk -t4 -c100 -d30s --latency http://localhost:8080/

# High concurrency
wrk -t8 -c400 -d60s --latency http://localhost:8080/
```

## Traefik Comparison

### 1. Configure Traefik

Create `traefik.yml`:

```yaml
entryPoints:
  web:
    address: ":8081"

providers:
  file:
    filename: "traefik-routes.yml"
```

Create `traefik-routes.yml`:

```yaml
http:
  routers:
    api:
      rule: "PathPrefix(`/`)"
      service: backend

  services:
    backend:
      loadBalancer:
        servers:
          - url: "http://127.0.0.1:9001"
          - url: "http://127.0.0.1:9002"
```

### 2. Start Traefik

```bash
traefik --configFile=traefik.yml
```

### 3. Benchmark Traefik

```bash
wrk -t4 -c100 -d30s --latency http://localhost:8081/
```

## Expected Results

| Metric | Traefik | Apex Target |
|--------|---------|-------------|
| RPS | ~50k | 100k+ |
| P50 | ~2ms | <500μs |
| P99 | ~10ms | <2ms |

## Scripts

### bench.ps1 (PowerShell)

```powershell
# Run full benchmark suite
.\scripts\bench.ps1
```

### bench.sh (Bash)

```bash
./scripts/bench.sh
```
