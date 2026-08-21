use sqlx::Row;
use tauri::State;

use crate::core::budget::config::BudgetConfig;
use crate::core::budget::policy::BudgetPolicy;
use crate::core::budget::tracker::BudgetTracker;
use crate::core::guardrails::tool_guard::{ToolGuardrail, ToolPolicy};
use crate::core::observability::exception::{ExceptionQuery, ExceptionRecorder};
use crate::utils::error::AppError;

// ── 预算命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn budget_get_config(
    _state: State<'_, crate::AppState>,
) -> Result<BudgetConfig, AppError> {
    // Return default config for now
    Ok(BudgetConfig::default())
}

#[tauri::command]
pub async fn budget_get_status(
    _state: State<'_, crate::AppState>,
) -> Result<BudgetStatusDto, AppError> {
    let tracker = BudgetTracker::new(BudgetConfig::default(), BudgetPolicy::default());
    let snapshot = tracker.snapshot().await;
    Ok(BudgetStatusDto {
        daily_tokens_used: snapshot.daily_tokens_used,
        daily_tokens_limit: 1_000_000,
        daily_cost_used: snapshot.daily_cost_used,
        daily_cost_limit: 10.0,
        monthly_cost_used: snapshot.monthly_cost_used,
        monthly_cost_limit: 200.0,
        active_workflows: snapshot.active_workflows,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetStatusDto {
    pub daily_tokens_used: u64,
    pub daily_tokens_limit: u64,
    pub daily_cost_used: f64,
    pub daily_cost_limit: f64,
    pub monthly_cost_used: f64,
    pub monthly_cost_limit: f64,
    pub active_workflows: u32,
}

// ── 异常命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn exception_list(
    state: State<'_, crate::AppState>,
    session_id: Option<String>,
    agent_id: Option<String>,
    exception_type: Option<String>,
    severity: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<crate::core::observability::exception::AgentException>, AppError> {
    let recorder = ExceptionRecorder::new(state.db.clone());
    let query = ExceptionQuery {
        session_id,
        agent_id,
        exception_type,
        severity,
        limit,
    };
    recorder.query(&query).await
}

#[tauri::command]
pub async fn exception_resolve(
    state: State<'_, crate::AppState>,
    exception_id: String,
    resolution: String,
) -> Result<(), AppError> {
    let recorder = ExceptionRecorder::new(state.db.clone());
    recorder.resolve(&exception_id, "user", &resolution).await
}

/// §26.4 清除已处理的异常（可保留未处理的）
#[tauri::command]
pub async fn exception_clear(state: State<'_, crate::AppState>) -> Result<(), AppError> {
    sqlx::query("DELETE FROM agent_exceptions WHERE resolved_at IS NOT NULL")
        .execute(&state.db.pool)
        .await?;
    Ok(())
}

/// §26.4 导出结构化日志（JSON Lines）
#[tauri::command]
pub async fn log_export(state: State<'_, crate::AppState>) -> Result<String, AppError> {
    let rows = sqlx::query(
        "SELECT id, session_id, agent_id, exception_type, severity, message, created_at FROM agent_exceptions ORDER BY created_at DESC LIMIT 500",
    )
    .fetch_all(&state.db.pool)
    .await?;

    let lines: Vec<String> = rows
        .iter()
        .map(|row| {
            let id: String = row.get("id");
            let session_id: String = row.get("session_id");
            let agent_id: String = row.get("agent_id");
            let exception_type: String = row.get("exception_type");
            let severity: String = row.get("severity");
            let message: String = row.get("message");
            let created_at: i64 = row.get("created_at");
            serde_json::json!({
                "id": id,
                "session_id": session_id,
                "agent_id": agent_id,
                "exception_type": exception_type,
                "severity": severity,
                "message": message,
                "created_at": created_at,
            })
            .to_string()
        })
        .collect();

    Ok(lines.join("\n"))
}

/// §26.4 手动切换模型（返回可用模型列表）
#[tauri::command]
pub async fn model_switch_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let rows = sqlx::query(
        "SELECT m.model_id, m.display_name, m.max_tokens, p.name as provider_name, m.is_default FROM models m JOIN providers p ON p.id = m.provider_id WHERE m.kind = 'chat' ORDER BY m.is_default DESC",
    )
    .fetch_all(&state.db.pool)
    .await?;

    let models = rows
        .iter()
        .map(|row| {
            let model_id: String = row.get("model_id");
            let display_name: Option<String> = row.get("display_name");
            let provider_name: String = row.get("provider_name");
            let is_default: i64 = row.get("is_default");
            serde_json::json!({
                "model_id": model_id,
                "display_name": display_name,
                "provider_name": provider_name,
                "is_default": is_default == 1,
            })
        })
        .collect();

    Ok(models)
}

// ── 监控命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn monitor_get_budget(
    state: State<'_, crate::AppState>,
) -> Result<BudgetStatusDto, AppError> {
    budget_get_status(state).await
}

#[tauri::command]
pub async fn monitor_get_exceptions(
    state: State<'_, crate::AppState>,
    limit: Option<i64>,
) -> Result<Vec<crate::core::observability::exception::AgentException>, AppError> {
    let recorder = ExceptionRecorder::new(state.db.clone());
    let query = ExceptionQuery {
        session_id: None,
        agent_id: None,
        exception_type: None,
        severity: None,
        limit: Some(limit.unwrap_or(20)),
    };
    recorder.query(&query).await
}

// ── 护栏命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn guardrail_check_tool(
    tool_name: String,
    args: serde_json::Value,
) -> Result<crate::core::guardrails::tool_guard::GuardrailDecision, AppError> {
    let guard = ToolGuardrail::new(ToolPolicy::default());
    Ok(guard.check_tool_call(&tool_name, &args).await)
}
