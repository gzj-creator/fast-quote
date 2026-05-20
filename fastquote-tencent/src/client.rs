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
        let url = kline_url(symbol, period, count);
        Ok(self.client.get(url).send().await?.text().await?)
    }
}

pub fn kline_url(symbol: &str, period: &str, count: u32) -> String {
    if period.starts_with('m') {
        format!("https://ifzq.gtimg.cn/appstock/app/kline/mkline?param={symbol},{period},,{count}")
    } else {
        format!("https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={symbol},{period},,,{count},qfq")
    }
}

impl Default for TencentClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::kline_url;

    #[test]
    fn minute_kline_uses_mkline_endpoint() {
        let url = kline_url("sh600519", "m1", 120);

        assert_eq!(
            url,
            "https://ifzq.gtimg.cn/appstock/app/kline/mkline?param=sh600519,m1,,120"
        );
    }

    #[test]
    fn day_kline_uses_forward_adjusted_endpoint() {
        let url = kline_url("sh600519", "day", 120);

        assert_eq!(
            url,
            "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=sh600519,day,,,120,qfq"
        );
    }
}
