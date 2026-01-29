## Phân tích luồng Proxy: Đếm điểm chạm

### 🔄 Full Request Journey (Client → Proxy → Backend → Proxy → Client)

```
CLIENT                    PROXY                         BACKEND
   │                        │                              │
   │  ┌─────────────────────┼──────────────────────────┐   │
   │  │      INBOUND        │                          │   │
   │  │                     │                          │   │
   ├──┼──① NIC receive ─────┤                          │   │
   │  │                     │                          │   │
   │  │  ② Kernel network ──┤                          │   │
   │  │     stack (TCP/IP)  │                          │   │
   │  │                     │                          │   │
   │  │  ③ Socket buffer ───┤                          │   │
   │  │                     │                          │   │
   │  │  ④ Syscall ─────────┤ (read/epoll)             │   │
   │  │     (context switch)│                          │   │
   │  │                     │                          │   │
   │  │  ⑤ User-space ──────┤                          │   │
   │  │     copy to buffer  │                          │   │
   │  │                     │                          │   │
   │  │  ⑥ TLS decrypt ─────┤ (if HTTPS)               │   │
   │  │                     │                          │   │
   │  │  ⑦ HTTP parse ──────┤                          │   │
   │  │                     │                          │   │
   │  │  ⑧ Route lookup ────┤                          │   │
   │  │                     │                          │   │
   │  │  ⑨ Middleware ──────┤ (auth, rate limit, etc)  │   │
   │  │     chain           │                          │   │
   │  │                     │                          │   │
   │  │  ⑩ Load balance ────┤                          │   │
   │  │     decision        │                          │   │
   │  │                     │                          │   │
   │  └─────────────────────┼──────────────────────────┘   │
   │                        │                              │
   │  ┌─────────────────────┼──────────────────────────┐   │
   │  │      OUTBOUND       │                          │   │
   │  │                     │                          │   │
   │  │  ⑪ Get connection ──┤ (pool lookup)            │   │
   │  │                     │                          │   │
   │  │  ⑫ Rewrite request ─┤ (headers, path)          │   │
   │  │                     │                          │   │
   │  │  ⑬ TLS encrypt ─────┤ (if backend HTTPS)       │   │
   │  │                     │                          │   │
   │  │  ⑭ Serialize HTTP ──┤                          │   │
   │  │                     │                          │   │
   │  │  ⑮ Syscall ─────────┤ (write)                  │   │
   │  │     (context switch)│                          │   │
   │  │                     │                          │   │
   │  │  ⑯ Kernel → NIC ────┼──────────────────────────┼───►
   │  │                     │                          │   │
   │  └─────────────────────┼──────────────────────────┘   │
   │                        │                              │
   │                        │         (backend processes)  │
   │                        │                              │
   │  ┌─────────────────────┼──────────────────────────┐   │
   │  │   RESPONSE BACK     │                          │   │
   │  │                     │                          │   │
   │  │  ⑰ NIC receive ─────┼◄─────────────────────────┼───┤
   │  │                     │                          │   │
   │  │  ⑱ Kernel → user ───┤                          │   │
   │  │                     │                          │   │
   │  │  ⑲ TLS decrypt ─────┤                          │   │
   │  │                     │                          │   │
   │  │  ⑳ HTTP parse ──────┤                          │   │
   │  │                     │                          │   │
   │  │  ㉑ Middleware ──────┤ (response transform)     │   │
   │  │                     │                          │   │
   │  │  ㉒ TLS encrypt ─────┤ (for client)             │   │
   │  │                     │                          │   │
   │  │  ㉓ Serialize ───────┤                          │   │
   │  │                     │                          │   │
   │  │  ㉔ Syscall write ───┤                          │   │
   │  │                     │                          │   │
   ◄──┼──㉕ NIC send ────────┤                          │   │
   │  │                     │                          │   │
   │  └─────────────────────┼──────────────────────────┘   │
   │                        │                              │
```

---

### 📊 Đếm điểm chạm

| Category | Touch Points | Có thể tối ưu? |
|----------|--------------|----------------|
| **Kernel ↔ User-space** | 4 (④⑮⑱㉔) | io_uring: batch syscalls |
| **Memory copy** | 6+ (⑤⑥⑬⑲㉒㉓) | Zero-copy: giảm 50% |
| **TLS operations** | 4 (⑥⑬⑲㉒) | Hardware offload |
| **HTTP parsing** | 2 (⑦⑳) | SIMD parser |
| **Logic decisions** | 4 (⑧⑨⑩⑫) | Compiled routing |

**Tổng: 25 touch points** cho một request-response cycle.

---

### 🔬 Chi tiết từng điểm chạm

#### Kernel-level (không thể tránh, chỉ optimize)

| # | Touch Point | Latency | Có thể bypass? |
|---|-------------|---------|----------------|
| ① | NIC → Kernel buffer | ~1μs | DPDK: bypass kernel |
| ② | TCP/IP stack | ~2-5μs | XDP: early steering |
| ③ | Socket buffer | ~1μs | SO_ZEROCOPY |
| ④ | Syscall read | ~1μs | io_uring: async |
| ⑮ | Syscall write | ~1μs | io_uring: async |
| ⑯ | Kernel → NIC | ~1μs | - |
| ⑰ | NIC → Kernel (response) | ~1μs | - |
| ⑱ | Kernel → User (response) | ~1μs | io_uring |
| ㉔ | Syscall write (response) | ~1μs | io_uring |
| ㉕ | Kernel → NIC (response) | ~1μs | - |

**Kernel overhead: ~10-15μs minimum**

---

#### TLS (expensive!)

| # | Touch Point | Latency | Có thể optimize? |
|---|-------------|---------|------------------|
| ⑥ | TLS decrypt (client) | ~10-50μs | Hardware: QAT, kTLS |
| ⑬ | TLS encrypt (backend) | ~10-50μs | Skip if backend HTTP |
| ⑲ | TLS decrypt (backend) | ~10-50μs | Skip if backend HTTP |
| ㉒ | TLS encrypt (client) | ~10-50μs | Hardware: QAT, kTLS |

**TLS overhead: 40-200μs** (nếu HTTPS cả 2 đầu)

---

#### Application logic (proxy controls này)

| # | Touch Point | Latency | Proposal optimize? |
|---|-------------|---------|-------------------|
| ⑦ | HTTP parse | ~1-5μs | httparse (SIMD) ✅ |
| ⑧ | Route lookup | ~1-100μs | Radix tree ✅ |
| ⑨ | Middleware chain | ~1-10μs | Generic (inline) ✅ |
| ⑩ | Load balance | ~0.1-1μs | Atomic counter ✅ |
| ⑪ | Pool lookup | ~0.1-10μs | Per-worker ✅ |
| ⑫ | Rewrite request | ~0.5-5μs | Zero-copy ✅ |
| ⑳ | HTTP parse response | ~1-5μs | httparse ✅ |
| ㉑ | Response middleware | ~1-10μs | Generic ✅ |

**Application overhead: 5-150μs** (phụ thuộc complexity)

---

### 🎯 Bottleneck thực sự là gì?

```
Latency breakdown (HTTPS client → HTTPS backend):

Kernel I/O:        ████░░░░░░░░░░░░░░░░  ~15μs (10%)
TLS operations:    ████████████████████  ~100μs (65%)
HTTP parsing:      ██░░░░░░░░░░░░░░░░░░  ~5μs (3%)
Routing + MW:      ███░░░░░░░░░░░░░░░░░  ~10μs (7%)
Serialization:     ██░░░░░░░░░░░░░░░░░░  ~5μs (3%)
Memory copy:       ████░░░░░░░░░░░░░░░░  ~15μs (10%)
                   ────────────────────
Total proxy:       ~150μs per request
```

**Insight quan trọng:**
- **TLS = 65% latency** → Hardware offload có impact lớn nhất
- Routing, middleware = 7% → Optimize ở đây có impact nhỏ
- **Nếu backend là HTTP** (không HTTPS) → latency giảm 50%

---

### 🔄 So sánh các optimize strategies

| Strategy | Điểm chạm giảm | Latency giảm | Effort |
|----------|---------------|--------------|--------|
| **Zero-copy buffers** | ⑤⑫㉓ | ~10μs | Medium |
| **io_uring** | ④⑮⑱㉔ | ~5μs | High |
| **Compiled routing** | ⑧ | ~50μs (@ 1000 routes) | Medium |
| **kTLS (kernel TLS)** | ⑥⑬⑲㉒ | ~30μs | Medium |
| **Skip backend TLS** | ⑬⑲ | ~50μs | Config |
| **Connection pool** | ⑪ | ~5μs | Low |
| **eBPF/XDP** | ①②③ | ~5μs | Very High |
| **DPDK** | ①②③④⑮⑯⑰⑱㉔㉕ | ~10μs | Extreme |

---

### 💡 Key Insight cho Proposal

```
PROPOSAL FOCUSES ON:          ACTUAL IMPACT:

Routing (radix tree)          ~5% latency
Middleware (generic)          ~3% latency  
Buffer (zero-copy)            ~10% latency
Connection pool               ~3% latency
                              ──────────
                              ~21% của proxy latency
                              
NHƯNG PROXY LATENCY = 10-20% của TOTAL LATENCY

Vậy tối ưu proposal = 21% × 15% = ~3% total latency improvement
```

---

### ❓ Câu hỏi chiến lược

1. **Bạn có control TLS config?**
   - Nếu backend có thể HTTP → bỏ 2 TLS operations → **50% faster**

2. **Bạn có cần tất cả middleware?**
   - Mỗi middleware = 1-5μs
   - 10 middlewares = 10-50μs overhead

3. **Backend latency là bao nhiêu?**
   - Nếu backend = 50ms → proxy 150μs = **0.3% overhead**
   - Optimize proxy từ 150μs → 50μs = **tiết kiệm 0.2%**

4. **Có thể dùng kernel TLS (kTLS)?**
   - Linux 4.13+ hỗ trợ
   - TLS trong kernel = ít context switch

---

Bạn muốn đi sâu vào điểm chạm nào?