use serde::Serialize;
use tauri::{Emitter, State};

use crate::core::session::{SessionInitReport, SessionLifecycle};
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

// ── §17.1 会话生命周期命令 ──────────────────────────────────

/// §17.1 会话初始化：校验 Provider/MCP/记忆，返回初始化报告
#[tauri::command]
pub async fn session_init(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<SessionInitReport, AppError> {
    let report = state.session_state.init_session(
        &session_id,
        &state.db.pool,
        &state.mcp_runtime,
    ).await.map_err(|e| AppError::Internal(e))?;

    let lifecycle = state.session_state.get_state(&session_id).await;
    let _ = app.emit("session:state-changed", serde_json::json!({
        "session_id": session_id,
        "lifecycle": lifecycle,
        "report": report,
    }));

    Ok(report)
}

/// §17.1 查询会话状态
#[tauri::command]
pub async fn session_state_query(
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<SessionLifecycle, AppError> {
    Ok(state.session_state.get_state(&session_id).await)
}

/// §17.1 手动触发会话清理
#[tauri::command]
pub async fn session_cleanup(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<(), AppError> {
    state.session_state.complete(&session_id).await;
    let _ = app.emit("session:state-changed", serde_json::json!({
        "session_id": session_id,
        "lifecycle": SessionLifecycle::Done,
    }));
    Ok(())
}
