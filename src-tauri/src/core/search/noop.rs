use async_trait::async_trait;
use super::{SearchHit, SearchProvider};

/// 无配置时的默认 Provider，返回空结果 + 提示文案
pub struct NoopSearchProvider;

#[async_trait]
impl SearchProvider for NoopSearchProvider {
    fn name(&self) -> &'static str {
        "noop"
    }

    async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<SearchHit>, String> {
        Ok(vec![])
    }
}

impl NoopSearchProvider {
    /// 返回用户可见的提示文案
    pub fn hint() -> &'static str {
        "搜索服务未配置。请在设置中配置网络搜索 Provider（Tavily / Serper / Searxng）。"
    }
}
