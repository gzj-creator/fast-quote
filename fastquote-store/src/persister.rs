use crate::redis_store::RedisStore;
use fastquote_core::{HubMessage, MarketData};
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
        let mut batch = Vec::with_capacity(256);

        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Ok(msg) => {
                            self.write_redis(&msg).await;
                            batch.push(msg);
                        }
                        Err(broadcast::error::RecvError::Lagged(count)) => {
                            error!("Persister lagged {count} messages");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        // MySQL batch persistence belongs in the next storage expansion.
                        batch.clear();
                    }
                }
            }
        }

        Ok(())
    }

    async fn write_redis(&mut self, msg: &HubMessage) {
        match &msg.data {
            MarketData::Depth(depth) => {
                if let Err(err) = self.redis.write_depth(depth).await {
                    error!("Redis write depth error: {err}");
                }
            }
            MarketData::Kline(kline) => {
                if let Err(err) = self.redis.write_kline(kline).await {
                    error!("Redis write kline error: {err}");
                }
            }
            MarketData::Index(index) => {
                if let Err(err) = self.redis.write_index(index).await {
                    error!("Redis write index error: {err}");
                }
            }
            MarketData::Financial(_) => {}
        }
    }
}
