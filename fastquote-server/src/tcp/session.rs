use crate::tcp::frame;
use fastquote_core::{HubMessage, MarketData};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use tracing::warn;

pub struct TcpSession {
    reader: Mutex<OwnedReadHalf>,
    writer: Arc<Mutex<OwnedWriteHalf>>,
    rx: Mutex<broadcast::Receiver<HubMessage>>,
    subs: Mutex<HashSet<String>>,
}

impl TcpSession {
    pub fn new(stream: TcpStream, rx: broadcast::Receiver<HubMessage>) -> Self {
        let (reader, writer) = stream.into_split();
        Self {
            reader: Mutex::new(reader),
            writer: Arc::new(Mutex::new(writer)),
            rx: Mutex::new(rx),
            subs: Mutex::new(HashSet::new()),
        }
    }

    pub async fn run(self) {
        let session = Arc::new(self);
        let mut send_task = tokio::spawn({
            let session = session.clone();
            async move { session.send_loop().await }
        });
        let mut recv_task = tokio::spawn({
            let session = session.clone();
            async move { session.recv_loop().await }
        });

        tokio::select! {
            _ = &mut send_task => recv_task.abort(),
            _ = &mut recv_task => send_task.abort(),
        }
    }

    async fn send_loop(&self) {
        let mut rx = self.rx.lock().await;
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if !self.is_subscribed(&msg).await {
                        continue;
                    }

                    let frame = match &msg.data {
                        MarketData::Depth(depth) => {
                            frame::encode_frame(frame::msg_type::DEPTH, &frame::encode_depth(depth))
                        }
                        _ => continue,
                    };

                    if self.writer.lock().await.write_all(&frame).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    warn!("TCP session lagged {count} messages");
                }
                Err(_) => break,
            }
        }
    }

    async fn recv_loop(&self) {
        loop {
            let Some((msg_type, payload)) = self.read_frame().await else {
                break;
            };

            match msg_type {
                frame::msg_type::PING => {
                    let pong = frame::encode_frame(frame::msg_type::PONG, &[]);
                    if self.writer.lock().await.write_all(&pong).await.is_err() {
                        break;
                    }
                }
                frame::msg_type::SUB => self.add_subscriptions(&payload).await,
                frame::msg_type::UNSUB => self.remove_subscriptions(&payload).await,
                _ => {}
            }
        }
    }

    async fn read_frame(&self) -> Option<(u8, Vec<u8>)> {
        let mut reader = self.reader.lock().await;
        let mut header = [0u8; frame::HEADER_LEN];
        if reader.read_exact(&mut header).await.is_err() {
            return None;
        }

        let (msg_type, len) = frame::decode_header(&header)?;
        let len = len as usize;
        if len > 1024 * 1024 {
            return None;
        }

        let mut payload = vec![0u8; len];
        if len > 0 && reader.read_exact(&mut payload).await.is_err() {
            return None;
        }
        Some((msg_type, payload))
    }

    async fn add_subscriptions(&self, payload: &[u8]) {
        if let Ok(text) = std::str::from_utf8(payload) {
            let mut subs = self.subs.lock().await;
            for symbol in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                subs.insert(symbol.to_string());
            }
        }
    }

    async fn remove_subscriptions(&self, payload: &[u8]) {
        if let Ok(text) = std::str::from_utf8(payload) {
            let mut subs = self.subs.lock().await;
            for symbol in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                subs.remove(symbol);
            }
        }
    }

    async fn is_subscribed(&self, msg: &HubMessage) -> bool {
        let subs = self.subs.lock().await;
        subs.is_empty() || subs.contains(&msg.symbol_key())
    }
}
