use std::collections::HashMap;
use tauri::State;

use crate::core::autoagents::workflow::{TaskDefinition, TaskValidationResult};
use crate::data::models::{WorkflowDto, WorkflowRow};
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

// ── 任务定义命令 ──────────────────────────────────────────

#[tauri::command]
pub async fn task_save_template(
    state: State<'_, crate::AppState>,
    definition: TaskDefinition,
) -> Result<WorkflowDto, AppError> {
    let pool = &state.db.pool;
    let def_json = serde_json::to_string(&definition)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    sqlx::query(
        "INSERT INTO workflows (id, name, description, definition, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&definition.name)
    .bind(&definition.description)
    .bind(&def_json)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    let row: WorkflowRow = sqlx::query_as("SELECT * FROM workflows WHERE id = ?")
        .bind(&id)
        .fetch_one(pool)
        .await?;

    Ok(WorkflowDto {
        id: row.id,
        name: row.name,
        description: row.description,
        definition: serde_json::from_str(&row.definition).unwrap_or(serde_json::json!({})),
    })
}

#[tauri::command]
pub async fn task_run(
    _state: State<'_, crate::AppState>,
    _definition: TaskDefinition,
    _inputs: HashMap<String, serde_json::Value>,
) -> Result<WorkflowRunResult, AppError> {
    let run_id = uuid::Uuid::new_v4().to_string();
    Ok(WorkflowRunResult {
        run_id,
        status: "pending".to_string(),
    })
}

#[tauri::command]
pub async fn task_validate(
    _state: State<'_, crate::AppState>,
    definition: TaskDefinition,
) -> Result<TaskValidationResult, AppError> {
    let mut errors = Vec::new();

    // 环检测：拓扑排序
    let mut sorted = Vec::new();
    let mut remaining = definition.stages.clone();
    let mut visited = std::collections::HashSet::new();

    loop {
        let mut progress = false;
        let mut i = 0;
        while i < remaining.len() {
            if remaining[i].depends_on.iter().all(|dep| visited.contains(dep.as_str())) {
                visited.insert(remaining[i].id.clone());
                sorted.push(remaining.remove(i));
                progress = true;
            } else {
                i += 1;
            }
        }
        if !progress {
            if !remaining.is_empty() {
                errors.push(format!(
                    "存在循环依赖，涉及阶段: {}",
                    remaining.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
            break;
        }
    }

    // 引用检查：depends_on 必须指向已定义的 stage id
    let stage_ids: std::collections::HashSet<&str> =
        definition.stages.iter().map(|s| s.id.as_str()).collect();
    for stage in &definition.stages {
        for dep in &stage.depends_on {
            if !stage_ids.contains(dep.as_str()) {
                errors.push(format!(
                    "阶段 '{}' 依赖的 '{}' 不存在",
                    stage.id, dep
                ));
            }
        }
    }

    // 模板变量引用检查：{{key}} 必须来自 inputs 或阶段输出
    let input_keys: std::collections::HashSet<&str> =
        definition.inputs.iter().map(|i| i.key.as_str()).collect();
    for stage in &definition.stages {
        let template = &stage.prompt_template;
        let mut pos = 0;
        while let Some(open) = template[pos..].find("{{") {
            let abs_open = pos + open;
            if let Some(close) = template[abs_open + 2..].find("}}") {
                let var = template[abs_open + 2..abs_open + 2 + close].trim();
                if var.contains('.') {
                    let stage_id = var.split('.').next().unwrap_or("");
                    if !stage_ids.contains(stage_id) {
                        errors.push(format!(
                            "阶段 '{}' 模板引用的阶段 '{}' 不存在",
                            stage.id, stage_id
                        ));
                    }
                } else if !input_keys.contains(var) {
                    errors.push(format!(
                        "阶段 '{}' 模板引用的变量 '{}' 未在 inputs 中定义",
                        stage.id, var
                    ));
                }
                pos = abs_open + 2 + close + 2;
            } else {
                break;
            }
        }
    }

    Ok(TaskValidationResult {
        ok: errors.is_empty(),
        errors,
    })
}

#[tauri::command]
pub async fn task_rerun(
    state: State<'_, crate::AppState>,
    workflow_id: String,
) -> Result<WorkflowRunResult, AppError> {
    let pool = &state.db.pool;

    // 验证 workflow 存在
    let _row: WorkflowRow = sqlx::query_as("SELECT * FROM workflows WHERE id = ?")
        .bind(&workflow_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::Validation(format!("任务定义 '{}' 不存在", workflow_id)))?;

    let run_id = uuid::Uuid::new_v4().to_string();
    Ok(WorkflowRunResult {
        run_id,
        status: "pending".to_string(),
    })
}
