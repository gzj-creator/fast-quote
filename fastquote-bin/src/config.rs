use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub tdx: TdxSection,
    pub tencent: TencentSection,
    pub store: StoreSection,
    pub tcp: TcpSection,
    pub ws: WsSection,
    #[serde(default)]
    pub symbols: Vec<String>,
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
    #[serde(default)]
    pub database_url: Option<String>,
    #[serde(default)]
    pub sqlite_url: Option<String>,
    #[serde(default)]
    pub mysql_url: Option<String>,
}

impl StoreSection {
    pub fn database_url(&self) -> &str {
        self.database_url
            .as_deref()
            .or(self.sqlite_url.as_deref())
            .or(self.mysql_url.as_deref())
            .unwrap_or("")
    }
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
            database_url = "sqlite://data/fastquote.db"

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
        assert_eq!(config.store.database_url(), "sqlite://data/fastquote.db");
        assert_eq!(config.tcp.addr, "127.0.0.1:9000");
        assert_eq!(config.ws.addr, "127.0.0.1:9001");
    }

    #[test]
    fn store_prefers_database_url_for_relational_persistence() {
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
            database_url = "mysql://localhost/fastquote"

            [tcp]
            addr = "127.0.0.1:9000"

            [ws]
            addr = "127.0.0.1:9001"
            "#,
        )
        .unwrap();

        assert_eq!(config.store.database_url(), "mysql://localhost/fastquote");
    }

    #[test]
    fn store_falls_back_to_sqlite_url_when_database_url_is_absent() {
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
            sqlite_url = "sqlite://data/fastquote.db"

            [tcp]
            addr = "127.0.0.1:9000"

            [ws]
            addr = "127.0.0.1:9001"
            "#,
        )
        .unwrap();

        assert_eq!(config.store.database_url(), "sqlite://data/fastquote.db");
    }

    #[test]
    fn store_falls_back_to_mysql_url_for_legacy_config() {
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

        assert_eq!(config.store.database_url(), "mysql://localhost/fastquote");
    }
}
