use std::sync::Arc;

use sqlx::Row;
use tauri::{Emitter, State};

use crate::core::budget::config::BudgetConfig;
use crate::core::budget::policy::BudgetPolicy;
use crate::core::budget::tracker::BudgetTracker;
use crate::core::guardrails::tool_guard::{ToolGuardrail, ToolPolicy};
use crate::core::observability::exception::{ExceptionRecorder, ExceptionQuery};
use crate::core::orchestrator::session::OrchestratorSession;
use crate::core::orchestrator::engine::OrchestratorEngine;
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

// ── 自主编排命令 ──────────────────────────────────────────

#[tauri::command]
pub async fn orchestrator_start(
    app: tauri::AppHandle,
    _state: State<'_, crate::AppState>,
    user_request: String,
) -> Result<OrchestratorSession, AppError> {
    let session = OrchestratorSession::new(user_request, 5);
    let tracker = Arc::new(BudgetTracker::new(BudgetConfig::default(), BudgetPolicy::default()));
    let engine = OrchestratorEngine::new(tracker)
        .on_event({
            let app = app.clone();
            move |event| {
                let _ = app.emit("orchestrator:event", serde_json::json!({
                    "event_type": event.event_type,
                    "message": event.message,
                    "timestamp": event.timestamp,
                }));
            }
        });

    // Run in background
    let mut session_clone = session.clone();
    tokio::spawn(async move {
        let _ = engine.run(&mut session_clone).await;
    });

    Ok(session)
}

#[tauri::command]
pub async fn orchestrator_get_session(
    _session_id: String,
) -> Result<OrchestratorSession, AppError> {
    Ok(OrchestratorSession::new("placeholder".into(), 5))
}

// ── 工作流交互命令 ────────────────────────────────────────

/// 暂停工作流
#[tauri::command]
pub async fn workflow_pause(
    state: State<'_, crate::AppState>,
    run_id: String,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE workflow_runs SET status = 'paused', finished_at = ? WHERE id = ? AND status = 'running'",
    )
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(&run_id)
    .execute(&state.db.pool)
    .await?;
    Ok(())
}

/// 恢复工作流
#[tauri::command]
pub async fn workflow_resume(
    state: State<'_, crate::AppState>,
    run_id: String,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE workflow_runs SET status = 'running', finished_at = NULL WHERE id = ? AND status = 'paused'",
    )
    .bind(&run_id)
    .execute(&state.db.pool)
    .await?;
    Ok(())
}

/// 获取活跃工作流列表
#[tauri::command]
pub async fn monitor_list_active_workflows(
    state: State<'_, crate::AppState>,
) -> Result<Vec<ActiveWorkflowDto>, AppError> {
    let rows = sqlx::query(
        "SELECT id, workflow_id, status, source, created_at, finished_at FROM workflow_runs WHERE status IN ('running', 'paused') ORDER BY created_at DESC"
    )
    .fetch_all(&state.db.pool)
    .await?;

    let mut results = Vec::new();
    for row in rows {
        let id: String = row.try_get("id")?;
        let workflow_id: String = row.try_get("workflow_id")?;
        let status: String = row.try_get("status")?;
        let source: String = row.try_get("source").unwrap_or_default();
        let created_at: i64 = row.try_get("created_at")?;
        let finished_at: Option<i64> = row.try_get("finished_at")?;

        results.push(ActiveWorkflowDto {
            id,
            workflow_id,
            status,
            source,
            created_at,
            finished_at,
        });
    }
    Ok(results)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActiveWorkflowDto {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub source: String,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}
