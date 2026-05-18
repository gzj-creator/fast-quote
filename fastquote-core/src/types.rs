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

/// 价格 x1000（避免浮点误差）
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
