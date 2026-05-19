use crate::protocol::Encoder;
use anyhow::{bail, Result};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time;
use tracing::{error, info, warn};

/// TDX 连接配置
#[derive(Debug, Clone)]
pub struct TdxConfig {
    pub host: String,
    pub port: u16,
    pub reconnect_ms: u64,
    pub heartbeat_s: u64,
}

impl Default for TdxConfig {
    fn default() -> Self {
        Self {
            host: "119.147.212.81".into(),
            port: 7709,
            reconnect_ms: 3000,
            heartbeat_s: 60,
        }
    }
}

/// TDX 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connected,
    LoggedIn,
    Subscribed,
}

/// TDX TCP 连接管理器
pub struct TdxConnection {
    config: TdxConfig,
    stream: Option<TcpStream>,
    state: ConnectionState,
}

impl TdxConnection {
    pub fn new(config: TdxConfig) -> Self {
        Self {
            config,
            stream: None,
            state: ConnectionState::Disconnected,
        }
    }

    /// TCP 连接 + 登录，失败后按配置间隔重连。
    pub async fn connect_and_login(&mut self) -> Result<()> {
        loop {
            match self.try_connect_and_login().await {
                Ok(()) => {
                    info!(
                        "TDX connected and logged in to {}:{}",
                        self.config.host, self.config.port
                    );
                    return Ok(());
                }
                Err(err) => {
                    error!(
                        "TDX connect/login failed: {err}, retrying in {}ms",
                        self.config.reconnect_ms
                    );
                    time::sleep(Duration::from_millis(self.config.reconnect_ms)).await;
                }
            }
        }
    }

    async fn try_connect_and_login(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let stream = TcpStream::connect(&addr).await?;
        stream.set_nodelay(true)?;
        self.stream = Some(stream);
        self.state = ConnectionState::Connected;

        self.send_raw(&Encoder::login()).await?;
        let Some((_pkt_type, _body)) = self.recv_packet().await? else {
            bail!("login response parse failed");
        };
        self.state = ConnectionState::LoggedIn;
        Ok(())
    }

    /// 发送订阅请求
    pub async fn subscribe(&mut self, symbols: &[fastquote_core::Symbol]) -> Result<()> {
        self.send_raw(&Encoder::subscribe(symbols)).await?;
        self.state = ConnectionState::Subscribed;
        Ok(())
    }

    /// 接收一个完整 TDX 包，返回 pkt_type 和去掉 checksum 后的 body。
    pub async fn recv_packet(&mut self) -> Result<Option<(u16, Vec<u8>)>> {
        let stream = match &mut self.stream {
            Some(stream) => stream,
            None => return Ok(None),
        };

        let mut header = [0u8; 4];
        if let Err(err) = stream.read_exact(&mut header).await {
            warn!("TDX read header error: {err}");
            self.state = ConnectionState::Disconnected;
            return Ok(None);
        }

        let total_len = u16::from_le_bytes([header[0], header[1]]) as usize;
        let pkt_type = u16::from_le_bytes([header[2], header[3]]);
        if total_len < 4 {
            bail!("TDX packet length too small: {total_len}");
        }

        let remaining_len = total_len - 2;
        if remaining_len > 65_536 {
            bail!("TDX packet too large: {remaining_len}");
        }

        let mut remaining = vec![0u8; remaining_len];
        if remaining_len > 0 {
            stream.read_exact(&mut remaining).await?;
        }
        if remaining.len() < 2 {
            bail!("TDX packet missing checksum");
        }

        let body_len = remaining.len() - 2;
        Ok(Some((pkt_type, remaining[..body_len].to_vec())))
    }

    async fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        let stream = match &mut self.stream {
            Some(stream) => stream,
            None => bail!("not connected"),
        };
        stream.write_all(data).await?;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.state, ConnectionState::Subscribed)
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }
}
