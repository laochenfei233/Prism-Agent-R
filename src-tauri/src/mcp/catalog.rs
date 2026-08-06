use std::collections::HashMap;
use tokio::sync::RwLock;

use super::transport::McpTool;

// ── 工具缓存 ──────────────────────────────────────────────

pub struct McpCatalog {
    /// server_id -> (tools, timestamp)
    cache: RwLock<HashMap<String, (Vec<McpTool>, std::time::Instant)>>,
    ttl: std::time::Duration,
}

impl McpCatalog {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl: std::time::Duration::from_secs(3600), // 1 小时 TTL
        }
    }

    /// 更新工具缓存
    pub async fn update(&self, server_id: &str, tools: Vec<McpTool>) {
        let mut cache = self.cache.write().await;
        cache.insert(server_id.to_string(), (tools, std::time::Instant::now()));
    }

    /// 获取缓存的工具（检查 TTL）
    pub async fn get(&self, server_id: &str) -> Option<Vec<McpTool>> {
        let cache = self.cache.read().await;
        cache.get(server_id).and_then(|(tools, ts)| {
            if ts.elapsed() < self.ttl {
                Some(tools.clone())
            } else {
                None
            }
        })
    }

    /// 清除缓存
    pub async fn invalidate(&self, server_id: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(server_id);
    }

    /// 清除所有缓存
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

impl Default for McpCatalog {
    fn default() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl: std::time::Duration::from_secs(3600),
        }
    }
}
