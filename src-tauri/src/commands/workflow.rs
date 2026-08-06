use std::collections::HashMap;
use tauri::State;

use crate::data::models::WorkflowDto;
use crate::data::services::workflow_service::WorkflowService;
use crate::utils::error::AppError;

// ── 工作流命令 ────────────────────────────────────────────

#[tauri::command]
pub async fn workflow_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<WorkflowDto>, AppError> {
    let svc = WorkflowService::new(state.db.clone());
    svc.list().await
}

#[tauri::command]
pub async fn workflow_run(
    _state: State<'_, crate::AppState>,
    _workflow_id: String,
    _inputs: HashMap<String, serde_json::Value>,
) -> Result<WorkflowRunResult, AppError> {
    // MVP 阶段简化实现：直接返回 run_id
    // 完整实现需要 Coordinator + WorkflowEngine
    let run_id = uuid::Uuid::new_v4().to_string();
    Ok(WorkflowRunResult {
        run_id,
        status: "pending".to_string(),
    })
}

#[tauri::command]
pub async fn workflow_stop(
    _state: State<'_, crate::AppState>,
    _run_id: String,
) -> Result<(), AppError> {
    // MVP 阶段简化实现
    Ok(())
}

#[tauri::command]
pub async fn workflow_result(
    _state: State<'_, crate::AppState>,
    run_id: String,
) -> Result<WorkflowResultDto, AppError> {
    // MVP 阶段简化实现
    Ok(WorkflowResultDto {
        run_id,
        status: "pending".to_string(),
        outputs: HashMap::new(),
        error: None,
    })
}

// ── 返回类型 ──────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowRunResult {
    pub run_id: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowResultDto {
    pub run_id: String,
    pub status: String,
    pub outputs: HashMap<String, String>,
    pub error: Option<String>,
}
