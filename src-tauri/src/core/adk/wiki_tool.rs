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
