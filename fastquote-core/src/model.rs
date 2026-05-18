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
