# Proposed: Apex

> **Status:** 📋 Proposed  
> **Date:** 2026-01-28  
> **Version:** 3.0 (Minimalist)

---

## Một câu

**Reverse proxy nhanh hơn Traefik 3x, an toàn hơn nginx, đơn giản hơn Envoy.**

---

## Tại sao tồn tại

| Vấn đề | Giải pháp |
|--------|-----------|
| nginx CVEs từ C | Rust memory safety |
| Traefik chậm do GC + locks | Lock-free, zero-copy |
| Envoy phức tạp | Single binary, simple config |

---

## Luật sắt (3 điều duy nhất)

```
1. KHÔNG Mutex/RwLock trong hot path
2. KHÔNG allocation per-request (trừ arena)  
3. KHÔNG panic trên user input
```

Vi phạm = Bug. Không có ngoại lệ.

---

## Mục tiêu duy nhất Phase 1

**HTTP/1.1 reverse proxy đạt 100k RPS, P99 < 500μs**

Không hơn. Không kém.

---

## Stack (đã quyết định)

```toml
tokio = "1"      # async runtime
hyper = "1"      # HTTP
rustls = "0.23"  # TLS  
arc-swap = "1"   # lock-free config
```

Không thêm dependencies cho đến khi cần.

---

## Cấu trúc (tối giản)

```
crates/
├── apex/          # Binary
├── core/          # Hot path (không deps ngoài)
├── config/        # Config parsing
└── server/        # HTTP server
```

4 crates. Thêm sau khi cần.

---

## Một việc tiếp theo

**Viết benchmark so sánh với Traefik.**

Chưa có baseline = không biết mình đang ở đâu.

```bash
wrk -t4 -c100 -d30s http://localhost:8080/
```

---

## Không làm (cho đến khi Phase 1 xong)

- ❌ HTTP/2, HTTP/3
- ❌ ACME/Auto TLS  
- ❌ Load balancing algorithms
- ❌ Middleware system
- ❌ Metrics/Tracing
- ❌ Docker/K8s providers

Mỗi feature = một cơ hội để fail.

---

## Thành công = 

```
Traefik:  50,000 RPS
Apex:    100,000 RPS  ← Đây
```

Một con số. Đo được. Không thể tranh cãi.

---

## Thất bại =

- Không đạt 100k RPS sau 4 tuần
- Vi phạm 1 trong 3 luật sắt
- Thêm feature trước khi đạt target

---

## Timeline

| Tuần | Việc |
|------|------|
| 1 | Scaffold + Traefik baseline |
| 2 | Basic proxy flow |
| 3 | Optimize hot path |
| 4 | Benchmark + iterate |

4 tuần. Một mục tiêu.

---

*"Perfection is achieved not when there is nothing more to add, but when there is nothing left to take away."*  
— Antoine de Saint-Exupéry

---

*Last updated: 2026-01-28*
