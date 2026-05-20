use crate::{redis_store::RedisStore, OrmStore};
use fastquote_core::{HubMessage, MarketData};
use tokio::sync::broadcast;
use tracing::{error, info};

pub struct Persister {
    redis: Option<RedisStore>,
    database: Option<OrmStore>,
}

impl Persister {
    pub async fn new(redis_url: &str, database_url: &str) -> Self {
        let redis = match RedisStore::new(redis_url).await {
            Ok(redis) => {
                info!("Persister connected to Redis");
                Some(redis)
            }
            Err(err) => {
                tracing::warn!("Persister: Redis unavailable ({err}), Redis persistence disabled");
                None
            }
        };
        let database = match OrmStore::new(database_url).await {
            Ok(database) => {
                info!("Persister connected to database");
                Some(database)
            }
            Err(err) => {
                tracing::warn!(
                    "Persister: database unavailable ({err}), relational persistence disabled"
                );
                None
            }
        };

        Self { redis, database }
    }

    pub async fn run(mut self, mut rx: broadcast::Receiver<HubMessage>) -> anyhow::Result<()> {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if self.redis.is_some() {
                        self.write_redis(&msg).await;
                    }
                    if self.database.is_some() {
                        self.write_database(&msg).await;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    error!("Persister lagged {count} messages");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }

        Ok(())
    }

    async fn write_redis(&mut self, msg: &HubMessage) {
        let redis = match &mut self.redis {
            Some(r) => r,
            None => return,
        };
        match &msg.data {
            MarketData::Depth(depth) => {
                if let Err(err) = redis.write_depth(depth).await {
                    error!("Redis write depth error: {err}");
                }
            }
            MarketData::Kline(kline) => {
                if let Err(err) = redis.write_kline(kline).await {
                    error!("Redis write kline error: {err}");
                }
            }
            MarketData::Index(index) => {
                if let Err(err) = redis.write_index(index).await {
                    error!("Redis write index error: {err}");
                }
            }
            MarketData::Financial(_) => {}
        }
    }

    async fn write_database(&mut self, msg: &HubMessage) {
        let database = match &self.database {
            Some(s) => s,
            None => return,
        };
        let source = format!("{:?}", msg.source);
        match &msg.data {
            MarketData::Depth(depth) => {
                if let Err(err) = database.write_depth(&source, depth).await {
                    error!("Database write depth error: {err}");
                }
            }
            MarketData::Kline(kline) => {
                if let Err(err) = database.write_kline(&source, kline).await {
                    error!("Database write kline error: {err}");
                }
            }
            MarketData::Index(index) => {
                if let Err(err) = database.write_index(&source, index).await {
                    error!("Database write index error: {err}");
                }
            }
            MarketData::Financial(_) => {}
        }
    }
}
