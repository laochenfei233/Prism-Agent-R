pub mod noop;
pub mod searxng;
pub mod serper;
pub mod service;
pub mod tavily;
pub mod web_search;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 网络搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
    pub published_at: Option<i64>,
}

/// 网络搜索 Provider trait
#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Provider 名称（"tavily" / "serper" / "searxng"）
    fn name(&self) -> &'static str;

    /// 执行搜索
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, String>;
}
