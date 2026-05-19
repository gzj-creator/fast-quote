use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub tdx: TdxSection,
    pub tencent: TencentSection,
    pub store: StoreSection,
    pub tcp: TcpSection,
    pub ws: WsSection,
}

#[derive(Debug, Deserialize)]
pub struct TdxSection {
    pub host: String,
    pub port: u16,
    pub reconnect_ms: u64,
    pub heartbeat_s: u64,
}

#[derive(Debug, Deserialize)]
pub struct TencentSection {
    pub poll_interval_ms: u64,
    pub kline_sync_interval_s: u64,
    pub financial_sync_interval_s: u64,
}

#[derive(Debug, Deserialize)]
pub struct StoreSection {
    pub redis_url: String,
    pub mysql_url: String,
}

#[derive(Debug, Deserialize)]
pub struct TcpSection {
    pub addr: String,
}

#[derive(Debug, Deserialize)]
pub struct WsSection {
    pub addr: String,
}

pub fn load_config(path: &str) -> anyhow::Result<AppConfig> {
    let content = std::fs::read_to_string(path)?;
    parse_config_str(&content)
}

pub fn parse_config_str(content: &str) -> anyhow::Result<AppConfig> {
    Ok(toml::from_str(content)?)
}

#[cfg(test)]
mod tests {
    use super::parse_config_str;

    #[test]
    fn parses_all_config_sections() {
        let config = parse_config_str(
            r#"
            [tdx]
            host = "127.0.0.1"
            port = 7709
            reconnect_ms = 100
            heartbeat_s = 30

            [tencent]
            poll_interval_ms = 1000
            kline_sync_interval_s = 60
            financial_sync_interval_s = 3600

            [store]
            redis_url = "redis://localhost"
            mysql_url = "mysql://localhost/fastquote"

            [tcp]
            addr = "127.0.0.1:9000"

            [ws]
            addr = "127.0.0.1:9001"
            "#,
        )
        .unwrap();

        assert_eq!(config.tdx.host, "127.0.0.1");
        assert_eq!(config.tdx.port, 7709);
        assert_eq!(config.tencent.poll_interval_ms, 1000);
        assert_eq!(config.store.redis_url, "redis://localhost");
        assert_eq!(config.tcp.addr, "127.0.0.1:9000");
        assert_eq!(config.ws.addr, "127.0.0.1:9001");
    }
}
