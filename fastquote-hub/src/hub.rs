use dashmap::DashMap;
use fastquote_core::HubMessage;
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
