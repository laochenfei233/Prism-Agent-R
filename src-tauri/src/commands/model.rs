use tauri::State;

use crate::data::models::{ModelDto, ProviderDto};
use crate::data::services::ModelService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn model_list(state: State<'_, crate::AppState>) -> Result<Vec<ModelDto>, AppError> {
    let svc = ModelService::new(state.db.pool.clone());
    svc.list_models().await
}

#[tauri::command]
pub async fn model_providers(
    state: State<'_, crate::AppState>,
) -> Result<Vec<ProviderDto>, AppError> {
    let svc = ModelService::new(state.db.pool.clone());
    svc.list_providers().await
}
