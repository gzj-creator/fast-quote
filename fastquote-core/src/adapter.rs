use crate::types::Symbol;
use anyhow::Result;

/// 数据源适配器 trait
pub trait Adapter: Send + Sync + 'static {
    fn start(&self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn stop(&self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn subscribe(&self, symbols: &[Symbol]) -> impl std::future::Future<Output = Result<()>> + Send;
}
