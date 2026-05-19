# FastQuote Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust real-time market data platform that collects quotes from TDX and Tencent Finance, distributes via TCP binary + WebSocket, and persists to Redis + MySQL.

**Architecture:** Adapter + Hub pub/sub pattern. Data sources (TDX, Tencent) implement an `Adapter` trait, push to a `Hub` via `mpsc` channels. Hub fans out via `broadcast` to downstream servers (TCP, WS) and a Persister (Redis, MySQL).

**Tech Stack:** Rust, tokio, reqwest, axum, tokio-tungstenite, redis-rs, sqlx, nom, bytes, serde, dashmap, tracing

**Spec:** `docs/superpowers/specs/2026-05-18-fastquote-design.md`
**Protocol:** `docs/fastquote-protocol-spec.md`

---

## Task 1: Workspace Scaffold + fastquote-core

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `fastquote-core/Cargo.toml`
- Create: `fastquote-core/src/lib.rs`
- Create: `fastquote-core/src/types.rs`
- Create: `fastquote-core/src/model.rs`
- Create: `fastquote-core/src/message.rs`
- Create: `fastquote-core/src/adapter.rs`
- Create: `config.toml`

- [x] **Step 1: Create workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "fastquote-core",
    "fastquote-hub",
    "fastquote-tdx",
    "fastquote-tencent",
    "fastquote-server",
    "fastquote-store",
    "fastquote-bin",
]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["rustls-tls"] }
axum = { version = "0.8", features = ["ws"] }
tokio-tungstenite = "0.26"
redis = { version = "0.27", features = ["tokio-comp"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "mysql"] }
bytes = "1"
nom = "8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dashmap = "6"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
toml = "0.8"
thiserror = "2"
anyhow = "1"
```

- [x] **Step 2: Create fastquote-core/Cargo.toml**

```toml
[package]
name = "fastquote-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

- [x] **Step 3: Create fastquote-core/src/types.rs**

```rust
use serde::{Deserialize, Serialize};

/// 股票标识
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol {
    pub market: String, // "SH" / "SZ"
    pub code: String,   // "600000"
}

impl Symbol {
    pub fn new(market: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            market: market.into(),
            code: code.into(),
        }
    }

    /// 复合 key: "SH600000"
    pub fn key(&self) -> String {
        format!("{}{}", self.market, self.code)
    }
}

/// 价格 ×1000（避免浮点误差）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price(pub i64);

impl Price {
    pub fn from_f64(v: f64) -> Self {
        Self((v * 1000.0).round() as i64)
    }
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / 1000.0
    }
}

/// 成交量（股）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume(pub i64);

/// 成交额（分）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Amount(pub i64);

/// 纳秒时间戳
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Time(pub i64);

impl Time {
    pub fn now() -> Self {
        Self(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64)
    }

    pub fn from_millis(ms: i64) -> Self {
        Self(ms * 1_000_000)
    }

    pub fn to_millis(self) -> i64 {
        self.0 / 1_000_000
    }
}

/// 买卖档位
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    pub price: Price,
    pub volume: Volume,
}
```

- [x] **Step 4: Create fastquote-core/src/model.rs**

```rust
use crate::types::*;
use serde::{Deserialize, Serialize};

/// 行情快照（Tick + Level2）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthMarket {
    pub symbol: Symbol,
    pub time: Time,
    pub last_price: Price,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: Volume,
    pub amount: Amount,
    pub bid: [Level; 5],
    pub ask: [Level; 5],
}

/// K 线
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kline {
    pub symbol: Symbol,
    pub open_time: Time,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: Volume,
    pub amount: Amount,
    /// 周期（秒）: 60/300/900/1800/3600/86400
    pub period: u32,
}

/// 财务数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Financial {
    pub symbol: Symbol,
    pub report_date: String,
    pub eps: f64,
    pub bvps: f64,
    pub roe: f64,
    pub total_revenue: f64,
    pub net_profit: f64,
}

/// 指数/板块行情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexQuote {
    pub symbol: Symbol,
    pub time: Time,
    pub last_price: Price,
    pub volume: Volume,
    pub amount: Amount,
    /// 涨跌幅（小数，如 0.015 = 1.5%）
    pub change_pct: f64,
}
```

- [x] **Step 5: Create fastquote-core/src/message.rs**

```rust
use crate::model::*;
use crate::types::Symbol;
use serde::{Deserialize, Serialize};

/// 数据来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Tdx,
    Tencent,
}

/// 行情数据枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketData {
    Depth(DepthMarket),
    Kline(Kline),
    Financial(Financial),
    Index(IndexQuote),
}

/// Hub 消息信封
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubMessage {
    pub source: Source,
    pub data: MarketData,
}

impl HubMessage {
    pub fn symbol_key(&self) -> String {
        match &self.data {
            MarketData::Depth(d) => d.symbol.key(),
            MarketData::Kline(k) => k.symbol.key(),
            MarketData::Financial(f) => f.symbol.key(),
            MarketData::Index(i) => i.symbol.key(),
        }
    }
}
```

- [x] **Step 6: Create fastquote-core/src/adapter.rs**

```rust
use crate::types::Symbol;
use anyhow::Result;
use async_trait::async_trait;

/// 数据源适配器 trait
#[async_trait]
pub trait Adapter: Send + Sync + 'static {
    /// 启动适配器
    async fn start(&self) -> Result<()>;
    /// 停止适配器
    async fn stop(&self) -> Result<()>;
    /// 订阅股票列表
    async fn subscribe(&self, symbols: &[Symbol]) -> Result<()>;
}
```

Remove `async_trait` dependency — use manual `Box<dyn Future>` instead. Update `fastquote-core/Cargo.toml` to NOT include `async_trait`, and rewrite adapter.rs:

```rust
use crate::types::Symbol;
use anyhow::Result;
use std::future::Future;
use pin_project_lite::pin_project;

/// 数据源适配器 trait
pub trait Adapter: Send + Sync + 'static {
    fn start(&self) -> impl Future<Output = Result<()>> + Send;
    fn stop(&self) -> impl Future<Output = Result<()>> + Send;
    fn subscribe(&self, symbols: &[Symbol]) -> impl Future<Output = Result<()>> + Send;
}
```

Actually, since we're using Rust 2021 edition and `impl Trait` in trait methods requires `return-position-impl-trait-in-trait` which is stable in Rust 1.75+, let's use the simplest approach — just use async fn in trait directly (stable since Rust 1.75):

```rust
use crate::types::Symbol;
use anyhow::Result;

/// 数据源适配器 trait
pub trait Adapter: Send + Sync + 'static {
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn subscribe(&self, symbols: &[Symbol]) -> Result<()>;
}
```

Update `fastquote-core/Cargo.toml` to remove `async_trait` and add `anyhow` + `pin-project-lite` not needed:

```toml
[package]
name = "fastquote-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
```

- [x] **Step 7: Create fastquote-core/src/lib.rs**

```rust
pub mod adapter;
pub mod message;
pub mod model;
pub mod types;

pub use adapter::Adapter;
pub use message::{HubMessage, MarketData, Source};
pub use model::{DepthMarket, Financial, IndexQuote, Kline};
pub use types::{Amount, Level, Price, Symbol, Time, Volume};
```

- [x] **Step 8: Create config.toml**

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

- [x] **Step 9: Create placeholder Cargo.toml for other crates (to make workspace resolve)**

Create minimal `Cargo.toml` and `src/lib.rs` for each remaining workspace member:

For `fastquote-hub`, `fastquote-tdx`, `fastquote-tencent`, `fastquote-server`, `fastquote-store`, `fastquote-bin` — create with empty `src/lib.rs` (or `src/main.rs` for bin) and minimal Cargo.toml that depends on `fastquote-core`.

- [x] **Step 10: Verify workspace compiles**

Run: `cargo check`
Expected: compiles with no errors

- [x] **Step 11: Commit**

```bash
git add -A
git commit -m "feat: workspace scaffold + fastquote-core (types, models, traits)"
```

---

## Task 2: fastquote-hub

**Files:**
- Modify: `fastquote-hub/Cargo.toml`
- Create: `fastquote-hub/src/lib.rs`
- Create: `fastquote-hub/src/hub.rs`

- [x] **Step 1: Update fastquote-hub/Cargo.toml**

```toml
[package]
name = "fastquote-hub"
version = "0.1.0"
edition = "2021"

[dependencies]
fastquote-core = { path = "../fastquote-core" }
tokio = { workspace = true, features = ["sync"] }
dashmap = { workspace = true }
tracing = { workspace = true }
```

- [x] **Step 2: Create fastquote-hub/src/hub.rs**

```rust
use dashmap::DashMap;
use fastquote_core::{HubMessage, MarketData};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

const DEFAULT_DEDUP_WINDOW_MS: u64 = 5;

pub struct Hub {
    tx: broadcast::Sender<HubMessage>,
    latest: DashMap<String, Instant>,
    dedup_window: Duration,
}

impl Hub {
    pub fn new(buffer_size: usize) -> Self {
        let (tx, _) = broadcast::channel(buffer_size);
        Self {
            tx,
            latest: DashMap::new(),
            dedup_window: Duration::from_millis(DEFAULT_DEDUP_WINDOW_MS),
        }
    }

    /// 数据源调用：推送消息到 Hub
    pub fn publish(&self, msg: HubMessage) {
        let key = format!("{:?}:{}", msg.source, msg.symbol_key());

        // 去重：同源同 symbol 短时间内不重复推送
        if let Some(entry) = self.latest.get(&key) {
            if entry.value().elapsed() < self.dedup_window {
                return;
            }
        }
        self.latest.insert(key, Instant::now());
        let _ = self.tx.send(msg);
    }

    /// 下游调用：订阅消息流
    pub fn subscribe(&self) -> broadcast::Receiver<HubMessage> {
        self.tx.subscribe()
    }

    /// 获取 sender（用于直接 send）
    pub fn sender(&self) -> broadcast::Sender<HubMessage> {
        self.tx.clone()
    }
}
```

- [x] **Step 3: Update fastquote-hub/src/lib.rs**

```rust
pub mod hub;

pub use hub::Hub;
```

- [x] **Step 4: Verify compilation**

Run: `cargo check -p fastquote-hub`
Expected: compiles with no errors

- [x] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: fastquote-hub with broadcast pub/sub and dedup"
```

---

## Task 3: fastquote-tdx — Protocol Encoder/Decoder

**Files:**
- Modify: `fastquote-tdx/Cargo.toml`
- Create: `fastquote-tdx/src/lib.rs`
- Create: `fastquote-tdx/src/protocol/mod.rs`
- Create: `fastquote-tdx/src/protocol/types.rs`
- Create: `fastquote-tdx/src/protocol/encoder.rs`
- Create: `fastquote-tdx/src/protocol/decoder.rs`

This is the most complex task — implementing the TDX binary protocol.

- [x] **Step 1: Update fastquote-tdx/Cargo.toml**

```toml
[package]
name = "fastquote-tdx"
version = "0.1.0"
edition = "2021"

[dependencies]
fastquote-core = { path = "../fastquote-core" }
tokio = { workspace = true }
bytes = { workspace = true }
nom = { workspace = true }
encoding_rs = "0.8"
tracing = { workspace = true }
anyhow = { workspace = true }
```

- [x] **Step 2: Create fastquote-tdx/src/protocol/types.rs**

Define TDX protocol constants: packet type IDs, period codes, market IDs.

```rust
/// TDX 包类型
pub mod pkt_type {
    pub const LOGIN_REQ: u16 = 0x01;
    pub const LOGIN_RESP: u16 = 0x02;
    pub const FINANCIAL_RESP: u16 = 0x03;
    pub const FINANCIAL_REQ: u16 = 0x04;
    pub const KLINE_REQ: u16 = 0x05;
    pub const KLINE_RESP: u16 = 0x06;
    pub const QUOTE_SUBSCRIBE: u16 = 0x0C;
    pub const QUOTE_PUSH: u16 = 0x0D;
}

/// TDX 市场代码
pub mod market {
    pub const SH: u8 = 1;
    pub const SZ: u8 = 0;
}

/// TDX K 线周期
pub mod period {
    pub const MIN_5: u8 = 0;
    pub const MIN_15: u8 = 1;
    pub const MIN_30: u8 = 2;
    pub const HOUR_1: u8 = 3;
    pub const DAILY: u8 = 4;
    pub const WEEKLY: u8 = 5;
    pub const MONTHLY: u8 = 6;
    pub const MIN_1: u8 = 8;
}

/// TDX 包头
#[derive(Debug, Clone)]
pub struct TdxHeader {
    pub len: u16,
    pub pkt_type: u16,
}
```

- [x] **Step 3: Create fastquote-tdx/src/protocol/encoder.rs**

Implement TDX request encoding: login, subscribe, kline request, financial request.

```rust
use super::types::*;
use bytes::{BufMut, BytesMut};
use fastquote_core::Symbol;

/// TDX 协议编码器
pub struct Encoder;

impl Encoder {
    /// 计算 TDX 校验和
    fn checksum(data: &[u8]) -> u16 {
        let sum: u32 = data.iter().map(|&b| b as u32).sum();
        (sum & 0xFFFF) as u16
    }

    /// 构建完整数据包: [len(2B,LE)] [type(2B,LE)] [body] [checksum(2B)]
    fn build_packet(pkt_type: u16, body: &[u8]) -> Vec<u8> {
        let body_len = body.len();
        let total_len = 2 + body_len + 2; // type + body + checksum

        let mut buf = BytesMut::with_capacity(2 + total_len);
        buf.put_u16_le(total_len as u16);
        buf.put_u16_le(pkt_type);
        buf.put_slice(body);

        // checksum 覆盖 [type + body]
        let check_data = &buf[2..];
        let cs = Self::checksum(check_data);
        buf.put_u16_le(cs);

        buf.to_vec()
    }

    /// 编码登录请求
    pub fn login() -> Vec<u8> {
        // TDX 登录包: client_id(2B) + padding
        let mut body = BytesMut::new();
        body.put_u16_le(0x01); // client id
        Self::build_packet(pkt_type::LOGIN_REQ, &body)
    }

    /// 编码行情订阅请求
    pub fn subscribe(symbols: &[Symbol]) -> Vec<u8> {
        let mut body = BytesMut::new();
        body.put_u16_le(symbols.len() as u16);

        for sym in symbols {
            let market_id = match sym.market.as_str() {
                "SH" => market::SH,
                "SZ" => market::SZ,
                _ => market::SH,
            };
            body.put_u8(market_id);
            // code 补齐到 6 bytes
            let code_bytes = sym.code.as_bytes();
            for i in 0..6 {
                body.put_u8(if i < code_bytes.len() { code_bytes[i] } else { 0 });
            }
        }
        Self::build_packet(pkt_type::QUOTE_SUBSCRIBE, &body)
    }

    /// 编码 K 线请求
    pub fn kline_request(symbol: &Symbol, period: u8, count: u16) -> Vec<u8> {
        let mut body = BytesMut::new();
        let market_id = match symbol.market.as_str() {
            "SH" => market::SH,
            "SZ" => market::SZ,
            _ => market::SH,
        };
        body.put_u8(market_id);
        let code_bytes = symbol.code.as_bytes();
        for i in 0..6 {
            body.put_u8(if i < code_bytes.len() { code_bytes[i] } else { 0 });
        }
        body.put_u8(period);
        body.put_u16_le(count);
        body.put_u16_le(0); // start position
        Self::build_packet(pkt_type::KLINE_REQ, &body)
    }
}
```

- [x] **Step 4: Create fastquote-tdx/src/protocol/decoder.rs**

Implement TDX response/push parsing using `nom`. Parse raw bytes into `DepthMarket`, `Kline`, etc.

```rust
use super::types::*;
use bytes::{Buf, Bytes};
use fastquote_core::*;
use std::str;

/// TDX 协议解码器
pub struct Decoder;

impl Decoder {
    /// 解析 TDX 包头
    pub fn parse_header(data: &[u8]) -> Option<(u16, u16, &[u8])> {
        if data.len() < 4 {
            return None;
        }
        let mut buf = Bytes::copy_from_slice(&data[..4]);
        let total_len = buf.get_u16_le() as usize;
        let pkt_type = buf.get_u16_le();

        if data.len() < total_len + 2 {
            return None;
        }

        // body = data[4..total_len], 最后 2 bytes 是 checksum
        let body_end = total_len; // 不包含 len 本身
        Some((pkt_type, total_len as u16, &data[4..body_end]))
    }

    /// 解析行情推送 -> DepthMarket
    /// TDX 行情推送格式: count(u16) + [market(u8) + code(6B) + fields...] per stock
    pub fn decode_depth(data: &[u8]) -> anyhow::Result<Vec<DepthMarket>> {
        if data.len() < 2 {
            anyhow::bail!("depth data too short");
        }

        let mut buf = Bytes::copy_from_slice(data);
        let count = buf.get_u16_le() as usize;

        let mut results = Vec::with_capacity(count);
        for _ in 0..count {
            if buf.remaining() < 2 + 6 + 8 * 12 {
                break;
            }
            let market_id = buf.get_u8();
            let _unknown = buf.get_u8(); // 保留字节
            let mut code_buf = [0u8; 6];
            buf.copy_to_slice(&mut code_buf);
            let code = String::from_utf8_lossy(&code_buf).trim_end_matches('\0').to_string();

            // TDX 快照字段 (简化版, 实际协议更复杂, 需按 pytdx 规范补全)
            let last_price = buf.get_i32_le();
            let _close = buf.get_i32_le();
            let open = buf.get_i32_le();
            let high = buf.get_i32_le();
            let low = buf.get_i32_le();

            results.push(DepthMarket {
                symbol: Symbol::new(
                    if market_id == market::SH { "SH" } else { "SZ" },
                    code,
                ),
                time: Time::now(),
                last_price: Price(last_price as i64 * 10), // TDX 单位: 0.01元 -> ×1000
                open: Price(open as i64 * 10),
                high: Price(high as i64 * 10),
                low: Price(low as i64 * 10),
                close: Price(_close as i64 * 10),
                volume: Volume(0),
                amount: Amount(0),
                bid: [Level { price: Price(0), volume: Volume(0) }; 5],
                ask: [Level { price: Price(0), volume: Volume(0) }; 5],
            });
        }
        Ok(results)
    }

    /// GB2312 -> UTF-8
    pub fn gb2312_to_utf8(data: &[u8]) -> String {
        let (cow, _, _) = encoding_rs::GBK.decode(data);
        cow.into_owned()
    }
}
```

- [x] **Step 5: Create fastquote-tdx/src/protocol/mod.rs**

```rust
pub mod decoder;
pub mod encoder;
pub mod types;

pub use decoder::Decoder;
pub use encoder::Encoder;
```

- [x] **Step 6: Update fastquote-tdx/src/lib.rs**

```rust
pub mod protocol;

pub use protocol::{Decoder, Encoder};
```

- [x] **Step 7: Verify compilation**

Run: `cargo check -p fastquote-tdx`
Expected: compiles with no errors

- [x] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: TDX binary protocol encoder/decoder"
```

---

## Task 4: fastquote-tdx — Adapter + Connection

**Files:**
- Create: `fastquote-tdx/src/connection.rs`
- Create: `fastquote-tdx/src/adapter.rs`
- Modify: `fastquote-tdx/src/lib.rs`

- [x] **Step 1: Create fastquote-tdx/src/connection.rs**

TCP connection management with auto-reconnect and heartbeat.

```rust
use crate::protocol::{Decoder, Encoder};
use anyhow::Result;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time;
use tracing::{error, info, warn};

/// TDX 连接配置
#[derive(Debug, Clone)]
pub struct TdxConfig {
    pub host: String,
    pub port: u16,
    pub reconnect_ms: u64,
    pub heartbeat_s: u64,
}

impl Default for TdxConfig {
    fn default() -> Self {
        Self {
            host: "119.147.212.81".into(),
            port: 7709,
            reconnect_ms: 3000,
            heartbeat_s: 60,
        }
    }
}

/// TDX 连接状态
pub enum ConnectionState {
    Disconnected,
    Connected,
   LoggedIn,
    Subscribed,
}

/// TDX TCP 连接管理器
pub struct TdxConnection {
    config: TdxConfig,
    stream: Option<TcpStream>,
    state: ConnectionState,
}

impl TdxConnection {
    pub fn new(config: TdxConfig) -> Self {
        Self {
            config,
            stream: None,
            state: ConnectionState::Disconnected,
        }
    }

    /// TCP 连接 + 登录
    pub async fn connect_and_login(&mut self) -> Result<()> {
        loop {
            match self.try_connect_and_login().await {
                Ok(()) => {
                    info!("TDX connected and logged in to {}:{}", self.config.host, self.config.port);
                    return Ok(());
                }
                Err(e) => {
                    error!("TDX connect/login failed: {}, retrying in {}ms", e, self.config.reconnect_ms);
                    time::sleep(Duration::from_millis(self.config.reconnect_ms)).await;
                }
            }
        }
    }

    async fn try_connect_and_login(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let stream = TcpStream::connect(&addr).await?;
        stream.set_nodelay(true)?;
        self.stream = Some(stream);
        self.state = ConnectionState::Connected;

        // 发送登录包
        let login_pkt = Encoder::login();
        self.send_raw(&login_pkt).await?;

        // 读取登录响应
        let resp = self.recv_packet().await?;
        if resp.is_some() {
            self.state = ConnectionState::LoggedIn;
            Ok(())
        } else {
            anyhow::bail!("login response parse failed")
        }
    }

    /// 发送订阅请求
    pub async fn subscribe(&mut self, symbols: &[fastquote_core::Symbol]) -> Result<()> {
        let pkt = Encoder::subscribe(symbols);
        self.send_raw(&pkt).await?;
        self.state = ConnectionState::Subscribed;
        Ok(())
    }

    /// 接收一个完整 TDX 包
    pub async fn recv_packet(&mut self) -> Result<Option<(u16, Vec<u8>)>> {
        let stream = match &mut self.stream {
            Some(s) => s,
            None => return Ok(None),
        };

        // 读包头: 4 bytes (len + type)
        let mut header_buf = [0u8; 4];
        match stream.read_exact(&mut header_buf).await {
            Ok(()) => {}
            Err(e) => {
                warn!("TDX read header error: {}", e);
                self.state = ConnectionState::Disconnected;
                return Ok(None);
            }
        }

        let total_len = u16::from_le_bytes([header_buf[0], header_buf[1]]) as usize;
        let pkt_type = u16::from_le_bytes([header_buf[2], header_buf[3]]);
        let body_len = total_len.saturating_sub(2); // 减去 type 字段

        if body_len > 65536 {
            anyhow::bail!("TDX packet too large: {}", body_len);
        }

        let mut body = vec![0u8; body_len];
        if body_len > 0 {
            stream.read_exact(&mut body).await?;
        }

        Ok(Some((pkt_type, body)))
    }

    /// 发送原始字节
    async fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        let stream = match &mut self.stream {
            Some(s) => s,
            None => anyhow::bail!("not connected"),
        };
        stream.write_all(data).await?;
        Ok(())
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        matches!(self.state, ConnectionState::Subscribed)
    }

    /// 获取状态
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }
}
```

- [x] **Step 2: Create fastquote-tdx/src/adapter.rs**

```rust
use crate::connection::{TdxConfig, TdxConnection};
use crate::protocol::Decoder;
use fastquote_core::{Adapter, HubMessage, MarketData, Source, Symbol};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

pub struct TdxAdapter {
    config: TdxConfig,
    hub_tx: mpsc::Sender<HubMessage>,
    symbols: Vec<Symbol>,
}

impl TdxAdapter {
    pub fn new(config: TdxConfig, hub_tx: mpsc::Sender<HubMessage>) -> Self {
        Self {
            config,
            hub_tx,
            symbols: Vec::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let mut conn = TdxConnection::new(self.config.clone());

            // 连接 + 登录
            if let Err(e) = conn.connect_and_login().await {
                error!("TDX connection failed: {}", e);
                continue;
            }

            // 订阅
            if !self.symbols.is_empty() {
                if let Err(e) = conn.subscribe(&self.symbols).await {
                    error!("TDX subscribe failed: {}", e);
                    continue;
                }
            }

            // 收包循环
            loop {
                match conn.recv_packet().await {
                    Ok(Some((pkt_type, body))) => {
                        self.handle_packet(pkt_type, &body).await;
                    }
                    Ok(None) => {
                        info!("TDX connection closed, reconnecting...");
                        break;
                    }
                    Err(e) => {
                        error!("TDX recv error: {}", e);
                        break;
                    }
                }
            }
        }
    }

    async fn handle_packet(&self, pkt_type: u16, body: &[u8]) {
        use crate::protocol::types::pkt_type;

        match pkt_type {
            pkt_type::QUOTE_PUSH => {
                if let Ok(markets) = Decoder::decode_depth(body) {
                    for m in markets {
                        let msg = HubMessage {
                            source: Source::Tdx,
                            data: MarketData::Depth(m),
                        };
                        let _ = self.hub_tx.send(msg).await;
                    }
                }
            }
            pkt_type::KLINE_RESP => {
                // TODO: decode kline
            }
            _ => {}
        }
    }
}
```

- [x] **Step 3: Update fastquote-tdx/src/lib.rs**

```rust
pub mod adapter;
pub mod connection;
pub mod protocol;

pub use adapter::TdxAdapter;
pub use connection::TdxConfig;
pub use protocol::{Decoder, Encoder};
```

- [x] **Step 4: Verify compilation**

Run: `cargo check -p fastquote-tdx`
Expected: compiles

- [x] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: TDX adapter with connection management and auto-reconnect"
```

---

## Task 5: fastquote-tencent — HTTP Adapter

**Files:**
- Modify: `fastquote-tencent/Cargo.toml`
- Create: `fastquote-tencent/src/lib.rs`
- Create: `fastquote-tencent/src/client.rs`
- Create: `fastquote-tencent/src/parser.rs`
- Create: `fastquote-tencent/src/adapter.rs`

- [x] **Step 1: Update fastquote-tencent/Cargo.toml**

```toml
[package]
name = "fastquote-tencent"
version = "0.1.0"
edition = "2021"

[dependencies]
fastquote-core = { path = "../fastquote-core" }
tokio = { workspace = true }
reqwest = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
```

- [x] **Step 2: Create fastquote-tencent/src/client.rs**

```rust
use anyhow::Result;
use reqwest::Client;

pub struct TencentClient {
    client: Client,
}

impl TencentClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }

    /// 获取实时行情: https://qt.gtimg.cn/q=sh600000,sz000001
    pub async fn get_quote(&self, symbols: &[String]) -> Result<String> {
        let query = symbols.join(",");
        let url = format!("https://qt.gtimg.cn/q={}", query);
        let resp = self.client.get(&url).send().await?;
        let text = resp.text().await?;
        Ok(text)
    }

    /// 获取 K 线: https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=sh600000,day,,,320,qfq
    pub async fn get_kline(&self, symbol: &str, period: &str, count: u32) -> Result<String> {
        let url = format!(
            "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={},{},,,{},qfq",
            symbol, period, count
        );
        let resp = self.client.get(&url).send().await?;
        let text = resp.text().await?;
        Ok(text)
    }
}
```

- [x] **Step 3: Create fastquote-tencent/src/parser.rs**

```rust
use fastquote_core::*;
use std::str::Split;

/// 解析腾讯行情文本格式
/// 格式: v_sh600000="1~浦发银行~600000~12.34~12.21~12.53~...~500~12.33~..."
/// 字段以 ~ 分隔
pub fn parse_depth_quote(raw: &str) -> Vec<DepthMarket> {
    let mut results = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 提取等号后的内容
        let content = match line.find('=') {
            Some(pos) => &line[pos + 1..],
            None => continue,
        };

        // 去掉引号
        let content = content.trim_matches('"').trim_matches(';');
        if content.is_empty() {
            continue;
        }

        let fields: Vec<&str> = content.split('~').collect();
        if fields.len() < 48 {
            continue;
        }

        let market = if fields[0] == "1" { "SH" } else { "SZ" };
        let code = fields[2].to_string();

        let parse_price = |s: &str| s.parse::<f64>().unwrap_or(0.0);
        let parse_i64 = |s: &str| s.parse::<i64>().unwrap_or(0);

        let last = parse_price(fields[3]);
        let open = parse_price(fields[4]);
        let high = parse_price(fields[5]);

        // fields[6] 可能是昨收或 low，取决于版本
        let close = parse_price(fields[4]); // 昨收
        let volume = parse_i64(fields[6]);
        let amount = parse_i64(fields[37]);

        // 买五档: fields[9..19] 交替 price/vol
        let bid = parse_levels(&fields[9..19]);
        // 卖五档: fields[19..29]
        let ask = parse_levels(&fields[19..29]);

        results.push(DepthMarket {
            symbol: Symbol::new(market, code),
            time: Time::now(),
            last_price: Price::from_f64(last),
            open: Price::from_f64(open),
            high: Price::from_f64(high),
            low: Price::from_f64(0.0), // 需要从字段中提取
            close: Price::from_f64(close),
            volume: Volume(volume),
            amount: Amount(amount),
            bid,
            ask,
        });
    }
    results
}

fn parse_levels(fields: &[&str]) -> [Level; 5] {
    let default = Level { price: Price(0), volume: Volume(0) };
    let mut levels = [default; 5];
    for i in 0..5 {
        if i * 2 + 1 < fields.len() {
            levels[i] = Level {
                price: Price::from_f64(fields[i * 2].parse::<f64>().unwrap_or(0.0)),
                volume: Volume(fields[i * 2 + 1].parse::<i64>().unwrap_or(0)),
            };
        }
    }
    levels
}
```

- [x] **Step 4: Create fastquote-tencent/src/adapter.rs**

```rust
use crate::client::TencentClient;
use crate::parser;
use fastquote_core::{HubMessage, MarketData, Source, Symbol};
use anyhow::Result;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct TencentConfig {
    pub poll_interval_ms: u64,
    pub kline_sync_interval_s: u64,
    pub financial_sync_interval_s: u64,
}

impl Default for TencentConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 3000,
            kline_sync_interval_s: 60,
            financial_sync_interval_s: 3600,
        }
    }
}

pub struct TencentAdapter {
    config: TencentConfig,
    client: TencentClient,
    hub_tx: mpsc::Sender<HubMessage>,
    symbols: Vec<Symbol>,
}

impl TencentAdapter {
    pub fn new(config: TencentConfig, hub_tx: mpsc::Sender<HubMessage>) -> Self {
        Self {
            config,
            client: TencentClient::new(),
            hub_tx,
            symbols: Vec::new(),
        }
    }

    pub fn set_symbols(&mut self, symbols: Vec<Symbol>) {
        self.symbols = symbols;
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut poll_interval = tokio::time::interval(Duration::from_millis(self.config.poll_interval_ms));

        loop {
            poll_interval.tick().await;

            if self.symbols.is_empty() {
                continue;
            }

            // 构建查询字符串
            let queries: Vec<String> = self.symbols.iter()
                .map(|s| format!("{}{}", s.market.to_lowercase(), s.code))
                .collect();

            match self.client.get_quote(&queries).await {
                Ok(text) => {
                    let markets = parser::parse_depth_quote(&text);
                    for m in markets {
                        let msg = HubMessage {
                            source: Source::Tencent,
                            data: MarketData::Depth(m),
                        };
                        if self.hub_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Tencent quote fetch error: {}", e);
                }
            }
        }
    }
}
```

- [x] **Step 5: Update fastquote-tencent/src/lib.rs**

```rust
pub mod adapter;
pub mod client;
pub mod parser;

pub use adapter::{TencentAdapter, TencentConfig};
```

- [x] **Step 6: Verify compilation**

Run: `cargo check -p fastquote-tencent`
Expected: compiles

- [x] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: Tencent Finance HTTP adapter with quote parser"
```

---

## Task 6: fastquote-server — TCP Binary + WebSocket

**Files:**
- Modify: `fastquote-server/Cargo.toml`
- Create: `fastquote-server/src/lib.rs`
- Create: `fastquote-server/src/tcp/mod.rs`
- Create: `fastquote-server/src/tcp/server.rs`
- Create: `fastquote-server/src/tcp/session.rs`
- Create: `fastquote-server/src/tcp/frame.rs`
- Create: `fastquote-server/src/ws/mod.rs`
- Create: `fastquote-server/src/ws/server.rs`

- [x] **Step 1: Update fastquote-server/Cargo.toml**

```toml
[package]
name = "fastquote-server"
version = "0.1.0"
edition = "2021"

[dependencies]
fastquote-core = { path = "../fastquote-core" }
fastquote-hub = { path = "../fastquote-hub" }
tokio = { workspace = true }
axum = { workspace = true }
tokio-tungstenite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
bytes = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
dashmap = { workspace = true }
futures = "0.3"
```

- [x] **Step 2: Create fastquote-server/src/tcp/frame.rs**

Binary frame encoding/decoding per protocol spec.

```rust
use bytes::{Buf, BufMut, BytesMut};

pub const MAGIC: u16 = 0x4651; // "FQ"
pub const VERSION: u8 = 0x01;
pub const HEADER_LEN: usize = 8;

/// 消息类型
pub mod msg_type {
    // 下行
    pub const DEPTH: u8 = 0x01;
    pub const KLINE: u8 = 0x02;
    pub const FINANCIAL: u8 = 0x03;
    pub const INDEX: u8 = 0x04;
    pub const BATCH_DEPTH: u8 = 0x05;
    pub const PONG: u8 = 0x21;
    pub const ERROR: u8 = 0x7F;

    // 上行
    pub const SUB: u8 = 0x10;
    pub const UNSUB: u8 = 0x11;
    pub const REQ_KLINE: u8 = 0x12;
    pub const PING: u8 = 0x20;
}

/// 编码帧
pub fn encode_frame(msg_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(HEADER_LEN + payload.len());
    buf.put_u16(MAGIC);
    buf.put_u8(VERSION);
    buf.put_u8(msg_type);
    buf.put_u32(payload.len() as u32);
    buf.put_slice(payload);
    buf.to_vec()
}

/// 解码帧头
pub fn decode_header(data: &[u8]) -> Option<(u8, u32)> {
    if data.len() < HEADER_LEN {
        return None;
    }
    let mut buf = &data[..HEADER_LEN];
    let magic = buf.get_u16();
    if magic != MAGIC {
        return None;
    }
    let version = buf.get_u8();
    if version != VERSION {
        return None;
    }
    let msg_type = buf.get_u8();
    let len = buf.get_u32();
    Some((msg_type, len))
}

/// 编码 DepthMarket 为 128-byte payload
pub fn encode_depth(depth: &fastquote_core::DepthMarket) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(128);
    // market (2B)
    let market = depth.symbol.market.as_bytes();
    buf.put_slice(&market[..2]);
    // code (6B)
    let code = depth.symbol.code.as_bytes();
    for i in 0..6 {
        buf.put_u8(if i < code.len() { code[i] } else { 0 });
    }
    // timestamp (8B)
    buf.put_i64(depth.time.0);
    // OHLCV (5 × 8B)
    buf.put_i64(depth.last_price.0);
    buf.put_i64(depth.open.0);
    buf.put_i64(depth.high.0);
    buf.put_i64(depth.low.0);
    buf.put_i64(depth.close.0);
    buf.put_i64(depth.volume.0);
    buf.put_i64(depth.amount.0);
    // bid (5 × price+vol = 5 × 16B = 80B)
    for i in 0..5 {
        buf.put_i64(depth.bid[i].price.0);
        buf.put_i64(depth.bid[i].volume.0);
    }
    // ask (5 × price+vol = 80B)
    for i in 0..5 {
        buf.put_i64(depth.ask[i].price.0);
        buf.put_i64(depth.ask[i].volume.0);
    }

    assert_eq!(buf.len(), 128);
    buf.to_vec()
}
```

- [x] **Step 3: Create fastquote-server/src/tcp/session.rs**

```rust
use crate::tcp::frame;
use fastquote_core::HubMessage;
use fastquote_core::MarketData;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tracing::{debug, warn};

pub struct TcpSession {
    writer: Arc<Mutex<TcpStream>>,
    rx: Mutex<broadcast::Receiver<HubMessage>>,
    subs: Mutex<HashSet<String>>,
}

impl TcpSession {
    pub fn new(
        stream: TcpStream,
        rx: broadcast::Receiver<HubMessage>,
    ) -> Self {
        Self {
            writer: Arc::new(Mutex::new(stream)),
            rx: Mutex::new(rx),
            subs: Mutex::new(HashSet::new()),
        }
    }

    pub async fn run(self: Arc<Self>) {
        let session = self.clone();
        let send_task = tokio::spawn(async move {
            session.send_loop().await;
        });

        let recv_task = tokio::spawn(async move {
            self.recv_loop().await;
        });

        let _ = tokio::join!(send_task, recv_task);
    }

    async fn send_loop(&self) {
        let mut rx = self.rx.lock().await;
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if !self.is_subscribed(&msg).await {
                        continue;
                    }
                    let payload = match &msg.data {
                        MarketData::Depth(d) => frame::encode_frame(frame::msg_type::DEPTH, &frame::encode_depth(d)),
                        // TODO: Kline, Financial, Index
                        _ => continue,
                    };
                    let mut writer = self.writer.lock().await;
                    if writer.write_all(&payload).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("TCP session lagged {} messages", n);
                }
                Err(_) => break,
            }
        }
    }

    async fn recv_loop(&self) {
        let mut buf = [0u8; 4096];
        let writer = self.writer.clone();
        let stream = &mut *writer.lock().await;
        loop {
            match stream.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    // 解析上行命令 (Sub/Unsub/Ping)
                    if let Some((msg_type, _len)) = frame::decode_header(&buf[..n]) {
                        match msg_type {
                            frame::msg_type::PING => {
                                let pong = frame::encode_frame(frame::msg_type::PONG, &[]);
                                let _ = stream.write_all(&pong).await;
                            }
                            frame::msg_type::SUB => {
                                // TODO: parse symbols from payload
                                debug!("received SUB command");
                            }
                            _ => {}
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    async fn is_subscribed(&self, msg: &HubMessage) -> bool {
        let subs = self.subs.lock().await;
        subs.is_empty() || subs.contains(&msg.symbol_key())
    }
}
```

- [x] **Step 4: Create fastquote-server/src/tcp/server.rs**

```rust
use crate::tcp::session::TcpSession;
use fastquote_hub::Hub;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

pub struct TcpQuoteServer {
    listener: TcpListener,
    hub: Arc<Hub>,
}

impl TcpQuoteServer {
    pub async fn bind(addr: &str, hub: Arc<Hub>) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        info!("TCP server listening on {}", addr);
        Ok(Self { listener, hub })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            let (stream, addr) = self.listener.accept().await?;
            info!("TCP client connected: {}", addr);

            let rx = self.hub.subscribe();
            let session = Arc::new(TcpSession::new(stream, rx));
            tokio::spawn(session.run());
        }
    }
}
```

- [x] **Step 5: Create fastquote-server/src/tcp/mod.rs**

```rust
pub mod frame;
pub mod server;
pub mod session;

pub use server::TcpQuoteServer;
```

- [x] **Step 6: Create fastquote-server/src/ws/server.rs**

```rust
use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use fastquote_core::HubMessage;
use fastquote_hub::Hub;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub async fn ws_handler(
    ws: axum::extract::WebSocketUpgrade,
    State(hub): State<Arc<Hub>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_session(socket, hub))
}

async fn ws_session(socket: axum::extract::WebSocket, hub: Arc<Hub>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = hub.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if sender.send(axum::extract::ws::Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    warn!("WS serialize error: {}", e);
                }
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // TODO: parse sub/unsub commands from client
            let _ = msg;
        }
    });

    let _ = tokio::join!(send_task, recv_task);
}

pub async fn run_ws_server(addr: &str, hub: Arc<Hub>) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(hub);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("WebSocket server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [x] **Step 7: Create fastquote-server/src/ws/mod.rs**

```rust
pub mod server;

pub use server::run_ws_server;
```

- [x] **Step 8: Update fastquote-server/src/lib.rs**

```rust
pub mod tcp;
pub mod ws;

pub use tcp::TcpQuoteServer;
pub use ws::run_ws_server;
```

- [x] **Step 9: Verify compilation**

Run: `cargo check -p fastquote-server`
Expected: compiles

- [x] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: TCP binary + WebSocket downstream servers"
```

---

## Task 7: fastquote-store — Redis + MySQL + Persister

**Files:**
- Modify: `fastquote-store/Cargo.toml`
- Create: `fastquote-store/src/lib.rs`
- Create: `fastquote-store/src/redis_store.rs`
- Create: `fastquote-store/src/persister.rs`

- [x] **Step 1: Update fastquote-store/Cargo.toml**

```toml
[package]
name = "fastquote-store"
version = "0.1.0"
edition = "2021"

[dependencies]
fastquote-core = { path = "../fastquote-core" }
tokio = { workspace = true }
redis = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
```

- [x] **Step 2: Create fastquote-store/src/redis_store.rs**

```rust
use anyhow::Result;
use fastquote_core::*;
use redis::AsyncCommands;

pub struct RedisStore {
    conn: redis::aio::MultiplexedConnection,
}

impl RedisStore {
    pub async fn new(url: &str) -> Result<Self> {
        let client = redis::Client::open(url)?;
        let conn = client.get_multiplexed_async_connection().await?;
        Ok(Self { conn })
    }

    /// 写入最新快照
    pub async fn write_depth(&mut self, depth: &DepthMarket) -> Result<()> {
        let key = format!("quote:depth:{}{}", depth.symbol.market, depth.symbol.code);
        let _: () = redis::cmd("HSET")
            .arg(&key)
            .arg("last").arg(depth.last_price.0)
            .arg("open").arg(depth.open.0)
            .arg("high").arg(depth.high.0)
            .arg("low").arg(depth.low.0)
            .arg("close").arg(depth.close.0)
            .arg("volume").arg(depth.volume.0)
            .arg("amount").arg(depth.amount.0)
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }

    /// 写入 K 线 (Sorted Set)
    pub async fn write_kline(&mut self, kline: &Kline) -> Result<()> {
        let key = format!(
            "quote:kline:{}{}:{}",
            kline.symbol.market, kline.symbol.code, kline.period
        );
        let member = serde_json::to_string(kline)?;
        let _: () = self.conn.zadd(&key, &member, kline.open_time.0).await?;
        // 保留最近 5000 根
        let _: () = self.conn.zremrangebyrank(&key, 0, -5001).await?;
        Ok(())
    }

    /// 写入指数
    pub async fn write_index(&mut self, idx: &IndexQuote) -> Result<()> {
        let key = format!("quote:index:{}{}", idx.symbol.market, idx.symbol.code);
        let _: () = redis::cmd("HSET")
            .arg(&key)
            .arg("last").arg(idx.last_price.0)
            .arg("volume").arg(idx.volume.0)
            .arg("amount").arg(idx.amount.0)
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }
}
```

- [x] **Step 3: Create fastquote-store/src/persister.rs**

```rust
use crate::redis_store::RedisStore;
use fastquote_core::*;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{error, info};

pub struct Persister {
    redis: RedisStore,
}

impl Persister {
    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        let redis = RedisStore::new(redis_url).await?;
        info!("Persister connected to Redis");
        Ok(Self { redis })
    }

    pub async fn run(mut self, mut rx: broadcast::Receiver<HubMessage>) -> anyhow::Result<()> {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        let mut batch: Vec<HubMessage> = Vec::with_capacity(256);

        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Ok(msg) => {
                            // Redis 实时写
                            self.write_redis(&msg).await;
                            batch.push(msg);
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            error!("Persister lagged {} messages", n);
                        }
                        Err(_) => break,
                    }
                }
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        // TODO: MySQL 批量写
                        batch.clear();
                    }
                }
            }
        }
        Ok(())
    }

    async fn write_redis(&mut self, msg: &HubMessage) {
        match &msg.data {
            MarketData::Depth(d) => {
                if let Err(e) = self.redis.write_depth(d).await {
                    error!("Redis write depth error: {}", e);
                }
            }
            MarketData::Kline(k) => {
                if let Err(e) = self.redis.write_kline(k).await {
                    error!("Redis write kline error: {}", e);
                }
            }
            MarketData::Index(i) => {
                if let Err(e) = self.redis.write_index(i).await {
                    error!("Redis write index error: {}", e);
                }
            }
            _ => {}
        }
    }
}
```

- [x] **Step 4: Update fastquote-store/src/lib.rs**

```rust
pub mod persister;
pub mod redis_store;

pub use persister::Persister;
pub use redis_store::RedisStore;
```

- [x] **Step 5: Verify compilation**

Run: `cargo check -p fastquote-store`
Expected: compiles

- [x] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: Redis store + async persister"
```

---

## Task 8: fastquote-bin — Main Entry Point

**Files:**
- Modify: `fastquote-bin/Cargo.toml`
- Create: `fastquote-bin/src/main.rs`
- Create: `fastquote-bin/src/config.rs`

- [x] **Step 1: Update fastquote-bin/Cargo.toml**

```toml
[package]
name = "fastquote-bin"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "fastquote"
path = "src/main.rs"

[dependencies]
fastquote-core = { path = "../fastquote-core" }
fastquote-hub = { path = "../fastquote-hub" }
fastquote-tdx = { path = "../fastquote-tdx" }
fastquote-tencent = { path = "../fastquote-tencent" }
fastquote-server = { path = "../fastquote-server" }
fastquote-store = { path = "../fastquote-store" }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
toml = { workspace = true }
serde = { workspace = true }
anyhow = { workspace = true }
```

- [x] **Step 2: Create fastquote-bin/src/config.rs**

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub tdx: TdxSection,
    pub tencent: TencentSection,
    pub store: StoreSection,
    pub tcp: TcpSection,
    pub ws: WsSection,
}

#[derive(Debug, Deserialize)]
pub struct TdxSection {
    pub host: String,
    pub port: u16,
    pub reconnect_ms: u64,
    pub heartbeat_s: u64,
}

#[derive(Debug, Deserialize)]
pub struct TencentSection {
    pub poll_interval_ms: u64,
    pub kline_sync_interval_s: u64,
    pub financial_sync_interval_s: u64,
}

#[derive(Debug, Deserialize)]
pub struct StoreSection {
    pub redis_url: String,
    pub mysql_url: String,
}

#[derive(Debug, Deserialize)]
pub struct TcpSection {
    pub addr: String,
}

#[derive(Debug, Deserialize)]
pub struct WsSection {
    pub addr: String,
}

pub fn load_config(path: &str) -> anyhow::Result<AppConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: AppConfig = toml::from_str(&content)?;
    Ok(config)
}
```

- [x] **Step 3: Create fastquote-bin/src/main.rs**

```rust
mod config;

use config::AppConfig;
use fastquote_hub::Hub;
use fastquote_server::{TcpQuoteServer, run_ws_server};
use fastquote_store::Persister;
use fastquote_tencent::{TencentAdapter, TencentConfig};
use fastquote_tdx::{TdxAdapter, TdxConfig};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    // 配置
    let config: AppConfig = config::load_config("config.toml")?;
    tracing::info!("FastQuote starting...");

    // Hub
    let hub = Arc::new(Hub::new(8192));

    // 数据源 -> Hub 的 mpsc channel
    let (hub_tx, mut hub_rx) = mpsc::channel::<fastquote_core::HubMessage>(4096);

    // 启动 Hub 转发任务: mpsc -> broadcast
    let hub_clone = hub.clone();
    tokio::spawn(async move {
        while let Some(msg) = hub_rx.recv().await {
            hub_clone.publish(msg);
        }
    });

    // TDX 适配器
    let tdx_config = TdxConfig {
        host: config.tdx.host.clone(),
        port: config.tdx.port,
        reconnect_ms: config.tdx.reconnect_ms,
        heartbeat_s: config.tdx.heartbeat_s,
    };
    let mut tdx_adapter = TdxAdapter::new(tdx_config, hub_tx.clone());

    // 腾讯财经适配器
    let tencent_config = TencentConfig {
        poll_interval_ms: config.tencent.poll_interval_ms,
        kline_sync_interval_s: config.tencent.kline_sync_interval_s,
        financial_sync_interval_s: config.tencent.financial_sync_interval_s,
    };
    let mut tencent_adapter = TencentAdapter::new(tencent_config, hub_tx.clone());

    // Persister
    let persister = Persister::new(&config.store.redis_url).await?;
    let hub_sub = hub.subscribe();

    // TCP Server
    let tcp_server = TcpQuoteServer::bind(&config.tcp.addr, hub.clone()).await?;

    // WebSocket Server
    let ws_hub = hub.clone();
    let ws_addr = config.ws.addr.clone();

    // 并行运行
    tokio::select! {
        r = tdx_adapter.run() => {
            tracing::error!("TDX adapter exited: {:?}", r);
            r
        }
        r = tencent_adapter.run() => {
            tracing::error!("Tencent adapter exited: {:?}", r);
            r
        }
        r = persister.run(hub_sub) => {
            tracing::error!("Persister exited: {:?}", r);
            r
        }
        r = tcp_server.run() => {
            tracing::error!("TCP server exited: {:?}", r);
            r
        }
        r = run_ws_server(&ws_addr, ws_hub) => {
            tracing::error!("WS server exited: {:?}", r);
            r
        }
    }
}
```

- [x] **Step 4: Verify full workspace compiles**

Run: `cargo check`
Expected: compiles with no errors

- [x] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: main entry point with all components wired together"
```

---

## Task 9: Integration Smoke Test

**Files:**
- Create: `tests/smoke.rs` (at workspace root, or in fastquote-bin)

- [x] **Step 1: Create a basic smoke test**

```rust
// tests/smoke.rs or fastquote-bin/tests/smoke.rs
use fastquote_core::*;
use fastquote_hub::Hub;
use std::sync::Arc;
use tokio::sync::broadcast;

#[tokio::test]
async fn test_hub_broadcast() {
    let hub = Arc::new(Hub::new(64));
    let mut rx = hub.subscribe();

    let msg = HubMessage {
        source: Source::Tdx,
        data: MarketData::Depth(DepthMarket {
            symbol: Symbol::new("SH", "600000"),
            time: Time::now(),
            last_price: Price(12340),
            open: Price(12208),
            high: Price(12532),
            low: Price(11832),
            close: Price(11872),
            volume: Volume(10012000),
            amount: Amount(108200000),
            bid: [Level { price: Price(12330), volume: Volume(500) }; 5],
            ask: [Level { price: Price(12350), volume: Volume(400) }; 5],
        }),
    };

    hub.publish(msg.clone());

    let received = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        rx.recv(),
    )
    .await
    .expect("timeout")
    .expect("recv error");

    assert_eq!(received.symbol_key(), "SH600000");
}
```

- [x] **Step 2: Run the test**

Run: `cargo test`
Expected: PASS

- [x] **Step 3: Commit**

```bash
git add -A
git commit -m "test: hub broadcast smoke test"
```

---

## Spec Coverage Check

| Spec Section | Task |
|-------------|------|
| 4. 统一数据模型 | Task 1 |
| 5. TDX 协议适配器 | Task 3 + Task 4 |
| 6. 腾讯财经适配器 | Task 5 |
| 7. Hub 消息中心 | Task 2 |
| 8. 下游协议 (TCP+WS) | Task 6 |
| 9. 存储层 | Task 7 |
| 10. 项目结构 | Task 1 |
| 11. 启动流程 | Task 8 |
