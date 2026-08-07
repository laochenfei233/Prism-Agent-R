use std::collections::HashMap;
use tauri::State;

use crate::data::models::McpServerDto;
use crate::data::services::mcp_service::{McpService, McpTestResult};
use crate::mcp::runtime::ServerStatusInfo;
use crate::mcp::transport::McpTool;
use crate::utils::error::AppError;

// ── 辅助函数 ──────────────────────────────────────────────

fn mcp_service(state: &State<'_, crate::AppState>) -> McpService {
    McpService::new(state.db.clone(), state.mcp_runtime.clone())
}

// ── MCP 命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn mcp_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<McpServerDto>, AppError> {
    let svc = mcp_service(&state);
    svc.list().await
}

#[tauri::command]
pub async fn mcp_add(
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
    svc.add(name, r#type, command, args, env, base_url, headers, timeout_ms).await
}

#[tauri::command]
pub async fn mcp_update(
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
    svc.update(&id, name, r#type, command, args, base_url, timeout_ms).await
}

#[tauri::command]
pub async fn mcp_remove(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = mcp_service(&state);
    svc.remove(&id).await
}

#[tauri::command]
pub async fn mcp_test(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<McpTestResult, AppError> {
    let svc = mcp_service(&state);
    svc.test(&id).await
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
    let result = state.mcp_runtime.call_tool(&server_id, &tool_name, arguments).await?;
    Ok(serde_json::json!(result))
}
