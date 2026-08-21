use crate::data::models::{DashboardOverview, KanbanData};
use crate::data::services::dashboard_service::DashboardService;
use crate::data::services::AgentService;
use crate::utils::error::AppError;
use crate::AppState;

#[tauri::command]
pub async fn dashboard_overview(
    state: tauri::State<'_, AppState>,
) -> Result<DashboardOverview, AppError> {
    let agent_svc = AgentService::new(state.db.pool.clone());
    agent_svc.ensure_builtin_agents().await?;
    let svc = DashboardService::new(&state.db);
    svc.overview().await
}

#[tauri::command]
pub async fn dashboard_kanban(state: tauri::State<'_, AppState>) -> Result<KanbanData, AppError> {
    let svc = DashboardService::new(&state.db);
    svc.kanban(&state.session_state).await
}

#[tauri::command]
pub async fn dashboard_tasks(
    agent_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<crate::core::adk::task_tools::Task>, AppError> {
    let store = crate::core::adk::task_tools::task_store().read().await;
    let mut tasks: Vec<crate::core::adk::task_tools::Task> = store.iter().cloned().collect();
    if let Some(aid) = &agent_id {
        tasks.retain(|t| t.owner.as_deref() == Some(aid.as_str()));
    }
    if let Some(s) = &status {
        tasks.retain(|t| t.status == *s);
    }
    tasks.sort_by_key(|t| t.created_at);
    Ok(tasks)
}
