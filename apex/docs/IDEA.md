# 🚀 Apex: The Future of Edge Infrastructure

> *"Không chỉ nhanh hơn, mà an toàn hơn từ trong lõi."*

---

## 🌟 Tầm nhìn (Vision)

**Trở thành reverse proxy tiêu chuẩn cho thế hệ cloud-native tiếp theo.**

Trong 10 năm tới, mọi ứng dụng sẽ:
- Chạy trên edge, không chỉ data center
- Yêu cầu zero-downtime deployments
- Cần bảo mật mặc định, không phải add-on

**Apex** được thiết kế từ đầu để đáp ứng tương lai đó — không phải patch từ công nghệ 20 năm tuổi.

---

## 🎯 Sứ mệnh (Mission)

**Dân chủ hóa hiệu năng cấp Cloudflare cho mọi tổ chức.**

Chúng tôi tin rằng:
- Startup 5 người xứng đáng có infra nhanh như Big Tech
- Bảo mật không nên là luxury feature
- Đơn giản hóa vận hành là tôn trọng thời gian DevOps

### Chúng tôi cam kết:

| Cam kết | Ý nghĩa |
|---------|---------|
| **Performance by Default** | Không cần tuning, out-of-box đã nhanh |
| **Security by Design** | Memory safety, không CVE từ buffer overflow |
| **Zero-Downtime Operations** | Hot reload mọi thứ, không restart |
| **Observable by Nature** | Metrics, tracing built-in |

---

## 💎 Giá trị cốt lõi (Core Values)

### 1. 🔒 **An toàn trên hết (Safety First)**

> *"Mỗi CVE của nginx là lời nhắc tại sao chúng tôi tồn tại."*

- **Memory safety** qua Rust compiler, không phải discipline
- **No unsafe** trong hot path trừ khi có audit
- **Crash = Bug**, không phải "expected behavior"

### 2. ⚡ **Hiệu năng không thỏa hiệp (Uncompromising Performance)**

> *"Nếu Traefik làm được 50k RPS, chúng tôi làm 200k."*

- Lock-free data plane
- Zero-copy request handling  
- P99 latency < 200μs under load

### 3. 🔄 **Đơn giản hóa vận hành (Operational Simplicity)**

> *"Config mới? Reload. Cert mới? Tự động. Downtime? Không tồn tại."*

- Hot reload config trong < 1ms
- Auto TLS với Let's Encrypt
- Single binary, không dependencies

### 4. 🔍 **Minh bạch và Observable (Transparent & Observable)**

> *"Bạn không thể fix những gì bạn không thấy."*

- Prometheus metrics mặc định
- Distributed tracing (OpenTelemetry)
- Structured logging

### 5. 🌱 **Thiết kế cho tương lai (Future-Proof Design)**

> *"Code của hôm nay phải chạy được 10 năm nữa."*

- Stable ABI cho protocol types
- Backward-compatible config
- Modular architecture cho extensions

---

## 🎪 Tại sao không dùng những gì đã có?

### nginx (1999)
- ✅ Proven, stable, fast
- ❌ C code = CVE factory (buffer overflows)
- ❌ Config reload = worker restart
- ❌ Auto TLS = afterthought (certbot external)

### Traefik (2016)
- ✅ Cloud-native, auto everything
- ❌ Go GC = latency spikes
- ❌ Lock contention under load
- ❌ 50-80k RPS ceiling

### Envoy (2016)
- ✅ Feature-rich, extensible
- ❌ C++ = complexity + safety concerns
- ❌ Heavy resource footprint
- ❌ Configuration complexity

### **Apex (2026)**
- ✅ Rust = Performance + Safety
- ✅ Lock-free = Consistent latency
- ✅ 200k+ RPS target
- ✅ Simple config, powerful features

---

## 🏔️ Thách thức chúng tôi chấp nhận

| Thách thức | Cam kết |
|------------|---------|
| "Rust quá mới" | Stable toolchain, MSRV policy |
| "Ecosystem nhỏ" | Contribute back to community |
| "Khó hire" | Đào tạo, documentation xuất sắc |
| "Chưa proven" | Benchmark công khai, production case studies |

---

## 🗺️ Định hướng chiến lược

### Phase 1: Foundation (Q1 2026)
**Mục tiêu:** Chứng minh technical feasibility
- HTTP/1.1 reverse proxy
- Vượt Traefik 1.5x performance
- Core invariants validated

### Phase 2: Feature Parity (Q2 2026)
**Mục tiêu:** Thay thế Traefik cho internal workloads
- Auto TLS (ACME)
- Hot reload
- Load balancing + health checks

### Phase 3: Production Ready (Q3 2026)
**Mục tiêu:** Production deployments
- HTTP/2, gRPC support
- Observability stack
- Docker/K8s providers

### Phase 4: Industry Standard (2027+)
**Mục tiêu:** Trở thành lựa chọn mặc định
- HTTP/3 (QUIC)
- Plugin ecosystem
- Enterprise features

---

## 👥 Dự án này dành cho ai?

### ✅ Phù hợp với:
- **Platform teams** muốn kiểm soát edge infrastructure
- **Startups** cần performance mà không cần dedicated SRE
- **Security-conscious orgs** lo ngại CVE từ C/C++ proxies
- **Rust enthusiasts** muốn contribute vào infra project

### ❌ Không phù hợp với:
- Những ai cần production-ready **ngay hôm nay**
- Projects cần full nginx feature parity
- Organizations không thể accept early-adopter risk

---

## 📐 Đo lường thành công

| Metric | Target 2026 | Target 2027 |
|--------|-------------|-------------|
| Throughput | 200k+ RPS/core | 400k+ RPS/core |
| P99 Latency | < 200μs | < 100μs |
| CVE Count | 0 memory-safety | 0 memory-safety |
| Production Users | 10+ orgs | 100+ orgs |
| GitHub Stars | 1,000+ | 10,000+ |
| Contributors | 5+ | 20+ |

---

## 💬 Tagline Options

1. *"The Rust-powered edge for the cloud-native era"*
2. *"Fast by design. Safe by default."*
3. *"Where performance meets peace of mind"*
4. *"nginx speed, Traefik simplicity, Rust safety"*

---

## 📚 Tài liệu liên quan

| Document | Mô tả |
|----------|-------|
| [Proposed-Apex.md](proposal/Proposed-Apex.md) | Technical proposal chi tiết |
| [AGENT_INSTRUCTIONS.md](../.tools/AGENT_INSTRUCTIONS.md) | Hướng dẫn cho AI agents |
| [Discussion logs](plan/) | Lịch sử thảo luận thiết kế |

---

> *"Chúng tôi không xây dựng một reverse proxy khác.*
> *Chúng tôi xây dựng reverse proxy mà internet xứng đáng được có."*

---

*Version 2.0 — Rewritten 2026-01-28*
