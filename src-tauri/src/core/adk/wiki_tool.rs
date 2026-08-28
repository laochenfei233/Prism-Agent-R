use async_trait::async_trait;
use serde_json::json;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::ToolOutput;
use crate::core::adk::tool::ToolExecutor;
use crate::data::db::Database;

/// 对话内工具：wiki_write（§10.1.1 三入口之一）
/// Agent 在对话中调用 → WikiService::write_ai 将新知识入库（自动分类到 entities/concepts 等页面）
pub struct WikiWriteTool {
    db: Database,
}

impl WikiWriteTool {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ToolExecutor for WikiWriteTool {
    fn name(&self) -> &str {
        "wiki_write"
    }

    fn description(&self) -> &str {
        "将新知识写入指定知识库（自动分类到 entities/concepts 等页面），返回变更摘要。参数：wiki_id（知识库 ID）、info（要写入的知识内容）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "wiki_id": { "type": "string", "description": "知识库 ID" },
                "info": { "type": "string", "description": "要写入的知识内容" }
            },
            "required": ["wiki_id", "info"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let wiki_id = args["wiki_id"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("wiki_write: 缺少 wiki_id".into()))?;
        let info = args["info"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("wiki_write: 缺少 info".into()))?;
        if info.trim().is_empty() {
            return Ok(ToolOutput::text("info 内容为空，未执行写入。".into()));
        }

        let svc = crate::data::services::wiki_service::WikiService::new(self.db.clone());
        match svc.write_ai(wiki_id, info, false).await {
            Ok(result) => {
                let summary = result
                    .get("result")
                    .and_then(|s| s.as_str())
                    .unwrap_or("applied");
                Ok(ToolOutput::text(format!("Wiki 更新完成：{summary}")))
            }
            Err(e) => Ok(ToolOutput::text(format!("Wiki 写入失败：{e}"))),
        }
    }
}

// ── 对话内工具：wiki_search ──────────────────────────────
/// Agent 在对话中调用 → WikiService::search_pages 查询已有知识
pub struct WikiSearchTool {
    db: Database,
}

impl WikiSearchTool {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ToolExecutor for WikiSearchTool {
    fn name(&self) -> &str {
        "wiki_search"
    }

    fn description(&self) -> &str {
        "搜索知识库页面内容，返回匹配页面的路径、标题与内容摘要。参数：wiki_id（知识库 ID）、query（搜索关键词）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "wiki_id": { "type": "string", "description": "知识库 ID" },
                "query": { "type": "string", "description": "搜索关键词" }
            },
            "required": ["wiki_id", "query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let wiki_id = args["wiki_id"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("wiki_search: 缺少 wiki_id".into()))?;
        let query = args["query"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("wiki_search: 缺少 query".into()))?;
        if query.trim().is_empty() {
            return Ok(ToolOutput::text("query 为空，未执行搜索。".into()));
        }

        let svc = crate::data::services::wiki_service::WikiService::new(self.db.clone());
        match svc.search_pages(wiki_id, query).await {
            Ok(hits) => {
                if hits.is_empty() {
                    return Ok(ToolOutput::text(format!(
                        "知识库 {wiki_id} 中未找到与「{query}」相关的页面"
                    )));
                }
                let lines: Vec<String> = hits
                    .into_iter()
                    .map(|h| format!("- {}（标题: {}）\n  {}", h.path, h.title, h.snippet))
                    .collect();
                Ok(ToolOutput::text(format!(
                    "找到 {} 个相关页面:\n{}",
                    lines.len(),
                    lines.join("\n")
                )))
            }
            Err(e) => Ok(ToolOutput::text(format!("Wiki 搜索失败：{e}"))),
        }
    }
}
