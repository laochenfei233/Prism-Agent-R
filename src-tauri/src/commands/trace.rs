use tauri::State;

use crate::data::services::trace_service::{AgentTrace, TraceService};
use crate::utils::error::AppError;

#[tauri::command]
pub async fn trace_list(
    state: State<'_, crate::AppState>,
    session_id: String,
    limit: Option<i64>,
) -> Result<Vec<AgentTrace>, AppError> {
    let svc = TraceService::new(state.db.clone());
    svc.list_traces(&session_id, limit).await
}
