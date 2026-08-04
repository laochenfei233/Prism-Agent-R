use tauri::State;

use crate::data::models::MessageDto;
use crate::data::services::ChatService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn chat_history(
    state: State<'_, crate::AppState>,
    session_id: String,
    limit: Option<i64>,
) -> Result<Vec<MessageDto>, AppError> {
    let svc = ChatService::new(state.db.pool.clone());
    svc.history(&session_id, limit).await
}

#[tauri::command]
pub async fn chat_send(
    state: State<'_, crate::AppState>,
    session_id: String,
    content: String,
) -> Result<MessageDto, AppError> {
    let svc = ChatService::new(state.db.pool.clone());
    svc.save_message(&session_id, "user", &content, None, None, None, None).await
}
