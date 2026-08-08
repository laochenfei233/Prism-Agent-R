use std::sync::Arc;

use tauri::State;

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
    _state: State<'_, crate::AppState>,
    user_request: String,
) -> Result<OrchestratorSession, AppError> {
    let session = OrchestratorSession::new(user_request, 5);
    let tracker = BudgetTracker::new(BudgetConfig::default(), BudgetPolicy::default());
    let engine = OrchestratorEngine::new(Arc::new(tracker));

    // Run in background
    let session_clone = session.clone();
    tokio::spawn(async move {
        let mut s = session_clone;
        let _ = engine.run(&mut s).await;
    });

    Ok(session)
}

#[tauri::command]
pub async fn orchestrator_get_session(
    _session_id: String,
) -> Result<OrchestratorSession, AppError> {
    // For now, return a placeholder
    Ok(OrchestratorSession::new("placeholder".into(), 5))
}
