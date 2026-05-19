use crate::client::TencentClient;
use crate::parser;
use anyhow::Result;
use fastquote_core::{Adapter, HubMessage, MarketData, Source, Symbol};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::error;

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
    symbols: Arc<RwLock<Vec<Symbol>>>,
    stopped: Arc<AtomicBool>,
}

impl TencentAdapter {
    pub fn new(config: TencentConfig, hub_tx: mpsc::Sender<HubMessage>) -> Self {
        Self {
            config,
            client: TencentClient::new(),
            hub_tx,
            symbols: Arc::new(RwLock::new(Vec::new())),
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn set_symbols(&self, symbols: Vec<Symbol>) {
        *self.symbols.write().await = symbols;
    }

    pub async fn handle_quote_text(&self, text: &str) -> Result<()> {
        for depth in parser::parse_depth_quote(text) {
            let msg = HubMessage {
                source: Source::Tencent,
                data: MarketData::Depth(depth),
            };
            if self.hub_tx.send(msg).await.is_err() {
                break;
            }
        }
        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        self.stopped.store(false, Ordering::SeqCst);
        let mut interval = tokio::time::interval(Duration::from_millis(self.config.poll_interval_ms));

        while !self.stopped.load(Ordering::SeqCst) {
            interval.tick().await;
            let symbols = self.symbols.read().await.clone();
            if symbols.is_empty() {
                continue;
            }

            let queries = symbols
                .iter()
                .map(|symbol| format!("{}{}", symbol.market.to_lowercase(), symbol.code))
                .collect::<Vec<_>>();

            match self.client.get_quote(&queries).await {
                Ok(text) => self.handle_quote_text(&text).await?,
                Err(err) => error!("Tencent quote fetch error: {err}"),
            }
        }

        Ok(())
    }
}

impl Adapter for TencentAdapter {
    fn start(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move { self.run().await }
    }

    fn stop(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.stopped.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    fn subscribe(&self, symbols: &[Symbol]) -> impl std::future::Future<Output = Result<()>> + Send {
        let symbols = symbols.to_vec();
        async move {
            self.set_symbols(symbols).await;
            Ok(())
        }
    }
}
