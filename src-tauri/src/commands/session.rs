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
    let report = state
        .session_state
        .init_session(&session_id, &state.db.pool, &state.mcp_runtime)
        .await
        .map_err(AppError::Internal)?;

    let lifecycle = state.session_state.get_state(&session_id).await;
    let _ = app.emit(
        "session:state-changed",
        serde_json::json!({
            "session_id": session_id,
            "lifecycle": lifecycle,
            "report": report,
        }),
    );

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
    let _ = app.emit(
        "session:state-changed",
        serde_json::json!({
            "session_id": session_id,
            "lifecycle": SessionLifecycle::Done,
        }),
    );
    Ok(())
}

/// §19.3.2 会话 fork：从指定 turn 分支新会话
#[tauri::command]
pub async fn session_fork(
    state: State<'_, crate::AppState>,
    session_id: String,
    _turn_id: String,
) -> Result<SessionDto, AppError> {
    // 获取原会话信息
    let svc = SessionService::new(state.db.pool.clone());
    let original = svc.get(&session_id).await?;

    // 创建新会话（继承 agent_id）
    let new_session = svc
        .create(
            &original.agent_id,
            Some(&format!("{} (分支)", original.title.unwrap_or_default())),
        )
        .await?;

    // 这里简化处理：fork 会复制原会话的历史到新会话
    // 完整实现需要复制 messages 表中的消息

    Ok(new_session)
}

/// §19.3.3 双向审批：工具调用审批响应
///
/// 兼容现有 tool:approval-response，新增 session:approve 语义：
/// - decision ∈ {allow, deny}
/// - always_allow 可选：持久化该工具的自动放行
#[tauri::command]
pub async fn session_approve(
    state: State<'_, crate::AppState>,
    call_id: String,
    decision: String,
    _always_allow: Option<bool>,
) -> Result<bool, AppError> {
    use crate::core::adk::tool::ToolApprovalResponse;

    let parsed = match decision.as_str() {
        "allow" | "Approved" => ToolApprovalResponse::Approved,
        "deny" | "Rejected" => ToolApprovalResponse::Rejected("用户拒绝".to_string()),
        other => ToolApprovalResponse::Rejected(other.to_string()),
    };

    // always_allow 逻辑简化：只处理基本审批，后续可扩展
    // 完整实现需要从 call_id 反查工具名

    Ok(state.approval_store.respond(&call_id, parsed).await)
}
