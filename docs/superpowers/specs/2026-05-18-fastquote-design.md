# FastQuote - 实时行情数据平台设计文档

> 日期: 2026-05-18
> 状态: Approved

## 1. 概述

FastQuote 是一个 Rust 实现的实时行情数据平台，从通达信(TDX)和腾讯财经采集行情数据，通过 TCP 二进制和 WebSocket 双协议推送给下游策略/前端，同时异步落库到 Redis + MySQL。

### 核心目标

- **实时行情推送**: TDX TCP 裸连 + 腾讯财经 HTTP 轮询双数据源
- **数据落库**: Redis 缓存最新行情，MySQL 存储历史数据
- **双协议下游**: TCP 二进制（内部策略）+ WebSocket（前端可视化）
- **高可用**: 断线重连、数据源冗余、去重

### 数据类型

- Tick + Level2（买卖五档快照）
- K 线（1min/5min/日线等）
- 财务数据（EPS/ROE/营收/净利润）
- 指数 + 板块行情

## 2. 技术栈

| 组件 | 选择 | 说明 |
|------|------|------|
| 语言 | Rust | 内存安全、高性能、生态丰富 |
| 异步运行时 | tokio | 业界标准，work-stealing 调度 |
| HTTP 客户端 | reqwest | 腾讯财经数据拉取 |
| WebSocket | axum + tokio-tungstenite | 前端推送 |
| 二进制解析 | nom + bytes | TDX 协议编解码 |
| JSON | serde + serde_json | WebSocket 帧序列化 |
| Redis | redis-rs | 实时行情缓存 |
| MySQL | sqlx | 历史数据持久化 |
| 并发容器 | dashmap | 数据新鲜度追踪、会话管理 |
| 日志 | tracing + tracing-subscriber | 结构化日志 |
| 配置 | toml | 运行时配置 |

## 3. 架构设计

### 方案: Adapter + Hub (Pub/Sub)

```
┌─────────────┐                    ┌──────────────┐
│ TdxAdapter  │──→ mpsc ──→───────→│              │──→ broadcast ──→ TcpServer ──→ 策略/内部服务
│ (TCP推送)    │                    │     Hub      │
└─────────────┘                    │              │──→ broadcast ──→ WsServer  ──→ 前端/可视化
                                   │  publish()   │
┌─────────────┐                    │              │──→ broadcast ──→ Persister ──→ Redis + MySQL
│ TencentAdapter│──→ mpsc ──→─────→│              │
│ (HTTP轮询)   │                    └──────────────┘
└─────────────┘
```

核心思想:
- 数据源通过 Adapter trait 隔离，每个数据源独立实现
- Hub 用 `tokio::sync::broadcast` 做发布/订阅，下游各自独立消费
- Persister 异步批量写入存储

## 4. 统一数据模型

```rust
// 基础类型
struct Symbol { code: String, market: String }  // "600000", "SH"
struct Price(i64)    // ×1000 整数存储，避免浮点误差
struct Volume(i64)
struct Amount(i64)   // 单位: 分
struct Time(i64)     // 纳秒时间戳

// 行情快照
struct DepthMarket {
    symbol: Symbol, time: Time,
    last_price: Price, open: Price, high: Price, low: Price, close: Price,
    volume: Volume, amount: Amount,
    bid: [Level; 5], ask: [Level; 5],  // 买卖五档
}
struct Level { price: Price, volume: Volume }

// K 线
struct Kline {
    symbol: Symbol, open_time: Time,
    open: Price, high: Price, low: Price, close: Price,
    volume: Volume, amount: Amount, period: u32,
}

// 财务数据
struct Financial {
    symbol: Symbol, report_date: String,
    eps: f64, bvps: f64, roe: f64, total_revenue: f64, net_profit: f64,
}

// 指数/板块
struct IndexQuote {
    symbol: Symbol, time: Time,
    last_price: Price, volume: Volume, amount: Amount, change_pct: f64,
}

// Hub 消息信封
enum Source { Tdx, Tencent }
enum MarketData { Depth(DepthMarket), Kline(Kline), Financial(Financial), Index(IndexQuote) }
struct HubMessage { source: Source, data: MarketData }
```

## 5. TDX 协议适配器

### 协议格式

```
数据包: [Len(2B,LE)] [Type(2B,LE)] [Body(变长)] [Checksum(2B)]
Checksum = sum(all_bytes) & 0xFFFF
```

### 关键消息类型

| Type | 方向 | 用途 |
|------|------|------|
| 0x01 | C→S | 登录 |
| 0x02 | S→C | 登录响应 |
| 0x0C | C→S | 订阅行情 |
| 0x0D | S→C | 行情推送 |
| 0x05 | C→S | 请求 K 线 |
| 0x06 | S→C | K 线响应 |
| 0x04 | C→S | 请求财务 |
| 0x03 | S→C | 财务响应 |

### 连接生命周期

```
[Disconnected] → connect() → [Connecting] → TCP成功 → [Login]
→ 发送登录包 → 收到响应 → [Subscribed] → 发送订阅 → [Running]
→ 收包/解析/推送Hub → 心跳保活 → 断线 → [Disconnected] (3s后重连)
```

### 模块结构

```
fastquote-tdx/src/
├── connection.rs    # TCP 连接管理 + 自动重连
├── protocol/
│   ├── encoder.rs   # 请求编码（登录/订阅/K线/财务）
│   ├── decoder.rs   # 响应解码 → 统一数据模型
│   └── types.rs     # 协议常量、包类型
└── adapter.rs       # TdxAdapter impl Adapter
```

## 6. 腾讯财经适配器

### 接口

- 实时行情: `https://qt.gtimg.cn/q=sh600000,sz000001` (文本格式)
- K 线: `https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=...` (JSON)
- 财务: `https://qt.gtimg.cn/q=s_sh600000` (文本格式)
- 指数: `https://qt.gtimg.cn/q=sh000001,sz399001` (文本格式)

### 数据源优先级

| 数据类型 | TDX | 腾讯财经 |
|---------|-----|---------|
| Tick + Level2 | **主** | 补充 |
| K 线 | **主** | 补充 |
| 财务数据 | 补充 | **主** |
| 指数 + 板块 | 补充 | **主** |

### 模块结构

```
fastquote-tencent/src/
├── client.rs    # reqwest HTTP 客户端封装
├── parser.rs    # 响应解析（文本/JSON → 统一模型）
└── adapter.rs   # TencentAdapter impl Adapter
```

## 7. Hub 消息中心

```rust
pub struct Hub {
    tx: broadcast::Sender<HubMessage>,
    latest: DashMap<String, Instant>,  // 去重: 同 symbol 5ms 内不重复
}

impl Hub {
    pub fn new(buffer_size: usize) -> Self;
    pub fn publish(&self, msg: HubMessage);       // 数据源调用
    pub fn subscribe(&self) -> broadcast::Receiver<HubMessage>;  // 下游调用
}
```

- `broadcast::Sender`: 所有下游共享同一流，互不影响
- `DashMap`: 并发安全 HashMap，追踪数据新鲜度做去重
- Hub 无业务逻辑，纯粹做路由分发

## 8. 下游协议

### 8.1 TCP 二进制协议（内部策略/服务）

```
帧格式: [Magic(2B): 0xFQ01] [Type(1B)] [Len(4B, BE)] [Payload(N bytes)]

下行 Type: 1=Depth, 2=Kline, 3=Financial, 4=Index
上行 Type: 0x10=Sub, 0x11=Unsub, 0x12=ReqKline, 0x20=Ping, 0x21=Pong

DepthMarket Payload: 固定 128 bytes, packed struct
  market(2B) + code(6B) + ts(8B) + OHLCV(5×8B) + 五档(20×8B)
```

### 8.2 WebSocket 协议（前端/可视化）

```
下行: JSON 文本帧
{ "type": "depth", "symbol": {...}, "data": {...} }

上行: 订阅命令
{ "cmd": "sub", "symbols": ["SH600000"] }
{ "cmd": "unsub", "symbols": ["SH600000"] }
```

### 协议对比

| 维度 | TCP 二进制 | WebSocket |
|------|-----------|-----------|
| 延迟 | ~10-50μs | ~100-500μs |
| 带宽 | Depth 128B | Depth ~500B |
| 目标 | 策略/内部服务 | 前端/可视化 |

## 9. 存储层

### Redis（实时缓存）

```
Key 设计:
  quote:depth:{market}{code}             → Hash    最新快照
  quote:kline:{market}{code}:{period}    → ZSet    最近 N 根 K 线 (score=ts)
  quote:index:{market}{code}             → Hash    最新指数
```

每条行情实时写入 Redis，保证下游可以随时查询最新状态。

### MySQL（历史持久化）

```sql
-- K 线历史
CREATE TABLE kline (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    market VARCHAR(4), code VARCHAR(8), period INT,
    open_time BIGINT, open_price BIGINT, high_price BIGINT,
    low_price BIGINT, close_price BIGINT, volume BIGINT, amount BIGINT,
    UNIQUE KEY uk_kline (market, code, period, open_time)
);

-- 财务数据
CREATE TABLE financial (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    market VARCHAR(4), code VARCHAR(8), report_date DATE,
    eps DOUBLE, bvps DOUBLE, roe DOUBLE,
    total_revenue DOUBLE, net_profit DOUBLE,
    UNIQUE KEY uk_fin (market, code, report_date)
);

-- Tick 快照归档 (按日分区)
CREATE TABLE tick_snapshot (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    market VARCHAR(4), code VARCHAR(8), ts BIGINT,
    last_price BIGINT, volume BIGINT, amount BIGINT,
    bid1_price BIGINT, bid1_vol BIGINT, ... ask5_price BIGINT, ask5_vol BIGINT,
    KEY idx_sym_ts (market, code, ts)
);
```

### 写入策略

- Redis: 每条实时写
- MySQL: 500ms 批量攒写，`INSERT ... ON DUPLICATE KEY UPDATE`
- Tick 归档: 可选，按日分区

## 10. 项目结构

```
FastQuote/
├── Cargo.toml (workspace)
├── config.toml
├── fastquote-core/        # 数据模型 + trait 定义
├── fastquote-tdx/         # TDX 协议适配器
├── fastquote-tencent/     # 腾讯财经适配器
├── fastquote-hub/         # 消息中心
├── fastquote-server/      # TCP + WebSocket 下游服务
├── fastquote-store/       # Redis + MySQL 存储
├── fastquote-bin/         # 可执行入口
└── migrations/            # MySQL schema
```

### Workspace 依赖

```toml
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["rustls-tls"] }
axum = { version = "0.8", features = ["ws"] }
tokio-tungstenite = "0.24"
redis = { version = "0.27", features = ["tokio-comp"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "mysql"] }
bytes = "1"
nom = "8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dashmap = "6"
tracing = "0.1"
tracing-subscriber = "0.3"
toml = "0.8"
```

## 11. 启动流程

```rust
fn main() -> Result<()> {
    // 1. 加载配置
    // 2. 初始化日志
    // 3. 创建 Hub
    // 4. 启动 TDX + 腾讯财经适配器
    // 5. 启动 Persister
    // 6. 启动 TCP Server + WebSocket Server
    // 7. tokio::select! 并行运行所有组件
}
```

配置文件:

```toml
[tdx]
host = "119.147.212.81"
port = 7709
reconnect_ms = 3000
heartbeat_s = 60

[tencent]
poll_interval_ms = 3000
kline_sync_interval_s = 60
financial_sync_interval_s = 3600

[store]
redis_url = "redis://127.0.0.1:6379"
mysql_url = "mysql://root@127.0.0.1/fastquote"

[tcp]
addr = "0.0.0.0:9000"

[ws]
addr = "0.0.0.0:9001"
```
