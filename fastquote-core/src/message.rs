use crate::model::*;
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
