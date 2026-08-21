use std::collections::HashMap;
use tauri::{Emitter, State};

use crate::data::models::McpServerDto;
use crate::data::services::mcp_service::{McpService, McpTestResult};
use crate::mcp::runtime::ServerStatusInfo;
use crate::mcp::transport::McpTool;
use crate::utils::error::AppError;

// ── 辅助函数 ──────────────────────────────────────────────

fn mcp_service(state: &State<'_, crate::AppState>) -> McpService {
    McpService::new(state.db.clone(), state.mcp_runtime.clone())
}

/// 通知前端 MCP 服务器状态/工具集变化（节流由前端处理）
fn emit_mcp_changed(app: &tauri::AppHandle, kind: &str) {
    let _ = app.emit("mcp:status-changed", serde_json::json!({ "kind": kind }));
    let _ = app.emit("mcp:tools-changed", serde_json::json!({ "kind": kind }));
}

// ── MCP 命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn mcp_list(state: State<'_, crate::AppState>) -> Result<Vec<McpServerDto>, AppError> {
    let svc = mcp_service(&state);
    svc.list().await
}

// Tauri commands must keep flat arg signatures (IPC contract); params mirror the frontend call.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn mcp_add(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    name: String,
    r#type: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    base_url: Option<String>,
    headers: Option<HashMap<String, String>>,
    timeout_ms: Option<i32>,
) -> Result<McpServerDto, AppError> {
    let svc = mcp_service(&state);
    let result = svc
        .add(
            name, r#type, command, args, env, base_url, headers, timeout_ms,
        )
        .await;
    if result.is_ok() {
        emit_mcp_changed(&app, "added");
    }
    result
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn mcp_update(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    id: String,
    name: Option<String>,
    r#type: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    base_url: Option<String>,
    timeout_ms: Option<i32>,
) -> Result<McpServerDto, AppError> {
    let svc = mcp_service(&state);
    let result = svc
        .update(&id, name, r#type, command, args, base_url, timeout_ms)
        .await;
    if result.is_ok() {
        emit_mcp_changed(&app, "updated");
    }
    result
}

#[tauri::command]
pub async fn mcp_remove(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = mcp_service(&state);
    let result = svc.remove(&id).await;
    if result.is_ok() {
        emit_mcp_changed(&app, "removed");
    }
    result
}

#[tauri::command]
pub async fn mcp_test(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<McpTestResult, AppError> {
    let svc = mcp_service(&state);
    let result = svc.test(&id).await;
    emit_mcp_changed(&app, "tested");
    result
}

#[tauri::command]
pub async fn mcp_tools(
    state: State<'_, crate::AppState>,
    server_id: Option<String>,
) -> Result<Vec<McpTool>, AppError> {
    let svc = mcp_service(&state);
    svc.tools(server_id.as_deref()).await
}

#[tauri::command]
pub async fn mcp_status_all(
    state: State<'_, crate::AppState>,
) -> Result<Vec<ServerStatusInfo>, AppError> {
    let svc = mcp_service(&state);
    Ok(svc.all_status().await)
}

#[tauri::command]
pub async fn mcp_call_tool(
    state: State<'_, crate::AppState>,
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    let result = state
        .mcp_runtime
        .call_tool(&server_id, &tool_name, arguments)
        .await?;
    Ok(serde_json::json!(result))
}
