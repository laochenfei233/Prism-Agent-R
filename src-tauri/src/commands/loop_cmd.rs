use serde::Deserialize;
use tauri::State;

use crate::core::autoagents::goal::TaskGoal;
use crate::core::autoagents::loop_scheduler::{AgentLoop, LoopKind};
use crate::utils::error::AppError;

/// Loop 创建请求
#[derive(Deserialize)]
pub struct LoopCreateRequest {
    pub kind: LoopKind,
    pub interval_secs: Option<u64>,
    pub max_rounds: Option<u32>,
    pub goal: Option<TaskGoal>,
    pub maker_workflow_id: Option<String>,
    pub checker_workflow_id: Option<String>,
}

/// §17.2 创建 Loop
#[tauri::command]
pub async fn loop_start(
    state: State<'_, crate::AppState>,
    request: LoopCreateRequest,
) -> Result<AgentLoop, AppError> {
    let loop_ = state.loop_scheduler.create_loop(
        request.kind,
        request.interval_secs,
        request.max_rounds.unwrap_or(5),
        request.goal,
        request.maker_workflow_id,
        request.checker_workflow_id,
    );
    Ok(loop_)
}

/// §17.2 停止 Loop
#[tauri::command]
pub async fn loop_stop(
    state: State<'_, crate::AppState>,
    loop_id: String,
) -> Result<bool, AppError> {
    Ok(state.loop_scheduler.stop_loop(&loop_id))
}

/// §17.2 列出所有 Loop
#[tauri::command]
pub async fn loop_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<AgentLoop>, AppError> {
    Ok(state.loop_scheduler.list_loops())
}
