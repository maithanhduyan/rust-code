# Thiết Kế WebSocket Server cho 100M Users

## Tổng Quan Kiến Trúc

```
                                    ┌─────────────────────────────────────┐
                                    │           Global DNS               │
                                    │    (GeoDNS / Latency-based)        │
                                    └──────────────┬──────────────────────┘
                                                   │
                    ┌──────────────────────────────┼──────────────────────────────┐
                    │                              │                              │
           ┌────────▼────────┐            ┌────────▼────────┐            ┌────────▼────────┐
           │   Region: US    │            │  Region: EU     │            │  Region: Asia   │
           └────────┬────────┘            └────────┬────────┘            └────────┬────────┘
                    │                              │                              │
           ┌────────▼────────┐            ┌────────▼────────┐            ┌────────▼────────┐
           │  Load Balancer  │            │  Load Balancer  │            │  Load Balancer  │
           │  (L4 - HAProxy) │            │  (L4 - HAProxy) │            │  (L4 - HAProxy) │
           └────────┬────────┘            └────────┬────────┘            └────────┬────────┘
                    │                              │                              │
     ┌──────────────┼──────────────┐               │                              │
     │              │              │               │                              │
┌────▼────┐   ┌────▼────┐   ┌────▼────┐           ...                           ...
│ WS Node │   │ WS Node │   │ WS Node │
│  1M conn│   │  1M conn│   │  1M conn│    (100 nodes per region)
└────┬────┘   └────┬────┘   └────┬────┘
     │              │              │
     └──────────────┼──────────────┘
                    │
           ┌────────▼────────┐
           │   Message Bus   │
           │ (Redis Cluster/ │
           │  Kafka/NATS)    │
           └────────┬────────┘
                    │
     ┌──────────────┼──────────────┐
     │              │              │
┌────▼────┐   ┌────▼────┐   ┌────▼────┐
│ Presence│   │ Message │   │  User   │
│ Service │   │ History │   │ Service │
└─────────┘   └─────────┘   └─────────┘
```

## Tính Toán Capacity

| Metric | Giá trị |
|--------|---------|
| **Total Users** | 100M |
| **Peak Concurrent** | ~10M (10% online) |
| **Connections/Node** | 1M |
| **Nodes Required** | 10-15 nodes |
| **Regions** | 3-5 |
| **Nodes/Region** | 3-5 |

## Các Thành Phần Chính

### 1. Edge Layer - GeoDNS + CDN

```
┌─────────────────────────────────────────────────────────┐
│                     Cloudflare / AWS Route53            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │ US-East     │  │ EU-West     │  │ AP-Tokyo    │     │
│  │ ws.app.com  │  │ ws.app.com  │  │ ws.app.com  │     │
│  │ → us-east   │  │ → eu-west   │  │ → ap-tokyo  │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
└─────────────────────────────────────────────────────────┘
```

### 2. Load Balancer Layer (L4)

```rust
// HAProxy config cho WebSocket
// /etc/haproxy/haproxy.cfg

frontend ws_front
    bind *:443 ssl crt /path/to/cert.pem
    mode tcp
    option tcplog
    default_backend ws_servers
    
    # Sticky sessions based on source IP
    stick-table type ip size 1m expire 30m
    stick on src

backend ws_servers
    mode tcp
    balance leastconn
    option httpchk GET /health
    
    server ws1 10.0.1.1:8080 check weight 100 maxconn 100000
    server ws2 10.0.1.2:8080 check weight 100 maxconn 100000
    server ws3 10.0.1.3:8080 check weight 100 maxconn 100000
```

### 3. WebSocket Node (1M connections each)

```rust
// Optimized server config
pub struct NodeConfig {
    pub node_id: String,
    pub region: String,
    pub max_connections: usize,      // 1_000_000
    pub workers: usize,              // num_cpus
    pub message_bus: MessageBusConfig,
    pub presence_service: PresenceConfig,
}

// Connection routing
pub struct ConnectionRouter {
    // Shard users across nodes using consistent hashing
    ring: HashRing<NodeId>,
    
    // Local connections on this node
    local: DashMap<UserId, ConnectionHandle>,
    
    // Remote node lookup
    remote: DashMap<UserId, NodeId>,
}
```

### 4. Message Bus (Cross-Node Communication)

```
┌─────────────────────────────────────────────────────────┐
│                    Redis Cluster                         │
│                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Master 1 │  │ Master 2 │  │ Master 3 │   (6 nodes)  │
│  │ Replica  │  │ Replica  │  │ Replica  │              │
│  └──────────┘  └──────────┘  └──────────┘              │
│                                                          │
│  Pub/Sub Channels:                                       │
│  - room:{room_id}     → Room messages                   │
│  - user:{user_id}     → Direct messages                 │
│  - broadcast:global   → System broadcasts               │
│  - node:{node_id}     → Node-specific commands          │
└─────────────────────────────────────────────────────────┘
```

```rust
// Cross-node messaging
pub struct MessageBus {
    redis: redis::cluster::ClusterClient,
}

impl MessageBus {
    // Publish message to a room (all nodes subscribe)
    pub async fn publish_room(&self, room_id: &str, msg: &ChatMessage) -> Result<()> {
        let channel = format!("room:{}", room_id);
        let data = bincode::serialize(msg)?;
        self.redis.publish(channel, data).await
    }
    
    // Direct message to specific user
    pub async fn send_to_user(&self, user_id: u64, msg: &ChatMessage) -> Result<()> {
        let channel = format!("user:{}", user_id);
        let data = bincode::serialize(msg)?;
        self.redis.publish(channel, data).await
    }
}
```

### 5. Presence Service (Who's Online)

```rust
// Distributed presence using Redis
pub struct PresenceService {
    redis: redis::cluster::ClusterClient,
    node_id: String,
}

impl PresenceService {
    // Register user online (with TTL for auto-cleanup)
    pub async fn set_online(&self, user_id: u64) -> Result<()> {
        let key = format!("presence:{}", user_id);
        let value = &self.node_id;
        // Set with 60s TTL, refresh every 30s
        self.redis.set_ex(key, value, 60).await
    }
    
    // Find which node has a user
    pub async fn find_user(&self, user_id: u64) -> Option<String> {
        let key = format!("presence:{}", user_id);
        self.redis.get(key).await.ok()
    }
    
    // Get online count (approximate)
    pub async fn online_count(&self) -> u64 {
        // Use HyperLogLog for memory-efficient counting
        self.redis.pfcount("presence:hll").await.unwrap_or(0)
    }
}
```

### 6. Room/Channel Management

```rust
// Distributed room management
pub struct RoomManager {
    // Local room subscriptions on this node
    local_rooms: DashMap<RoomId, HashSet<UserId>>,
    
    // Redis for cross-node room membership
    redis: redis::cluster::ClusterClient,
}

impl RoomManager {
    pub async fn join_room(&self, user_id: u64, room_id: &str) -> Result<()> {
        // Add to local
        self.local_rooms
            .entry(room_id.to_string())
            .or_default()
            .insert(user_id);
        
        // Add to Redis set
        let key = format!("room:members:{}", room_id);
        self.redis.sadd(key, user_id).await?;
        
        // Subscribe to room channel
        self.redis.subscribe(format!("room:{}", room_id)).await
    }
    
    pub async fn broadcast_to_room(&self, room_id: &str, msg: &ChatMessage) -> Result<()> {
        // Publish to Redis, all nodes receive
        let channel = format!("room:{}", room_id);
        self.redis.publish(channel, bincode::serialize(msg)?).await
    }
}
```

## Data Flow

```
User A (Node 1) sends message to Room "general"
          │
          ▼
┌─────────────────────┐
│  Node 1 receives    │
│  WebSocket message  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Publish to Redis   │
│  room:general       │
└──────────┬──────────┘
           │
     ┌─────┴─────┬─────────────┐
     ▼           ▼             ▼
┌─────────┐ ┌─────────┐   ┌─────────┐
│ Node 1  │ │ Node 2  │   │ Node 3  │
│ receives│ │ receives│   │ receives│
│ pub/sub │ │ pub/sub │   │ pub/sub │
└────┬────┘ └────┬────┘   └────┬────┘
     │           │             │
     ▼           ▼             ▼
  Local       Local         Local
  users in    users in      users in
  "general"   "general"     "general"
```

## Database Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Data Layer                               │
│                                                                  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │  Redis Cluster  │  │  ScyllaDB/      │  │   PostgreSQL    │ │
│  │  (Hot Data)     │  │  Cassandra      │  │   (Cold Data)   │ │
│  │                 │  │  (Message Store)│  │                 │ │
│  │  - Sessions     │  │                 │  │  - Users        │ │
│  │  - Presence     │  │  - Messages     │  │  - Rooms        │ │
│  │  - Pub/Sub      │  │  - Time-series  │  │  - Settings     │ │
│  │  - Rate limits  │  │  - 100TB+       │  │                 │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Kubernetes Deployment

```yaml
# websocket-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: websocket-server
spec:
  replicas: 10  # 10 nodes = 10M connections capacity
  selector:
    matchLabels:
      app: websocket
  template:
    metadata:
      labels:
        app: websocket
    spec:
      affinity:
        podAntiAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
          - labelSelector:
              matchLabels:
                app: websocket
            topologyKey: "kubernetes.io/hostname"
      containers:
      - name: websocket
        image: websocket-rs:latest
        ports:
        - containerPort: 8080
        resources:
          requests:
            memory: "32Gi"
            cpu: "8"
          limits:
            memory: "64Gi"
            cpu: "16"
        env:
        - name: REDIS_CLUSTER
          value: "redis-cluster:6379"
        - name: MAX_CONNECTIONS
          value: "1000000"
---
apiVersion: v1
kind: Service
metadata:
  name: websocket-service
spec:
  type: LoadBalancer
  externalTrafficPolicy: Local  # Preserve client IP
  ports:
  - port: 443
    targetPort: 8080
  selector:
    app: websocket
```

## Cost Estimation (AWS)

| Component | Spec | Qty | Monthly Cost |
|-----------|------|-----|--------------|
| **WS Nodes** | c6i.4xlarge (16 vCPU, 32GB) | 15 | $7,500 |
| **Redis Cluster** | r6g.2xlarge | 6 | $3,600 |
| **ScyllaDB** | i3.2xlarge | 6 | $6,000 |
| **Load Balancer** | NLB | 3 | $500 |
| **Data Transfer** | 100TB/month | - | $8,000 |
| **Total** | | | **~$25,000/month** |

## Key Optimizations

| Optimization | Impact |
|--------------|--------|
| **Binary Protocol** | 70% bandwidth reduction |
| **Connection Pooling** | Reduce Redis connections |
| **Batch Messages** | Group messages per 10ms |
| **Compression** | LZ4 for large payloads |
| **Edge Caching** | Cache room metadata |
| **Rate Limiting** | Protect against abuse |

## Summary

Để phục vụ **100M users**:

1. **~10-15 WebSocket nodes** (mỗi node 1M connections)
2. **3-5 regions** cho low latency globally  
3. **Redis Cluster** cho pub/sub và presence
4. **Consistent Hashing** để route users đến nodes
5. **GeoDNS** để route users đến region gần nhất
6. **Kubernetes** để auto-scaling và healing