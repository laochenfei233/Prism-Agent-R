use serde::{Deserialize, Serialize};
use sqlx::Row;
use crate::data::Database;
use crate::utils::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExceptionType {
    BudgetExceeded { level: String },
    GuardrailViolation { check: String },
    ToolError { tool: String, error: String },
    ModelError { error: String },
    Timeout { duration_secs: u64 },
    ContextOverflow { current: usize, limit: usize },
    RateLimitExceeded { retry_after: Option<u64> },
    PermissionDenied { resource: String },
    ValidationError { field: String, message: String },
}

impl ExceptionType {
    pub fn type_name(&self) -> String {
        match self {
            Self::BudgetExceeded { .. } => "budget_exceeded".into(),
            Self::GuardrailViolation { .. } => "guardrail_violation".into(),
            Self::ToolError { .. } => "tool_error".into(),
            Self::ModelError { .. } => "model_error".into(),
            Self::Timeout { .. } => "timeout".into(),
            Self::ContextOverflow { .. } => "context_overflow".into(),
            Self::RateLimitExceeded { .. } => "rate_limit_exceeded".into(),
            Self::PermissionDenied { .. } => "permission_denied".into(),
            Self::ValidationError { .. } => "validation_error".into(),
        }
    }

    pub fn severity(&self) -> String {
        match self {
            Self::BudgetExceeded { .. } => "high".into(),
            Self::GuardrailViolation { .. } => "critical".into(),
            Self::ToolError { .. } => "medium".into(),
            Self::ModelError { .. } => "high".into(),
            Self::Timeout { .. } => "medium".into(),
            Self::ContextOverflow { .. } => "medium".into(),
            Self::RateLimitExceeded { .. } => "low".into(),
            Self::PermissionDenied { .. } => "high".into(),
            Self::ValidationError { .. } => "low".into(),
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::BudgetExceeded { level } => format!("预算超限: {level}"),
            Self::GuardrailViolation { check } => format!("护栏违规: {check}"),
            Self::ToolError { tool, error } => format!("工具错误 [{tool}]: {error}"),
            Self::ModelError { error } => format!("模型错误: {error}"),
            Self::Timeout { duration_secs } => format!("执行超时: {duration_secs}s"),
            Self::ContextOverflow { current, limit } => format!("上下文溢出: {current}/{limit}"),
            Self::RateLimitExceeded { retry_after } => format!("频率限制, retry_after={:?}", retry_after),
            Self::PermissionDenied { resource } => format!("权限拒绝: {resource}"),
            Self::ValidationError { field, message } => format!("校验失败 [{field}]: {message}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentException {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub workflow_id: Option<String>,
    pub run_id: Option<String>,
    pub stage_id: Option<String>,
    pub exception_type: String,
    pub severity: String,
    pub message: String,
    pub context: Option<String>,
    pub tool_name: Option<String>,
    pub model_id: Option<String>,
    pub tokens_used: Option<i64>,
    pub cost_used: Option<f64>,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<String>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceptionQuery {
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub exception_type: Option<String>,
    pub severity: Option<String>,
    pub limit: Option<i64>,
}

pub struct ExceptionRecorder {
    db: Database,
    on_exception: Option<Box<dyn Fn(&AgentException) + Send + Sync>>,
}

impl ExceptionRecorder {
    pub fn new(db: Database) -> Self {
        Self { db, on_exception: None }
    }

    pub fn on_exception<F>(mut self, f: F) -> Self
    where
        F: Fn(&AgentException) + Send + Sync + 'static,
    {
        self.on_exception = Some(Box::new(f));
        self
    }

    pub async fn record(
        &self,
        session_id: &str,
        agent_id: &str,
        exception: ExceptionType,
        context: serde_json::Value,
    ) -> Result<AgentException, AppError> {
        let exc = AgentException {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            agent_id: agent_id.to_string(),
            workflow_id: None,
            run_id: None,
            stage_id: None,
            exception_type: exception.type_name(),
            severity: exception.severity(),
            message: exception.message(),
            context: serde_json::to_string(&context).ok(),
            tool_name: None,
            model_id: None,
            tokens_used: None,
            cost_used: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            resolved_at: None,
            resolved_by: None,
            resolution: None,
        };

        sqlx::query(
            "INSERT INTO agent_exceptions (id, session_id, agent_id, workflow_id, run_id, stage_id, exception_type, severity, message, context, tool_name, model_id, tokens_used, cost_used, created_at, resolved_at, resolved_by, resolution) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"
        )
        .bind(&exc.id)
        .bind(&exc.session_id)
        .bind(&exc.agent_id)
        .bind(&exc.workflow_id)
        .bind(&exc.run_id)
        .bind(&exc.stage_id)
        .bind(&exc.exception_type)
        .bind(&exc.severity)
        .bind(&exc.message)
        .bind(&exc.context)
        .bind(&exc.tool_name)
        .bind(&exc.model_id)
        .bind(exc.tokens_used)
        .bind(exc.cost_used)
        .bind(exc.created_at)
        .bind(exc.resolved_at)
        .bind(&exc.resolved_by)
        .bind(&exc.resolution)
        .execute(&self.db.pool)
        .await?;

        if let Some(f) = &self.on_exception {
            f(&exc);
        }

        Ok(exc)
    }

    pub async fn query(&self, q: &ExceptionQuery) -> Result<Vec<AgentException>, AppError> {
        let mut sql = "SELECT * FROM agent_exceptions WHERE 1=1".to_string();
        let mut bind_vals: Vec<String> = Vec::new();

        if let Some(sid) = &q.session_id {
            sql.push_str(" AND session_id = ?");
            bind_vals.push(sid.clone());
        }
        if let Some(aid) = &q.agent_id {
            sql.push_str(" AND agent_id = ?");
            bind_vals.push(aid.clone());
        }
        if let Some(et) = &q.exception_type {
            sql.push_str(" AND exception_type = ?");
            bind_vals.push(et.clone());
        }
        if let Some(sev) = &q.severity {
            sql.push_str(" AND severity = ?");
            bind_vals.push(sev.clone());
        }

        sql.push_str(" ORDER BY created_at DESC");
        let limit = q.limit.unwrap_or(50);
        sql.push_str(&format!(" LIMIT {limit}"));

        let mut query = sqlx::query(&sql);
        for val in &bind_vals {
            query = query.bind(val);
        }
        let rows = query.fetch_all(&self.db.pool).await?;

        let mut exceptions = Vec::new();
        for row in rows {
            exceptions.push(AgentException {
                id: row.try_get("id")?,
                session_id: row.try_get("session_id")?,
                agent_id: row.try_get("agent_id")?,
                workflow_id: row.try_get("workflow_id")?,
                run_id: row.try_get("run_id")?,
                stage_id: row.try_get("stage_id")?,
                exception_type: row.try_get("exception_type")?,
                severity: row.try_get("severity")?,
                message: row.try_get("message")?,
                context: row.try_get("context")?,
                tool_name: row.try_get("tool_name")?,
                model_id: row.try_get("model_id")?,
                tokens_used: row.try_get("tokens_used")?,
                cost_used: row.try_get("cost_used")?,
                created_at: row.try_get("created_at")?,
                resolved_at: row.try_get("resolved_at")?,
                resolved_by: row.try_get("resolved_by")?,
                resolution: row.try_get("resolution")?,
            });
        }
        Ok(exceptions)
    }

    pub async fn resolve(
        &self,
        exception_id: &str,
        resolved_by: &str,
        resolution: &str,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "UPDATE agent_exceptions SET resolved_at = ?, resolved_by = ?, resolution = ? WHERE id = ?",
        )
        .bind(now)
        .bind(resolved_by)
        .bind(resolution)
        .bind(exception_id)
        .execute(&self.db.pool)
        .await?;
        Ok(())
    }
}
