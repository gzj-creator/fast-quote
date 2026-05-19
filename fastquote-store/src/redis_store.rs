use anyhow::Result;
use fastquote_core::{DepthMarket, IndexQuote, Kline, Symbol};
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
        let key = depth_key(&depth.symbol);
        let _: () = redis::cmd("HSET")
            .arg(&key)
            .arg("last")
            .arg(depth.last_price.0)
            .arg("open")
            .arg(depth.open.0)
            .arg("high")
            .arg(depth.high.0)
            .arg("low")
            .arg(depth.low.0)
            .arg("close")
            .arg(depth.close.0)
            .arg("volume")
            .arg(depth.volume.0)
            .arg("amount")
            .arg(depth.amount.0)
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }

    /// 写入 K 线，保留最近 5000 根。
    pub async fn write_kline(&mut self, kline: &Kline) -> Result<()> {
        let key = kline_key(&kline.symbol, kline.period);
        let member = serde_json::to_string(kline)?;
        let _: () = self.conn.zadd(&key, &member, kline.open_time.0).await?;
        let _: () = self.conn.zremrangebyrank(&key, 0, -5001).await?;
        Ok(())
    }

    /// 写入指数
    pub async fn write_index(&mut self, idx: &IndexQuote) -> Result<()> {
        let key = index_key(&idx.symbol);
        let _: () = redis::cmd("HSET")
            .arg(&key)
            .arg("last")
            .arg(idx.last_price.0)
            .arg("volume")
            .arg(idx.volume.0)
            .arg("amount")
            .arg(idx.amount.0)
            .arg("change_pct")
            .arg(idx.change_pct)
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }
}

pub fn depth_key(symbol: &Symbol) -> String {
    format!("quote:depth:{}{}", symbol.market, symbol.code)
}

pub fn kline_key(symbol: &Symbol, period: u32) -> String {
    format!("quote:kline:{}{}:{period}", symbol.market, symbol.code)
}

pub fn index_key(symbol: &Symbol) -> String {
    format!("quote:index:{}{}", symbol.market, symbol.code)
}
