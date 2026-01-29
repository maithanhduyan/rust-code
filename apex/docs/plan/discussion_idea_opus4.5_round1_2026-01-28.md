Research nginx/traefik architecture

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/core)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/http)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/event)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/internal)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/docs/plan)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/docs/proposal)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/core/nginx.c#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/core/ngx_cycle.h)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/core/ngx_connection.h)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/core/ngx_module.h#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/event/ngx_event.h#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/event/ngx_event.c#1-1), lines 1 to 300

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/http/ngx_http.c#1-1), lines 1 to 300

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/http/ngx_http_request.h#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/provider)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/tls)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/server)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/middlewares)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/cmd/traefik/traefik.go#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/cmd/traefik/traefik.go#200-200), lines 200 to 400

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/server/server.go)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/provider/provider.go)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/tls/tlsmanager.go#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/provider/acme)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/server/configurationwatcher.go#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/server/routerfactory.go)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/middlewares/middleware.go)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/provider/acme/provider.go#1-1), lines 1 to 250

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/provider/aggregator/aggregator.go#1-1), lines 1 to 150

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/http/ngx_http_core_module.h#1-1), lines 1 to 250

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/http/ngx_http_upstream.h#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/event/modules)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/http/ngx_http_request.c#1-1), lines 1 to 150

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/http/ngx_http_core_module.c#1-1), lines 1 to 300

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/docs/IDEA.md#1-1), lines 1 to 100

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/docs/IDEA.md#100-100), lines 100 to 211

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/nginx/src/event/modules/ngx_epoll_module.c#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/server/router)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/server/router/router.go#1-1), lines 1 to 150

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/server/server_entrypoint_tcp.go#1-1), lines 1 to 200

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/server/router/router.go#150-150), lines 150 to 350

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/config)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/config/dynamic/config.go)

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/middlewares/retry/retry.go#1-1), lines 1 to 100

Read [](file:///c%3A/Users/tiach/Downloads/proxy-now/traefik/pkg/middlewares/ratelimiter/rate_limiter.go#1-1), lines 1 to 100

## Plan: Rust Reverse Proxy - Kiến trúc hoàn chỉnh

**TL;DR**: Xây dựng reverse proxy Rust kết hợp mô hình event-loop hiệu năng cao của nginx (epoll/io_uring) với hệ thống auto-TLS động của Traefik. Rust đảm bảo memory safety không cần GC, async native với Tokio, và hot reload config không downtime.

---

### Kiến trúc đề xuất

```
┌─────────────────────────────────────────────────────────────────┐
│                         Entry Points                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                       │
│  │ :80 HTTP │  │:443 HTTPS│  │ :8080 TCP│                       │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                       │
│       │             │             │                              │
├───────┼─────────────┼─────────────┼──────────────────────────────┤
│       ▼             ▼             ▼                              │
│  ┌─────────────────────────────────────┐    ┌──────────────────┐│
│  │         TLS Manager (rustls)        │◄───│  ACME Resolver   ││
│  │  - SNI-based cert selection         │    │  - HTTP-01       ││
│  │  - Dynamic cert reload              │    │  - TLS-ALPN-01   ││
│  └─────────────────┬───────────────────┘    │  - DNS-01        ││
│                    │                        └──────────────────┘│
├────────────────────┼─────────────────────────────────────────────┤
│                    ▼                                             │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │              Router (rule-based matching)                    ││
│  │  - Host matching    - Path prefix/regex                      ││
│  │  - Header matching  - Priority-based selection               ││
│  └─────────────────────────┬───────────────────────────────────┘│
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │           Middleware Chain (chain-of-responsibility)         ││
│  │  ┌────────┐ ┌──────────┐ ┌───────┐ ┌────────┐ ┌───────────┐ ││
│  │  │RateLimit│→│Retry     │→│Auth   │→│Headers │→│Compress   │ ││
│  │  └────────┘ └──────────┘ └───────┘ └────────┘ └───────────┘ ││
│  └─────────────────────────┬───────────────────────────────────┘│
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │              Load Balancer                                   ││
│  │  - Round Robin   - Weighted RR   - Least Connections         ││
│  │  - Health Check  - Circuit Breaker                           ││
│  └─────────────────────────┬───────────────────────────────────┘│
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │              Backend Pool (hyper client)                     ││
│  │  - Connection pooling   - Keep-alive                         ││
│  │  - Streaming body       - Zero-copy với io_uring             ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Cross-cutting Concerns                        │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │Config Watcher│  │ Metrics      │  │ Provider System        │ │
│  │(hot reload)  │  │ (Prometheus) │  │ (File, Docker, K8s)    │ │
│  └──────────────┘  └──────────────┘  └────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

---

### Phân tích Hiệu suất

| Yếu tố | Nginx (C) | Traefik (Go) | Rust Proxy | Ghi chú |
|--------|-----------|--------------|------------|---------|
| **Event model** | epoll/kqueue | goroutines + netpoller | tokio + io_uring | io_uring giảm syscall 50%+ |
| **Memory** | Pool allocator | GC (~10ms pause) | No GC, zero-cost abstractions | Rust thắng về latency P99 |
| **TLS** | OpenSSL (C) | Go crypto | rustls (memory-safe) | rustls nhanh hơn OpenSSL 10-20% |
| **Connection pool** | Pre-allocated | Runtime allocation | Pre-allocated + Arc | Tương đương nginx |
| **HTTP parsing** | Hand-optimized | stdlib | httparse (SIMD-enabled) | Có thể nhanh hơn nginx |

**Bottleneck tiềm năng:**
- `Arc<RwLock<Config>>` cho hot reload → Dùng `arc-swap` để lock-free
- Certificate lookup mỗi request → Cache với `DashMap` hoặc `moka`

---

### Phân tích An toàn (Safety)

| Loại lỗi | Nginx risk | Traefik risk | Rust protection |
|----------|------------|--------------|-----------------|
| Buffer overflow | Cao (CVE history) | Thấp (Go bounds check) | **Không thể** (compile-time) |
| Use-after-free | Cao | N/A (GC) | **Không thể** (ownership) |
| Data race | Có thể | Có thể | **Compile error** (Send/Sync) |
| Null pointer | Có thể | Nil panic | **Option<T>** bắt buộc handle |
| Integer overflow | Silent | Panic | **Checked/wrapping** configurable |

**Safety patterns cần áp dụng:**
- `#![forbid(unsafe_code)]` ở application layer
- Unsafe chỉ trong low-level I/O với audit
- Fuzzing với `cargo-fuzz` cho HTTP parser

---

### Steps

1. **Core Runtime**: Implement Tokio-based event loop với worker-per-core model, connection pool pre-allocated như ngx_cycle.h, sử dụng `io_uring` cho Linux modern

2. **TLS Manager**: Xây dựng dynamic cert resolver với `rustls::ResolvesServerCert`, tích hợp ACME client hỗ trợ HTTP-01/TLS-ALPN-01/DNS-01, cert storage với `arc-swap` cho lock-free hot reload

3. **Router & Middleware**: Implement rule-based router với priority matching (tham khảo router), middleware chain pattern với Tower service

4. **Provider System**: File watcher với `notify` crate, Docker provider via API socket, atomic config swap với `Arc::swap`

5. **HTTP Proxy Core**: Dùng `hyper` với connection pooling, streaming body không buffer, header rewriting middleware

6. **Observability**: Prometheus metrics endpoint, structured logging với `tracing`, distributed tracing với OpenTelemetry

---

### Further Considerations

1. **io_uring vs epoll**: io_uring giảm syscall đáng kể nhưng chỉ Linux 5.1+. Nên abstract như nginx's `ngx_event_actions_t` để hỗ trợ cả hai?

2. **Config format**: TOML (đơn giản) vs YAML (Traefik-compatible) vs CUE (type-safe)? Recommend TOML cho MVP, hỗ trợ YAML sau.

3. **Plugin system**: WASM plugins (portable) vs native Rust plugins (performance)? WASM sandbox tốt cho security nhưng overhead ~10%.