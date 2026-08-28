use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::json;

use crate::data::db::Database;
use crate::data::services::memory_service::MemoryService;

use super::error::AgentError;
use super::model::ToolOutput;
use super::tool::ToolExecutor;

// ── Memory Search Tool ───────────────────────────────────

pub struct MemorySearchTool {
    db: Database,
    base_dir: PathBuf,
}

impl MemorySearchTool {
    pub fn new(db: Database, base_dir: PathBuf) -> Self {
        Self { db, base_dir }
    }
}

#[async_trait]
impl ToolExecutor for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn description(&self) -> &str {
        "搜索历史记忆（全局/项目/会话笔记），返回匹配的路径、范围与摘要。参数：query（搜索关键词）、limit（可选，默认 10，最大 50）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "搜索关键词" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10, "description": "最大结果数" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("memory_search: 缺少 query".into()))?;
        let limit = args["limit"].as_u64().unwrap_or(10).min(50) as usize;

        let svc = MemoryService::new(self.db.clone(), self.base_dir.clone());
        match svc.search(query).await {
            Ok(hits) => {
                if hits.is_empty() {
                    return Ok(ToolOutput::text(format!(
                        "记忆中未找到与「{query}」相关的内容"
                    )));
                }
                let lines: Vec<String> = hits
                    .into_iter()
                    .take(limit)
                    .map(|h| format!("- [{}] {}\n  {}", h.scope, h.path, h.snippet))
                    .collect();
                Ok(ToolOutput::text(format!(
                    "找到 {} 条记忆:\n{}",
                    lines.len(),
                    lines.join("\n")
                )))
            }
            Err(e) => Ok(ToolOutput::text(format!("记忆搜索失败：{e}"))),
        }
    }
}

// ── Memory Save Tool ─────────────────────────────────────

pub struct MemorySaveTool {
    db: Database,
    base_dir: PathBuf,
}

impl MemorySaveTool {
    pub fn new(db: Database, base_dir: PathBuf) -> Self {
        Self { db, base_dir }
    }
}

#[async_trait]
impl ToolExecutor for MemorySaveTool {
    fn name(&self) -> &str {
        "memory_save"
    }

    fn description(&self) -> &str {
        "将一条值得长期保留的信息保存到记忆库（默认全局记忆）。参数：content（记忆内容，必填）、scope（可选：global 全局 / project 项目，默认 global）、project（可选，scope=project 时的项目名）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "要保存的记忆内容" },
                "scope": { "type": "string", "enum": ["global", "project"], "default": "global", "description": "记忆范围" },
                "project": { "type": "string", "description": "项目名（scope=project 时使用）" }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let content = args["content"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("memory_save: 缺少 content".into()))?;
        let scope = args["scope"].as_str().unwrap_or("global");
        let project = args["project"].as_str();

        if content.trim().is_empty() {
            return Ok(ToolOutput::text("content 为空，未保存记忆。".into()));
        }
        if content.chars().count() > 20_000 {
            return Ok(ToolOutput::error(
                "记忆内容过长（超过 20000 字符），请拆分后保存。".into(),
            ));
        }

        let svc = MemoryService::new(self.db.clone(), self.base_dir.clone());
        match svc.append_entry(scope, project, content).await {
            Ok(path) => Ok(ToolOutput::text(format!(
                "已保存记忆到 {path}（scope={scope}）"
            ))),
            Err(e) => Ok(ToolOutput::text(format!("记忆保存失败：{e}"))),
        }
    }
}
