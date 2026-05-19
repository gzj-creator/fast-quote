use crate::connection::{TdxConfig, TdxConnection};
use crate::protocol::Decoder;
use anyhow::Result;
use fastquote_core::{Adapter, HubMessage, MarketData, Source, Symbol};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

pub struct TdxAdapter {
    config: TdxConfig,
    hub_tx: mpsc::Sender<HubMessage>,
    symbols: Arc<RwLock<Vec<Symbol>>>,
    stopped: Arc<AtomicBool>,
}

impl TdxAdapter {
    pub fn new(config: TdxConfig, hub_tx: mpsc::Sender<HubMessage>) -> Self {
        Self {
            config,
            hub_tx,
            symbols: Arc::new(RwLock::new(Vec::new())),
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn run(&self) -> Result<()> {
        self.stopped.store(false, Ordering::SeqCst);

        while !self.stopped.load(Ordering::SeqCst) {
            let mut conn = TdxConnection::new(self.config.clone());
            if let Err(err) = conn.connect_and_login().await {
                error!("TDX connection failed: {err}");
                continue;
            }

            let symbols = self.symbols.read().await.clone();
            if !symbols.is_empty() {
                if let Err(err) = conn.subscribe(&symbols).await {
                    error!("TDX subscribe failed: {err}");
                    continue;
                }
            }

            loop {
                if self.stopped.load(Ordering::SeqCst) {
                    return Ok(());
                }

                match conn.recv_packet().await {
                    Ok(Some((pkt_type, body))) => self.handle_packet(pkt_type, &body).await?,
                    Ok(None) => {
                        info!("TDX connection closed, reconnecting");
                        break;
                    }
                    Err(err) => {
                        error!("TDX recv error: {err}");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn handle_packet(&self, pkt_type: u16, body: &[u8]) -> Result<()> {
        use crate::protocol::types::pkt_type;

        match pkt_type {
            pkt_type::QUOTE_PUSH => {
                for depth in Decoder::decode_depth(body)? {
                    let msg = HubMessage {
                        source: Source::Tdx,
                        data: MarketData::Depth(depth),
                    };
                    let _ = self.hub_tx.send(msg).await;
                }
            }
            pkt_type::KLINE_RESP => {
                // K-line decoding is handled in a later protocol expansion.
            }
            _ => {}
        }

        Ok(())
    }
}

impl Adapter for TdxAdapter {
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
            *self.symbols.write().await = symbols;
            Ok(())
        }
    }
}
