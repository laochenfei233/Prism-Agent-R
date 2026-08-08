use serde::{Deserialize, Serialize};
use sqlx::Row;

use super::spec::SpecDocument;
use super::plan::{ExecutionPlan, TaskResult};

/// 自主编排会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorSession {
    pub id: String,
    pub user_request: String,
    pub spec: Option<SpecDocument>,
    pub plan: Option<ExecutionPlan>,
    /// §27.3 最近一轮的任务执行结果（供审查）
    pub task_results: Vec<TaskResult>,
    pub status: OrchestratorStatus,
    pub cycle_count: u32,
    pub max_cycles: u32,
    pub history: Vec<OrchestratorEvent>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestratorStatus {
    SpecGenerating,
    SpecReviewing,
    PlanGenerating,
    Executing,
    Reviewing,
    Repairing,
    Completed,
    Paused,
    BudgetExhausted,
    Failed(String),
}

impl OrchestratorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SpecGenerating => "spec_generating",
            Self::SpecReviewing => "spec_reviewing",
            Self::PlanGenerating => "plan_generating",
            Self::Executing => "executing",
            Self::Reviewing => "reviewing",
            Self::Repairing => "repairing",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::BudgetExhausted => "budget_exhausted",
            Self::Failed(_) => "failed",
        }
    }
}#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorEvent {
    pub event_type: String,
    pub message: String,
    pub timestamp: i64,
    pub data: Option<serde_json::Value>,
}

impl OrchestratorSession {
    pub fn new(user_request: String, max_cycles: u32) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_request,
            spec: None,
            plan: None,
            task_results: Vec::new(),
            status: OrchestratorStatus::SpecGenerating,
            cycle_count: 0,
            max_cycles,
            history: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn push_event(&mut self, event: OrchestratorEvent) {
        self.history.push(event);
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    pub fn all_tasks_passed(&self) -> bool {
        // Check if all spec tasks have been reviewed and passed
        // This is a simplified check - the actual implementation tracks task results
        false
    }

    // ── §27.2 SQLite 持久化（崩溃可恢复） ──────────────

    /// 持久化会话到 orchestrator_sessions 表
    pub async fn save(&self, pool: &sqlx::SqlitePool) -> Result<(), String> {
        let spec_json = self.spec.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default());
        let plan_json = self.plan.as_ref().map(|p| serde_json::to_string(p).unwrap_or_default());
        let history_json = serde_json::to_string(&self.history).unwrap_or_else(|_| "[]".into());

        sqlx::query(
            "INSERT INTO orchestrator_sessions (id, user_request, spec, plan, status, cycle_count, max_cycles, history, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) ON CONFLICT(id) DO UPDATE SET user_request=excluded.user_request, spec=excluded.spec, plan=excluded.plan, status=excluded.status, cycle_count=excluded.cycle_count, max_cycles=excluded.max_cycles, history=excluded.history, updated_at=excluded.updated_at",
        )
        .bind(&self.id)
        .bind(&self.user_request)
        .bind(&spec_json)
        .bind(&plan_json)
        .bind(self.status.as_str())
        .bind(self.cycle_count as i64)
        .bind(self.max_cycles as i64)
        .bind(&history_json)
        .bind(self.created_at)
        .bind(self.updated_at)
        .execute(pool)
        .await
        .map_err(|e| format!("保存编排会话失败: {e}"))?;

        Ok(())
    }

    /// 从数据库加载会话（崩溃后恢复）
    pub async fn load(pool: &sqlx::SqlitePool, id: &str) -> Result<Option<Self>, String> {
        let row = sqlx::query(
            "SELECT id, user_request, spec, plan, status, cycle_count, max_cycles, history, created_at, updated_at FROM orchestrator_sessions WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("加载编排会话失败: {e}"))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let session_id: String = row.try_get("id").map_err(|e| e.to_string())?;
        let user_request: String = row.try_get("user_request").map_err(|e| e.to_string())?;
        let spec_json: Option<String> = row.try_get("spec").map_err(|e| e.to_string())?;
        let plan_json: Option<String> = row.try_get("plan").map_err(|e| e.to_string())?;
        let status_str: String = row.try_get("status").map_err(|e| e.to_string())?;
        let cycle_count: i64 = row.try_get("cycle_count").map_err(|e| e.to_string())?;
        let max_cycles: i64 = row.try_get("max_cycles").map_err(|e| e.to_string())?;
        let history_json: String = row.try_get("history").map_err(|e| e.to_string())?;
        let created_at: i64 = row.try_get("created_at").map_err(|e| e.to_string())?;
        let updated_at: i64 = row.try_get("updated_at").map_err(|e| e.to_string())?;

        let status = match status_str.as_str() {
            "spec_generating" => OrchestratorStatus::SpecGenerating,
            "spec_reviewing" => OrchestratorStatus::SpecReviewing,
            "plan_generating" => OrchestratorStatus::PlanGenerating,
            "executing" => OrchestratorStatus::Executing,
            "reviewing" => OrchestratorStatus::Reviewing,
            "repairing" => OrchestratorStatus::Repairing,
            "completed" => OrchestratorStatus::Completed,
            "paused" => OrchestratorStatus::Paused,
            "budget_exhausted" => OrchestratorStatus::BudgetExhausted,
            _ => OrchestratorStatus::Failed(status_str),
        };

        Ok(Some(Self {
            id: session_id,
            user_request,
            spec: spec_json.and_then(|s| serde_json::from_str(&s).ok()),
            plan: plan_json.and_then(|p| serde_json::from_str(&p).ok()),
            task_results: Vec::new(),
            status,
            cycle_count: cycle_count as u32,
            max_cycles: max_cycles as u32,
            history: serde_json::from_str(&history_json).unwrap_or_default(),
            created_at,
            updated_at,
        }))
    }
}
