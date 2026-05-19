use anyhow::Result;
use reqwest::Client;

pub struct TencentClient {
    client: Client,
}

impl TencentClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }

    /// 获取实时行情: https://qt.gtimg.cn/q=sh600000,sz000001
    pub async fn get_quote(&self, symbols: &[String]) -> Result<String> {
        let query = symbols.join(",");
        let url = format!("https://qt.gtimg.cn/q={query}");
        Ok(self.client.get(url).send().await?.text().await?)
    }

    /// 获取 K 线: https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=...
    pub async fn get_kline(&self, symbol: &str, period: &str, count: u32) -> Result<String> {
        let url = format!(
            "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={symbol},{period},,,{count},qfq"
        );
        Ok(self.client.get(url).send().await?.text().await?)
    }
}

impl Default for TencentClient {
    fn default() -> Self {
        Self::new()
    }
}
