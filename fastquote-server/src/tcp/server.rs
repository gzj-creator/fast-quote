use crate::tcp::session::TcpSession;
use fastquote_hub::Hub;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

pub struct TcpQuoteServer {
    listener: TcpListener,
    hub: Arc<Hub>,
}

impl TcpQuoteServer {
    pub async fn bind(addr: &str, hub: Arc<Hub>) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        info!("TCP quote server listening on {addr}");
        Ok(Self { listener, hub })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            let (stream, addr) = self.listener.accept().await?;
            info!("TCP client connected: {addr}");
            let session = TcpSession::new(stream, self.hub.subscribe());
            tokio::spawn(async move {
                session.run().await;
            });
        }
    }
}
