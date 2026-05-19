# FastQuote 协议设计规范

> 版本: 1.0.0
> 日期: 2026-05-18
> 状态: Draft

本文档定义 FastQuote 行情平台与下游消费者之间的通信协议，供策略/内部服务和前端/可视化团队对接使用。

---

## 1. 协议概览

FastQuote 提供两种下游协议：

| 协议 | 传输层 | 编码 | 延迟 | 目标场景 | 默认端口 |
|------|--------|------|------|----------|---------|
| **TCP Binary** | TCP 长连接 | 二进制 | ~10-50μs | 策略/内部服务 | 9000 |
| **WebSocket** | WebSocket | JSON | ~100-500μs | 前端/可视化 | 9001 |

### 字节序约定

- TCP 二进制协议：**大端序 (Big-Endian / Network Byte Order)**，除非特别标注
- WebSocket JSON 协议：UTF-8 文本

---

## 2. TCP 二进制协议

### 2.1 通用帧格式 (Frame)

所有 TCP 通信（上行/下行）共享同一帧格式：

```
偏移  字段      类型      大小    说明
──────────────────────────────────────────────────────
0x00  Magic    u16       2B     固定值 0x46 0x51 ("FQ")
0x02  Version  u8        1B     协议版本, 当前 0x01
0x03  Type     u8        1B     消息类型 (见下表)
0x04  Length   u32       4B     Payload 长度 (大端序)
0x08  Payload  byte[N]   NB     消息体, 长度 = Length
──────────────────────────────────────────────────────
总帧头: 8 bytes
```

图示：

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     Magic     | Ver |  Type   |           Length              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
│                          Payload                               │
│                          (Length bytes)                        │
│                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 2.2 消息类型定义

#### 下行消息 (Server → Client)

| Type | 名称 | Payload | 说明 |
|------|------|---------|------|
| 0x01 | DepthMarket | [DepthMarketFrame](#31-depthmarketframe) | 行情快照推送 |
| 0x02 | Kline | [KlineFrame](#32-klineframe) | K 线数据推送 |
| 0x03 | Financial | [FinancialFrame](#33-financialframe) | 财务数据推送 |
| 0x04 | IndexQuote | [IndexFrame](#34-indexframe) | 指数/板块推送 |
| 0x05 | BatchDepth | [BatchDepthFrame](#35-batchdepthframe) | 批量行情推送 |
| 0x21 | Pong | [PongFrame](#38-pongframe) | 心跳响应 |
| 0x7F | Error | [ErrorFrame](#39-errorframe) | 错误响应 |

#### 上行消息 (Client → Server)

| Type | 名称 | Payload | 说明 |
|------|------|---------|------|
| 0x10 | Sub | [SubFrame](#36-subframe) | 订阅股票 |
| 0x11 | Unsub | [UnsubFrame](#36-subframe) | 取消订阅 |
| 0x12 | ReqKline | [ReqKlineFrame](#37-reqklineframe) | 请求历史 K 线 |
| 0x20 | Ping | (空 Payload) | 心跳探测 |

### 2.3 连接生命周期

```
Client                                    Server
  │                                         │
  │──────── TCP Connect (port 9000) ───────>│
  │<──────── TCP ACK ──────────────────────│
  │                                         │
  │──────── Sub [SH600000, SZ000001] ─────>│
  │<──────── DepthMarket [SH600000] ───────│  (开始推送)
  │<──────── DepthMarket [SZ000001] ───────│
  │<──────── DepthMarket [SH600000] ───────│  (持续推送)
  │         ...                             │
  │                                         │
  │──────── Ping ─────────────────────────>│  (心跳, 建议 30s)
  │<──────── Pong ─────────────────────────│
  │         ...                             │
  │                                         │
  │──────── Unsub [SH600000] ─────────────>│
  │<──────── DepthMarket [SZ000001] ───────│  (SH600000 不再推送)
  │         ...                             │
  │                                         │
  │──────── TCP Close ────────────────────>│  (或超时断开)
```

**心跳规则：**
- Client 建议 30s 发送一次 Ping
- Server 收到 Ping 立即回复 Pong
- Server 若 90s 未收到任何数据，主动断开连接

---

## 3. Payload 详细定义

### 通用字段编码规则

| 类型 | 编码 | 说明 |
|------|------|------|
| Symbol.market | `[u8; 2]` | ASCII: `S`=0x53, `H`=0x48 → "SH"; `S`=0x53, `Z`=0x5A → "SZ" |
| Symbol.code | `[u8; 6]` | ASCII 数字，不足右补 `0x00`。例: "600000" → `36 30 30 30 30 30` |
| Price | `i64` | 实际价格 × 1000。例: 12.345 → 12345 |
| Volume | `i64` | 股数 |
| Amount | `i64` | 金额，单位: 分 |
| Timestamp | `i64` | Unix 纳秒时间戳 |
| Period | `u32` | K 线周期，单位: 秒 |

### 3.1 DepthMarketFrame

Type = **0x01**

固定 **128 bytes**，无对齐填充：

```
偏移    字段          类型        大小    说明
────────────────────────────────────────────────────────
0x00    market       [u8; 2]     2B     "SH" / "SZ"
0x02    code         [u8; 6]     6B     股票代码
0x08    timestamp    i64         8B     行情时间 (纳秒)
0x10    last_price   i64         8B     最新价 ×1000
0x18    open         i64         8B     开盘价 ×1000
0x20    high         i64         8B     最高价 ×1000
0x28    low          i64         8B     最低价 ×1000
0x30    close        i64         8B     收盘价 ×1000 (昨收)
0x38    volume       i64         8B     成交量 (股)
0x40    amount       i64         8B     成交额 (分)
0x48    bid1_price   i64         8B     买一价 ×1000
0x50    bid1_vol     i64         8B     买一量
0x58    bid2_price   i64         8B     买二价 ×1000
0x60    bid2_vol     i64         8B     买二量
0x68    bid3_price   i64         8B     买三价 ×1000
0x70    bid3_vol     i64         8B     买三量
0x78    bid4_price   i64         8B     买四价 ×1000
0x80    bid4_vol     i64         8B     买四量
0x88    bid5_price   i64         8B     买五价 ×1000
0x90    bid5_vol     i64         8B     买五量
0x98    ask1_price   i64         8B     卖一价 ×1000
0xA0    ask1_vol     i64         8B     卖一量
0xA8    ask2_price   i64         8B     卖二价 ×1000
0xB0    ask2_vol     i64         8B     卖二量
0xB8    ask3_price   i64         8B     卖三价 ×1000
0xC0    ask3_vol     i64         8B     卖三量
0xC8    ask4_price   i64         8B     卖四价 ×1000
0xD0    ask4_vol     i64         8B     卖四量
0xD8    ask5_price   i64         8B     卖五价 ×1000
0xE0    ask5_vol     i64         8B     卖五量
────────────────────────────────────────────────────────
总计: 128 + 4 = 132 bytes (不含帧头)
```

**Hex 示例**（SH600000, 最新价 12.34, 买一 12.33/500股）：

```
帧头:
  46 51 01 01 00 00 00 80    // Magic=FQ, Ver=1, Type=0x01(Depth), Len=128

Payload:
  53 48                      // market: "SH"
  36 30 30 30 30 30          // code: "600000"
  00 00 01 93 28 6B 20 C0    // timestamp: 1716028800000000000 (ns)
  00 00 00 00 00 00 30 34    // last_price: 12340 (= 12.340)
  00 00 00 00 00 00 2F B0    // open: 12208 (= 12.208)
  00 00 00 00 00 00 30 F4    // high: 12532 (= 12.532)
  00 00 00 00 00 00 2E 38    // low: 11832 (= 11.832)
  00 00 00 00 00 00 2E 60    // close: 11872 (= 11.872, 昨收)
  00 00 00 00 00 98 D5 80    // volume: 10012000
  00 00 00 00 06 72 1B E0    // amount: 108200000 (1082万分)
  00 00 00 00 00 00 30 2A    // bid1_price: 12330 (12.330)
  00 00 00 00 00 00 01 F4    // bid1_vol: 500
  ... (后续档位)
```

### 3.2 KlineFrame

Type = **0x02**

固定 **72 bytes**：

```
偏移    字段          类型        大小    说明
────────────────────────────────────────────────────────
0x00    market       [u8; 2]     2B     "SH" / "SZ"
0x02    code         [u8; 6]     6B     股票代码
0x08    open_time    i64         8B     开盘时间 (纳秒)
0x10    open         i64         8B     开盘价 ×1000
0x18    high         i64         8B     最高价 ×1000
0x20    low          i64         8B     最低价 ×1000
0x28    close        i64         8B     收盘价 ×1000
0x30    volume       i64         8B     成交量
0x38    amount       i64         8B     成交额 (分)
0x40    period       u32         4B     周期 (秒): 60/300/900/1800/3600/86400
0x44    reserved     [u8; 28]    28B    预留 (填 0)
────────────────────────────────────────────────────────
总计: 72 bytes
```

**Period 常量：**

| 值 | 含义 |
|----|------|
| 60 | 1 分钟 |
| 300 | 5 分钟 |
| 900 | 15 分钟 |
| 1800 | 30 分钟 |
| 3600 | 1 小时 |
| 86400 | 日线 |

### 3.3 FinancialFrame

Type = **0x03**

变长，以 `\0` 分隔字符串字段：

```
偏移    字段            类型        大小    说明
────────────────────────────────────────────────────────
0x00    market         [u8; 2]     2B     "SH" / "SZ"
0x02    code           [u8; 6]     6B     股票代码
0x08    report_date    [u8; 10]    10B    "YYYY-MM-DD\0"
0x12    eps            f64         8B     每股收益 (IEEE 754)
0x1A    bvps           f64         8B     每股净资产
0x22    roe            f64         8B     净资产收益率 (小数, 如 0.15 = 15%)
0x2A    total_revenue  f64         8B     总营收 (元)
0x32    net_profit     f64         8B     净利润 (元)
0x3A    reserved       [u8; 6]     6B     预留
────────────────────────────────────────────────────────
总计: 64 bytes
```

### 3.4 IndexFrame

Type = **0x04**

固定 **64 bytes**：

```
偏移    字段          类型        大小    说明
────────────────────────────────────────────────────────
0x00    market       [u8; 2]     2B     "SH" / "SZ"
0x02    code         [u8; 6]     6B     指数代码
0x08    timestamp    i64         8B     时间 (纳秒)
0x10    last_price   i64         8B     最新点位 ×1000
0x18    volume       i64         8B     成交量
0x20    amount       i64         8B     成交额 (分)
0x28    change_pct   i64         8B     涨跌幅 ×10000 (如 +1.5% = 15000)
0x30    reserved     [u8; 32]    32B    预留
────────────────────────────────────────────────────────
总计: 64 bytes
```

### 3.5 BatchDepthFrame

Type = **0x05**

多只股票行情批量推送，减少小包数量：

```
偏移    字段          类型        大小        说明
──────────────────────────────────────────────────────
0x00    count        u16         2B         DepthMarketFrame 数量 N
0x02    frames       byte[]      N × 128B   连续的 DepthMarketFrame
──────────────────────────────────────────────────────
Payload = 2 + N × 128 bytes
```

### 3.6 SubFrame / UnsubFrame

Type = **0x10** (Sub) / **0x11** (Unsub)

```
偏移    字段          类型        大小        说明
──────────────────────────────────────────────────────
0x00    count        u16         2B         Symbol 数量 N
0x02    symbols      byte[]      N × 8B     连续的 Symbol 编码
──────────────────────────────────────────────────────

Symbol 编码 (8 bytes):
  [0..2]  market: [u8; 2]    "SH" / "SZ"
  [2..8]  code:   [u8; 6]    "600000"
```

**示例**（订阅 SH600000 和 SZ000001）：

```
帧头:
  46 51 01 10 00 00 00 12    // Magic=FQ, Ver=1, Type=0x10(Sub), Len=18

Payload:
  00 02                      // count: 2
  53 48 36 30 30 30 30 30    // SH 600000
  53 5A 30 30 30 30 30 31    // SZ 000001
```

### 3.7 ReqKlineFrame

Type = **0x12**

请求历史 K 线数据，Server 会以多个 KlineFrame (Type=0x02) 回复：

```
偏移    字段          类型        大小    说明
────────────────────────────────────────────────────────
0x00    market       [u8; 2]     2B     "SH" / "SZ"
0x02    code         [u8; 6]     6B     股票代码
0x08    period       u32         4B     周期 (秒)
0x0C    count        u16         2B     请求根数 (最多 1000)
0x0E    end_time     i64         8B     结束时间 (纳秒), 0 = 最新
0x16    reserved     [u8; 2]     2B     预留
────────────────────────────────────────────────────────
总计: 24 bytes
```

### 3.8 PongFrame

Type = **0x21**

```
偏移    字段          类型        大小    说明
────────────────────────────────────────────────────────
0x00    server_ts    i64         8B     服务器当前时间 (纳秒)
────────────────────────────────────────────────────────
总计: 8 bytes
```

### 3.9 ErrorFrame

Type = **0x7F**

```
偏移    字段          类型        大小    说明
────────────────────────────────────────────────────────
0x00    error_code   u16         2B     错误码 (见下表)
0x02    msg_len      u16         2B     错误消息长度
0x04    message      [u8; N]     NB     UTF-8 错误消息
────────────────────────────────────────────────────────
```

**错误码：**

| Code | 名称 | 说明 |
|------|------|------|
| 0x0001 | INVALID_FRAME | 帧格式错误 (Magic/Version 不匹配) |
| 0x0002 | UNKNOWN_TYPE | 未知消息类型 |
| 0x0003 | INVALID_SYMBOL | 股票代码格式错误 |
| 0x0004 | TOO_MANY_SUBS | 订阅数量超限 (单连接最多 5000 只) |
| 0x0005 | KLINE_LIMIT | K 线请求数量超限 (单次最多 1000 根) |
| 0x00FF | INTERNAL | 服务内部错误 |

---

## 4. WebSocket 协议

### 4.1 连接

```
ws://<host>:9001/ws
```

连接建立后即可收发 JSON 消息。

### 4.2 下行消息 (Server → Client)

#### DepthMarket (type = "depth")

```json
{
  "type": "depth",
  "symbol": { "market": "SH", "code": "600000" },
  "data": {
    "time": 1716028800000,
    "last": 12.340,
    "open": 12.208,
    "high": 12.532,
    "low": 11.832,
    "close": 11.872,
    "volume": 10012000,
    "amount": 108200000,
    "bid": [
      { "price": 12.330, "vol": 500 },
      { "price": 12.320, "vol": 1200 },
      { "price": 12.310, "vol": 800 },
      { "price": 12.300, "vol": 3000 },
      { "price": 12.290, "vol": 1500 }
    ],
    "ask": [
      { "price": 12.350, "vol": 400 },
      { "price": 12.360, "vol": 900 },
      { "price": 12.370, "vol": 600 },
      { "price": 12.380, "vol": 2000 },
      { "price": 12.390, "vol": 1100 }
    ]
  }
}
```

#### Kline (type = "kline")

```json
{
  "type": "kline",
  "symbol": { "market": "SH", "code": "600000" },
  "data": {
    "open_time": 1716028800000,
    "open": 12.208,
    "high": 12.532,
    "low": 11.832,
    "close": 12.340,
    "volume": 10012000,
    "amount": 108200000,
    "period": 60
  }
}
```

#### Financial (type = "financial")

```json
{
  "type": "financial",
  "symbol": { "market": "SH", "code": "600000" },
  "data": {
    "report_date": "2025-12-31",
    "eps": 1.23,
    "bvps": 15.67,
    "roe": 0.0785,
    "total_revenue": 50000000000,
    "net_profit": 5000000000
  }
}
```

#### Index (type = "index")

```json
{
  "type": "index",
  "symbol": { "market": "SH", "code": "000001" },
  "data": {
    "time": 1716028800000,
    "last": 3125.670,
    "volume": 250000000,
    "amount": 35000000000,
    "change_pct": 1.52
  }
}
```

### 4.3 上行消息 (Client → Server)

#### 订阅

```json
{ "cmd": "sub", "symbols": ["SH600000", "SZ000001"] }
```

#### 取消订阅

```json
{ "cmd": "unsub", "symbols": ["SH600000"] }
```

#### 请求 K 线

```json
{
  "cmd": "req_kline",
  "symbol": { "market": "SH", "code": "600000" },
  "period": 300,
  "count": 100
}
```

Server 返回多条 Kline 消息。

#### 心跳

```json
{ "cmd": "ping" }
```

Server 回复：

```json
{ "cmd": "pong", "server_time": 1716028800000 }
```

### 4.4 错误响应

```json
{
  "type": "error",
  "code": 4,
  "message": "too many subscriptions, max 5000"
}
```

---

## 5. Symbol 编码规范

### 市场代码

| 市场 | TCP 编码 | WS 编码 |
|------|---------|---------|
| 上海证券交易所 | `"SH"` (0x53 0x48) | `"SH"` |
| 深圳证券交易所 | `"SZ"` (0x53 0x5A) | `"SZ"` |

### 股票代码

- 6 位数字字符串
- TCP: 固定 6 bytes ASCII，不足右补 `0x00`
- WS: 普通字符串 `"600000"`

### 复合 Key 格式

在订阅/过滤中使用 `{market}{code}` 格式：

```
"SH600000"    // 浦发银行
"SZ000001"    // 平安银行
"SH000001"    // 上证指数
"SZ399001"    // 深证成指
```

---

## 6. 价格/数值编码规范

### 价格 (Price)

所有价格字段以 **i64** 存储，值为 **实际价格 × 1000**。

| 实际价格 | 编码值 | 说明 |
|---------|--------|------|
| 12.345 | 12345 | |
| 0.010 | 10 | |
| 100.000 | 100000 | |
| 3125.670 | 3125670 | 指数点位 |

**解码公式**: `real_price = encoded_value / 1000.0`

### 涨跌幅 (change_pct)

编码为 **i64**，值为 **实际百分比 × 10000**。

| 实际涨跌幅 | 编码值 |
|-----------|--------|
| +1.50% | 15000 |
| -0.82% | -8200 |
| +10.00% | 100000 |

**解码公式**: `real_pct = encoded_value / 10000.0`

### 成交额 (Amount)

单位: **分**（整数，无小数）。

### 时间戳

单位: **Unix 纳秒** (i64)。

**毫秒转换**: `ms = ns / 1_000_000`

---

## 7. 限流与约束

| 参数 | TCP | WebSocket |
|------|-----|-----------|
| 单连接最大订阅数 | 5000 | 5000 |
| K 线单次请求上限 | 1000 根 | 1000 根 |
| 心跳超时 | 90s | 90s |
| 推送频率 | ~3s/次 (TDX 快照) | ~3s/次 |
| 单帧最大 Payload | 1 MB | 1 MB |

---

## 8. 版本兼容

帧头中的 `Version` 字段用于协议版本协商。当前版本为 **0x01**。

- Server 检测到 Version 不匹配时，回复 ErrorFrame (code=0x0001) 并断开连接
- 新版本只加字段不删字段，旧字段偏移不变，新字段追加到 reserved 区域
- `reserved` 区域当前填 0，接收方应忽略其内容

---

## 附录 A: 快速对接 Checklist

### TCP Binary 对接

- [ ] 建立 TCP 连接到 `host:9000`
- [ ] 实现帧解析器：读 8 bytes 头 → 解析 Length → 读 Length bytes Payload
- [ ] 发送 SubFrame 订阅股票
- [ ] 循环读取下行帧，按 Type 分发解析
- [ ] 价格解码: `real = value / 1000.0`
- [ ] 实现 30s Ping 心跳

### WebSocket 对接

- [ ] 连接 `ws://host:9001/ws`
- [ ] 发送 `{"cmd":"sub","symbols":["SH600000"]}`
- [ ] 接收 JSON 消息，按 `type` 字段分发
- [ ] 实现 30s `{"cmd":"ping"}` 心跳
