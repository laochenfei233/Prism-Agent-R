use crate::AppState;
use crate::data::models::DashboardOverview;
use crate::data::services::dashboard_service::DashboardService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn dashboard_overview(
    state: tauri::State<'_, AppState>,
) -> Result<DashboardOverview, AppError> {
    let svc = DashboardService::new(&state.db);
    svc.overview().await
}
