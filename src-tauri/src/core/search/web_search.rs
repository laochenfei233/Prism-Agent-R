use std::sync::Arc;
use async_trait::async_trait;
use serde_json::json;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::ToolOutput;
use crate::core::adk::tool::ToolExecutor;
use super::service::SearchService;

pub struct WebSearchTool {
    service: Arc<SearchService>,
}

impl WebSearchTool {
    pub fn new(service: Arc<SearchService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ToolExecutor for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "搜索互联网获取最新信息。参数：query（搜索词），limit（结果数，默认 5，最大 10）。返回带来源链接的结果摘要，适合时效性信息、事实核查、资料搜集。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "搜索词" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 10, "default": 5, "description": "返回结果数量" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| AgentError::Internal("web_search: 缺少 query 参数".to_string()))?;

        if query.trim().is_empty() {
            return Ok(ToolOutput::text("搜索词不能为空，请提供具体的搜索查询。".to_string()));
        }

        let limit = args["limit"].as_u64().unwrap_or(5).min(10) as usize;

        match self.service.search(query, limit).await {
            Ok(hits) if !hits.is_empty() => {
                let mut output = format!("搜索「{query}」找到 {} 条结果：\n\n", hits.len());
                for (i, hit) in hits.iter().enumerate() {
                    output.push_str(&format!(
                        "{}. [{}]({})\n   {}\n\n",
                        i + 1,
                        hit.title,
                        hit.url,
                        truncate(&hit.snippet, 200),
                    ));
                }
                Ok(ToolOutput::text(output))
            }
            Ok(_) => {
                Ok(ToolOutput::text(format!(
                    "未找到关于「{query}」的结果。搜索服务可能未配置或查询词过于具体。建议：\n1. 尝试更通用的搜索词\n2. 检查设置中的网络搜索配置"
                )))
            }
            Err(e) => {
                Ok(ToolOutput::text(format!(
                    "搜索「{query}」时出错：{e}。请稍后重试或检查网络搜索配置。"
                )))
            }
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", &s[..max_chars])
    }
}
