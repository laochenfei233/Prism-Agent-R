use tauri::State;

use crate::data::services::trace_service::{AgentTrace, TraceService};
use crate::utils::error::AppError;

#[tauri::command]
pub async fn trace_list(
    state: State<'_, crate::AppState>,
    session_id: String,
    limit: Option<i64>,
    min_grade: Option<f64>,
    tool_failed: Option<bool>,
) -> Result<Vec<AgentTrace>, AppError> {
    let svc = TraceService::new(state.db.clone());
    svc.list_traces_with_filter(&session_id, limit, min_grade, tool_failed).await
}

/// §17.3 单条轨迹评分回写
#[tauri::command]
pub async fn trace_grade(
    state: State<'_, crate::AppState>,
    trace_id: String,
    score: f64,
    reason: String,
) -> Result<(), AppError> {
    let svc = TraceService::new(state.db.clone());
    svc.grade_trace(&trace_id, score, &reason).await
}
