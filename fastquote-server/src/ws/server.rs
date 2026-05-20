use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use fastquote_core::{HubMessage, Kline, MarketData, Source, Symbol};
use fastquote_hub::Hub;
use fastquote_tencent::{
    client::TencentClient,
    parser::{parse_kline_response, tencent_period},
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn};

const DEFAULT_KLINE_PERIOD: u32 = 60;
const DEFAULT_KLINE_COUNT: u32 = 120;
const MAX_KLINE_COUNT: u32 = 1000;

#[derive(Debug, Clone)]
struct KlineRequest {
    symbol: Symbol,
    period: u32,
    count: u32,
}

#[derive(Debug, Deserialize)]
struct RawKlineRequest {
    cmd: String,
    symbol: Symbol,
    period: Option<u32>,
    count: Option<u32>,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_session(socket, hub))
}

async fn ws_session(socket: WebSocket, hub: Arc<Hub>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = hub.subscribe();
    let (direct_tx, mut direct_rx) = mpsc::channel::<String>(128);

    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(json) = direct_rx.recv() => {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                result = rx.recv() => {
                    match result {
                        Ok(msg) => match serde_json::to_string(&msg) {
                            Ok(json) => {
                                if sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                            Err(err) => warn!("WS serialize error: {err}"),
                        },
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("WS receiver lagged and skipped {skipped} messages");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                else => break,
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        let client = TencentClient::new();
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                handle_text_message(text.as_str(), &client, &direct_tx).await;
            }
        }
    });

    let _ = tokio::join!(send_task, recv_task);
}

async fn handle_text_message(text: &str, client: &TencentClient, direct_tx: &mpsc::Sender<String>) {
    let Some(req) = parse_kline_request(text) else {
        return;
    };

    if let Err(err) = send_kline_history(req, client, direct_tx).await {
        warn!("WS kline request failed: {err}");
    }
}

async fn send_kline_history(
    req: KlineRequest,
    client: &TencentClient,
    direct_tx: &mpsc::Sender<String>,
) -> anyhow::Result<()> {
    let period = tencent_period(req.period)
        .ok_or_else(|| anyhow::anyhow!("unsupported kline period {}", req.period))?;
    let symbol = format!("{}{}", req.symbol.market.to_lowercase(), req.symbol.code);
    let raw = client.get_kline(&symbol, period, req.count).await?;
    let klines = parse_kline_response(&raw, &req.symbol, req.period)?;

    for json in kline_json_messages(klines)? {
        if direct_tx.send(json).await.is_err() {
            break;
        }
    }

    Ok(())
}

fn parse_kline_request(text: &str) -> Option<KlineRequest> {
    let raw = serde_json::from_str::<RawKlineRequest>(text).ok()?;
    if raw.cmd != "req_kline" {
        return None;
    }

    Some(KlineRequest {
        symbol: raw.symbol,
        period: raw.period.unwrap_or(DEFAULT_KLINE_PERIOD),
        count: raw
            .count
            .unwrap_or(DEFAULT_KLINE_COUNT)
            .clamp(1, MAX_KLINE_COUNT),
    })
}

fn kline_json_messages(klines: Vec<Kline>) -> serde_json::Result<Vec<String>> {
    klines
        .into_iter()
        .map(|kline| {
            serde_json::to_string(&HubMessage {
                source: Source::Tencent,
                data: MarketData::Kline(kline),
            })
        })
        .collect()
}

pub fn router(hub: Arc<Hub>) -> Router {
    Router::new()
        .route("/", get(crate::frontend::index))
        .route("/ws", get(ws_handler))
        .with_state(hub)
}

pub async fn run_ws_server(addr: &str, hub: Arc<Hub>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("WebSocket quote server listening on {addr}");
    axum::serve(listener, router(hub)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{kline_json_messages, parse_kline_request};
    use fastquote_core::{Amount, Kline, Price, Symbol, Time, Volume};

    #[test]
    fn parses_req_kline_command_with_limits() {
        let req = parse_kline_request(
            r#"{
                "cmd": "req_kline",
                "symbol": { "market": "SH", "code": "600519" },
                "period": 60,
                "count": 5000
            }"#,
        )
        .unwrap();

        assert_eq!(req.symbol, Symbol::new("SH", "600519"));
        assert_eq!(req.period, 60);
        assert_eq!(req.count, 1000);
    }

    #[test]
    fn serializes_kline_messages_for_websocket() {
        let messages = kline_json_messages(vec![Kline {
            symbol: Symbol::new("SH", "600519"),
            open_time: Time(1),
            open: Price(1_000),
            high: Price(1_200),
            low: Price(900),
            close: Price(1_100),
            volume: Volume(10),
            amount: Amount(0),
            period: 60,
        }])
        .unwrap();

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains(r#""source":"Tencent""#));
        assert!(messages[0].contains(r#""Kline""#));
    }
}
