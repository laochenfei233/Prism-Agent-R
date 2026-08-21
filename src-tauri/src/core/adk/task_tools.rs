use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::error::AgentError;
use super::model::ToolOutput;
use super::tool::ToolExecutor;

// ── Task Model ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub subject: String,
    pub status: String,
    pub owner: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ── Global Task Store ─────────────────────────────────────

use std::sync::OnceLock;

static TASK_STORE_RAW: OnceLock<Arc<RwLock<Vec<Task>>>> = OnceLock::new();

pub fn task_store() -> &'static Arc<RwLock<Vec<Task>>> {
    TASK_STORE_RAW.get_or_init(|| Arc::new(RwLock::new(Vec::new())))
}

// ── Task Create Tool ──────────────────────────────────────

pub struct TaskCreateTool;

#[async_trait]
impl ToolExecutor for TaskCreateTool {
    fn name(&self) -> &str {
        "task_create"
    }

    fn description(&self) -> &str {
        "创建任务到共享看板。参数：subject（任务描述，必填）、owner（负责人，可选）。返回创建的任务。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string", "description": "任务描述" },
                "owner": { "type": "string", "description": "负责人（Agent ID）" }
            },
            "required": ["subject"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let subject = args["subject"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("task_create: 缺少 subject".into()))?;
        let owner = args["owner"].as_str().map(String::from);

        let now = chrono::Utc::now().timestamp_millis();
        let id = format!("task-{}-{}", now, rand::random::<u32>());
        let task = Task {
            id,
            subject: subject.to_string(),
            status: "todo".to_string(),
            owner,
            created_at: now,
            updated_at: now,
        };

        let mut store = task_store().write().await;
        store.push(task.clone());

        Ok(ToolOutput::text(format!(
            "已创建任务: {} [{}] {}",
            task.id, task.status, task.subject
        )))
    }
}

// ── Task Update Tool ──────────────────────────────────────

pub struct TaskUpdateTool;

#[async_trait]
impl ToolExecutor for TaskUpdateTool {
    fn name(&self) -> &str {
        "task_update"
    }

    fn description(&self) -> &str {
        "更新任务状态。参数：id（任务 ID，必填）、status（新状态：todo/doing/done，可选）、subject（新描述，可选）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "任务 ID" },
                "status": { "type": "string", "enum": ["todo", "doing", "done"], "description": "新状态" },
                "subject": { "type": "string", "description": "新描述" }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let id = args["id"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("task_update: 缺少 id".into()))?;
        let new_status = args["status"].as_str().map(String::from);
        let new_subject = args["subject"].as_str().map(String::from);

        let mut store = task_store().write().await;
        let task = store
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| AgentError::InvalidArgs(format!("任务不存在: {id}")))?;

        if let Some(s) = new_status {
            task.status = s;
        }
        if let Some(subj) = new_subject {
            task.subject = subj;
        }
        task.updated_at = chrono::Utc::now().timestamp_millis();

        let result = task.clone();
        Ok(ToolOutput::text(format!(
            "已更新任务: {} [{}] {}",
            result.id, result.status, result.subject
        )))
    }
}

// ── Task List Tool ────────────────────────────────────────

pub struct TaskListTool;

#[async_trait]
impl ToolExecutor for TaskListTool {
    fn name(&self) -> &str {
        "task_list"
    }

    fn description(&self) -> &str {
        "查询看板任务。参数：status（筛选状态：todo/doing/done，可选）、owner（筛选负责人，可选）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "status": { "type": "string", "enum": ["todo", "doing", "done"], "description": "筛选状态" },
                "owner": { "type": "string", "description": "筛选负责人" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let filter_status = args["status"].as_str();
        let filter_owner = args["owner"].as_str();

        let store = task_store().read().await;
        let mut tasks: Vec<&Task> = store.iter().collect();

        if let Some(s) = filter_status {
            tasks.retain(|t| t.status == s);
        }
        if let Some(o) = filter_owner {
            tasks.retain(|t| t.owner.as_deref() == Some(o));
        }

        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        if tasks.is_empty() {
            return Ok(ToolOutput::text("看板为空".to_string()));
        }

        let lines: Vec<String> = tasks
            .iter()
            .map(|t| {
                let owner_str = t
                    .owner
                    .as_deref()
                    .map(|o| format!(" @{o}"))
                    .unwrap_or_default();
                format!("[{}] {}{} — {}", t.status, t.id, owner_str, t.subject)
            })
            .collect();

        Ok(ToolOutput::text(lines.join("\n")))
    }
}

// ── Task Delete Tool ──────────────────────────────────────

pub struct TaskDeleteTool;

#[async_trait]
impl ToolExecutor for TaskDeleteTool {
    fn name(&self) -> &str {
        "task_delete"
    }

    fn description(&self) -> &str {
        "删除看板任务。参数：id（任务 ID，必填）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "任务 ID" }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let id = args["id"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("task_delete: 缺少 id".into()))?;

        let mut store = task_store().write().await;
        let before = store.len();
        store.retain(|t| t.id != id);
        let removed = store.len() < before;

        Ok(ToolOutput::text(if removed {
            format!("已删除任务: {id}")
        } else {
            format!("任务不存在: {id}")
        }))
    }
}
