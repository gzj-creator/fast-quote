use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use fastquote_hub::Hub;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tracing::{info, warn};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(hub): State<Arc<Hub>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_session(socket, hub))
}

async fn ws_session(socket: WebSocket, hub: Arc<Hub>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = hub.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(err) => warn!("WS serialize error: {err}"),
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // Subscription commands are accepted by the TCP path first; WS filtering
            // can be added without changing the outbound message schema.
        }
    });

    let _ = tokio::join!(send_task, recv_task);
}

pub fn router(hub: Arc<Hub>) -> Router {
    Router::new().route("/ws", get(ws_handler)).with_state(hub)
}

pub async fn run_ws_server(addr: &str, hub: Arc<Hub>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("WebSocket quote server listening on {addr}");
    axum::serve(listener, router(hub)).await?;
    Ok(())
}
