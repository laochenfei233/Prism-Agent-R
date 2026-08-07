use tauri::State;

use crate::data::models::SessionDto;
use crate::data::services::SessionService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn session_list(
    state: State<'_, crate::AppState>,
    agent_id: Option<String>,
) -> Result<Vec<SessionDto>, AppError> {
    let svc = SessionService::new(state.db.pool.clone());
    svc.list(agent_id.as_deref()).await
}

#[tauri::command]
pub async fn session_create(
    state: State<'_, crate::AppState>,
    agent_id: String,
    title: Option<String>,
) -> Result<SessionDto, AppError> {
    let svc = SessionService::new(state.db.pool.clone());
    svc.create(&agent_id, title.as_deref()).await
}

#[tauri::command]
pub async fn session_rename(
    state: State<'_, crate::AppState>,
    id: String,
    title: String,
) -> Result<SessionDto, AppError> {
    let svc = SessionService::new(state.db.pool.clone());
    svc.rename(&id, &title).await
}

#[tauri::command]
pub async fn session_delete(state: State<'_, crate::AppState>, id: String) -> Result<(), AppError> {
    let svc = SessionService::new(state.db.pool.clone());
    svc.delete(&id).await
}

#[tauri::command]
pub async fn session_search(
    state: State<'_, crate::AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<SessionDto>, AppError> {
    let svc = SessionService::new(state.db.pool.clone());
    svc.search(&query, limit.unwrap_or(20)).await
}
