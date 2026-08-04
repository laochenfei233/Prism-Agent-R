use tauri::State;

use crate::data::models::AgentDto;
use crate::data::services::AgentService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn agent_list(state: State<'_, crate::AppState>) -> Result<Vec<AgentDto>, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.list().await
}

#[tauri::command]
pub async fn agent_get(state: State<'_, crate::AppState>, id: String) -> Result<AgentDto, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.get(&id).await
}

#[tauri::command]
pub async fn agent_create(
    state: State<'_, crate::AppState>,
    name: String,
    description: Option<String>,
    system_prompt: Option<String>,
    model_id: Option<String>,
) -> Result<AgentDto, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.create(
        &name,
        description.as_deref(),
        system_prompt.as_deref(),
        model_id.as_deref(),
    ).await
}

#[tauri::command]
pub async fn agent_update(
    state: State<'_, crate::AppState>,
    id: String,
    name: Option<String>,
    description: Option<String>,
    system_prompt: Option<String>,
    model_id: Option<String>,
) -> Result<AgentDto, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.update(
        &id,
        name.as_deref(),
        description.as_deref(),
        system_prompt.as_deref(),
        model_id.as_deref(),
    ).await
}

#[tauri::command]
pub async fn agent_delete(state: State<'_, crate::AppState>, id: String) -> Result<(), AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.delete(&id).await
}
