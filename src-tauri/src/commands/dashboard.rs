use crate::AppState;
use crate::data::models::DashboardOverview;
use crate::data::services::dashboard_service::DashboardService;
use crate::data::services::AgentService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn dashboard_overview(
    state: tauri::State<'_, AppState>,
) -> Result<DashboardOverview, AppError> {
    let agent_svc = AgentService::new(state.db.pool.clone());
    agent_svc.ensure_builtin_agents().await?;
    let svc = DashboardService::new(&state.db);
    svc.overview().await
}
