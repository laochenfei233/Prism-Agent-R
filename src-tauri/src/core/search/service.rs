use std::sync::Arc;
use super::{noop::NoopSearchProvider, SearchHit, SearchProvider};

/// 搜索服务：管理多个 provider，支持选择/切换/降级
pub struct SearchService {
    providers: Vec<Arc<dyn SearchProvider>>,
    primary: String,
    fallback: Option<String>,
}

impl SearchService {
    /// 从配置构建 SearchService
    ///
    /// config 结构：
    /// - search.provider: "tavily" | "serper" | "searxng"
    /// - search.api_key: 加密后的 API Key
    /// - search.searxng_url: Searxng 实例地址
    /// - search.fallback_provider: 可选的备用 provider
    pub fn from_config(config: &std::collections::HashMap<String, String>) -> Self {
        let mut providers: Vec<Arc<dyn SearchProvider>> = Vec::new();

        // 读取配置
        let provider_name = config.get("search.provider").map(|s| s.as_str()).unwrap_or("noop");
        let api_key = config.get("search.api_key").cloned().unwrap_or_default();
        let searxng_url = config.get("search.searxng_url").cloned().unwrap_or_default();
        let fallback_name = config.get("search.fallback_provider").cloned();

        // 按配置实例化 provider
        match provider_name {
            "tavily" if !api_key.is_empty() => {
                providers.push(Arc::new(super::tavily::TavilyProvider::new(api_key.clone())));
            }
            "serper" if !api_key.is_empty() => {
                providers.push(Arc::new(super::serper::SerperProvider::new(api_key.clone())));
            }
            "searxng" if !searxng_url.is_empty() => {
                providers.push(Arc::new(super::searxng::SearxngProvider::new(searxng_url.clone())));
            }
            _ => {}
        }

        // 备用 provider
        if let Some(ref fb) = fallback_name {
            match fb.as_str() {
                "tavily" if !api_key.is_empty() && provider_name != "tavily" => {
                    providers.push(Arc::new(super::tavily::TavilyProvider::new(api_key)));
                }
                "serper" if !api_key.is_empty() && provider_name != "serper" => {
                    providers.push(Arc::new(super::serper::SerperProvider::new(api_key)));
                }
                "searxng" if !searxng_url.is_empty() && provider_name != "searxng" => {
                    providers.push(Arc::new(super::searxng::SearxngProvider::new(searxng_url)));
                }
                _ => {}
            }
        }

        let has_real = !providers.is_empty();
        let primary = if has_real {
            providers[0].name().to_string()
        } else {
            "noop".to_string()
        };

        // 无配置时注册 NoopProvider
        if !has_real {
            providers.push(Arc::new(NoopSearchProvider));
        }

        Self {
            providers,
            primary,
            fallback: fallback_name,
        }
    }

    /// 执行搜索：按优先级尝试 provider，失败自动切换
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
        let mut last_error = String::new();

        for provider in &self.providers {
            match provider.search(query, limit).await {
                Ok(hits) if !hits.is_empty() => return Ok(hits),
                Ok(_) => {
                    // 空结果，尝试下一个
                    last_error = format!("{} 返回空结果", provider.name());
                }
                Err(e) => {
                    last_error = e;
                    tracing::warn!("Search provider '{}' failed: {}", provider.name(), last_error);
                }
            }
        }

        // 所有 provider 都失败或返回空
        if self.providers.iter().any(|p| p.name() == "noop") {
            Ok(vec![])
        } else {
            Err(last_error)
        }
    }

    /// 返回当前可用的 provider 列表
    pub fn available_providers(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    /// 返回当前主 provider 名称
    pub fn primary_provider(&self) -> &str {
        &self.primary
    }

    /// 是否有真实搜索能力（非 Noop）
    pub fn has_real_provider(&self) -> bool {
        self.providers.iter().any(|p| p.name() != "noop")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_provider() {
        let svc = SearchService::from_config(&std::collections::HashMap::new());
        assert!(!svc.has_real_provider());
        assert_eq!(svc.primary_provider(), "noop");
    }

    #[tokio::test]
    async fn test_noop_search_returns_empty() {
        let svc = SearchService::from_config(&std::collections::HashMap::new());
        let hits = svc.search("test", 5).await.unwrap();
        assert!(hits.is_empty());
    }
}
