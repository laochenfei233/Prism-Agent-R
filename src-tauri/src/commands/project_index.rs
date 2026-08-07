// §10.2.1 项目级自动索引 IPC 命令

use tauri::State;

use crate::data::models::ProjectIndexStatus;
use crate::utils::error::AppError;

/// 当前项目索引状态（开关/目录/已索引数/运行中）
#[tauri::command]
pub async fn project_index_status(
    state: State<'_, crate::AppState>,
) -> Result<ProjectIndexStatus, AppError> {
    crate::data::services::project_index::status(&state.db).await
}

/// 切换项目索引开关（preferences: project_index.enabled，默认开）
#[tauri::command]
pub async fn project_index_toggle(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    enabled: bool,
) -> Result<ProjectIndexStatus, AppError> {
    crate::data::services::project_index::toggle(&state.db, app, enabled).await
}

/// 全量重建项目索引（后台任务 + rag:progress 事件）
#[tauri::command]
pub async fn project_index_reindex(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
) -> Result<ProjectIndexStatus, AppError> {
    crate::data::services::project_index::reindex(state.db.clone(), app).await
}
