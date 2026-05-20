mod config;

use config::AppConfig;
use fastquote_core::{Adapter, Symbol};
use fastquote_hub::Hub;
use fastquote_server::{run_ws_server, TcpQuoteServer};
use fastquote_store::Persister;
use fastquote_tdx::{TdxAdapter, TdxConfig};
use fastquote_tencent::{TencentAdapter, TencentConfig};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

fn parse_symbols(raw: &[String]) -> Vec<Symbol> {
    raw.iter()
        .filter_map(|s| {
            let s = s.trim();
            if s.len() < 4 {
                return None;
            }
            let (market, code) = s.split_at(2);
            Some(Symbol::new(market.to_uppercase(), code))
        })
        .collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let config: AppConfig = config::load_config("config/config.toml")?;
    tracing::info!("fast-quote starting");

    let hub = Arc::new(Hub::new(8192));
    let (hub_tx, mut hub_rx) = mpsc::channel::<fastquote_core::HubMessage>(4096);

    let hub_forwarder = hub.clone();
    tokio::spawn(async move {
        while let Some(msg) = hub_rx.recv().await {
            hub_forwarder.publish(msg);
        }
    });

    let tdx_adapter = TdxAdapter::new(
        TdxConfig {
            host: config.tdx.host.clone(),
            port: config.tdx.port,
            reconnect_ms: config.tdx.reconnect_ms,
            heartbeat_s: config.tdx.heartbeat_s,
        },
        hub_tx.clone(),
    );

    let tencent_adapter = TencentAdapter::new(
        TencentConfig {
            poll_interval_ms: config.tencent.poll_interval_ms,
            kline_sync_interval_s: config.tencent.kline_sync_interval_s,
            financial_sync_interval_s: config.tencent.financial_sync_interval_s,
        },
        hub_tx,
    );

    let symbols = parse_symbols(&config.symbols);
    if !symbols.is_empty() {
        tracing::info!("Subscribing {} symbols", symbols.len());
        tencent_adapter.subscribe(&symbols).await?;
    }

    let persister = Persister::new(&config.store.redis_url, config.store.database_url()).await;
    let hub_sub = hub.subscribe();
    let tcp_server = TcpQuoteServer::bind(&config.tcp.addr, hub.clone()).await?;
    let ws_addr = config.ws.addr.clone();
    let ws_hub = hub.clone();

    tokio::select! {
        result = tdx_adapter.run() => {
            tracing::error!("TDX adapter exited: {result:?}");
            result
        }
        result = tencent_adapter.run() => {
            tracing::error!("Tencent adapter exited: {result:?}");
            result
        }
        result = persister.run(hub_sub) => {
            tracing::error!("Persister exited: {result:?}");
            result
        }
        result = tcp_server.run() => {
            tracing::error!("TCP server exited: {result:?}");
            result
        }
        result = run_ws_server(&ws_addr, ws_hub) => {
            tracing::error!("WS server exited: {result:?}");
            result
        }
    }
}
