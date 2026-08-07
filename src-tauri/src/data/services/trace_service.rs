use serde::{Deserialize, Serialize};
use sqlx::Row;
use crate::data::db::Database;
use crate::utils::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTrace {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub trace_id: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub steps: Vec<TraceStep>,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_cost: f64,
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub step_index: u32,
    pub kind: String,
    pub input_summary: String,
    pub output_summary: String,
    pub latency_ms: u64,
    pub tool_name: Option<String>,
    pub error: Option<String>,
}

pub struct TraceService {
    db: Database,
}

impl TraceService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn record_trace(&self, trace: &AgentTrace) -> Result<(), AppError> {
        let steps_json = serde_json::to_string(&trace.steps).unwrap_or_default();
        let tokens_json = serde_json::json!({
            "prompt_tokens": trace.total_prompt_tokens,
            "completion_tokens": trace.total_completion_tokens,
        });
        sqlx::query(
            "INSERT INTO agent_traces (id, session_id, agent_id, trace_id, started_at, finished_at, steps, total_tokens, total_cost, outcome, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
        )
        .bind(&trace.id)
        .bind(&trace.session_id)
        .bind(&trace.agent_id)
        .bind(&trace.trace_id)
        .bind(trace.started_at)
        .bind(trace.finished_at)
        .bind(&steps_json)
        .bind(tokens_json.to_string())
        .bind(trace.total_cost)
        .bind(&trace.outcome)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.db.pool).await?;

        // 清理旧记录（保留条数可配置，默认 1000）
        let retain = crate::data::settings::prefs::get_i64(&self.db.pool, "trace.retain", 1000)
            .await
            .clamp(100, 10_000);
        sqlx::query(
            "DELETE FROM agent_traces WHERE id NOT IN (SELECT id FROM agent_traces ORDER BY started_at DESC LIMIT ?)"
        )
            .bind(retain)
            .execute(&self.db.pool).await?;
        Ok(())
    }

    pub async fn list_traces(&self, session_id: &str, limit: Option<i64>) -> Result<Vec<AgentTrace>, AppError> {
        let limit = limit.unwrap_or(50);
        let rows = sqlx::query("SELECT * FROM agent_traces WHERE session_id = ?1 ORDER BY started_at DESC LIMIT ?2")
            .bind(session_id).bind(limit).fetch_all(&self.db.pool).await?;

        let mut traces = Vec::new();
        for row in rows {
            let id: String = row.try_get("id")?;
            let session_id: String = row.try_get("session_id")?;
            let agent_id: String = row.try_get("agent_id")?;
            let trace_id: String = row.try_get("trace_id")?;
            let started_at: i64 = row.try_get("started_at")?;
            let finished_at: Option<i64> = row.try_get("finished_at")?;
            let steps_json: String = row.try_get("steps")?;
            let total_cost: f64 = row.try_get("total_cost")?;
            let outcome: String = row.try_get("outcome")?;
            let steps: Vec<TraceStep> = serde_json::from_str(&steps_json).unwrap_or_default();
            traces.push(AgentTrace { id, session_id, agent_id, trace_id, started_at, finished_at, steps, total_prompt_tokens: 0, total_completion_tokens: 0, total_cost, outcome });
        }
        Ok(traces)
    }
}
