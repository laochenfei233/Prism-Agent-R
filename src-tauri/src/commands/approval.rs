use tauri::State;

use crate::data::models::ApprovalRequestRow;
use crate::utils::error::AppError;

/// 查询审批历史（可按 Agent 过滤，默认最近 50 条）。
#[tauri::command]
pub async fn approval_history(
    state: State<'_, crate::AppState>,
    agent_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<ApprovalRequestRow>, AppError> {
    crate::data::services::approval_service::list_requests(
        &state.db.pool,
        agent_id.as_deref(),
        limit.unwrap_or(50).clamp(1, 200),
    )
    .await
}
