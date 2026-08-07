use std::collections::HashMap;
use std::sync::Arc;

use sqlx::Row;
use tauri::{Emitter, State};

use crate::core::adk::model::ModelProvider;
use crate::core::adk::tool::ToolRegistry;
use crate::core::autoagents::scheduler::global as global_scheduler;
use crate::core::autoagents::workflow::{
    StageStatus, TaskDefinition, TaskValidationResult, Workflow, WorkflowEngine,
};
use crate::core::autoagents::{Coordinator, GenericActor};
use crate::core::rig::provider::OpenAiProvider;
use crate::data::models::{ModelRow, ProviderRow, WorkflowDto, WorkflowRow};
use crate::data::services::workflow_service::WorkflowService;
use crate::utils::error::AppError;

// ── 工作流命令 ────────────────────────────────────────────

#[tauri::command]
pub async fn workflow_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<WorkflowDto>, AppError> {
    let svc = WorkflowService::new(state.db.clone());
    svc.ensure_builtin_workflows().await?;
    svc.list().await
}

#[tauri::command]
pub async fn workflow_run(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    workflow_id: String,
    inputs: HashMap<String, serde_json::Value>,
) -> Result<WorkflowRunResult, AppError> {
    let row: WorkflowRow = sqlx::query_as(
        "SELECT id, name, description, definition, created_at, updated_at FROM workflows WHERE id = ?",
    )
    .bind(&workflow_id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::Validation(format!("工作流 '{workflow_id}' 不存在")))?;

    let workflow: Workflow = serde_json::from_str(&row.definition)?;

    let svc = WorkflowService::new(state.db.clone());
    svc.ensure_schema().await?;

    let run_id = uuid::Uuid::new_v4().to_string();
    let status = run_workflow(&app, state.inner(), &workflow, inputs, &run_id, &workflow_id, "workflow").await?;
    Ok(WorkflowRunResult { run_id, status })
}

#[tauri::command]
pub async fn workflow_stop(
    state: State<'_, crate::AppState>,
    run_id: String,
) -> Result<(), AppError> {
    // 尽力标记取消；任务内联执行，无法中断模型调用，但记录最终状态
    sqlx::query(
        "UPDATE workflow_runs SET status = 'cancelled', finished_at = ? WHERE id = ? AND status = 'running'",
    )
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(&run_id)
    .execute(&state.db.pool)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn workflow_result(
    state: State<'_, crate::AppState>,
    run_id: String,
) -> Result<WorkflowResultDto, AppError> {
    let row = sqlx::query("SELECT status, outputs, error FROM workflow_runs WHERE id = ?")
        .bind(&run_id)
        .fetch_optional(&state.db.pool)
        .await?;

    let Some(row) = row else {
        return Ok(WorkflowResultDto {
            run_id,
            status: "not_found".to_string(),
            outputs: HashMap::new(),
            error: None,
        });
    };

    let status: String = row.get("status");
    let outputs = row
        .get::<Option<String>, _>("outputs")
        .and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok())
        .unwrap_or_default();
    let error: Option<String> = row.get("error");

    Ok(WorkflowResultDto {
        run_id,
        status,
        outputs,
        error,
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
pub async fn task_list_templates(
    state: State<'_, crate::AppState>,
) -> Result<Vec<WorkflowDto>, AppError> {
    let svc = WorkflowService::new(state.db.clone());
    svc.list_templates().await
}

#[tauri::command]
pub async fn task_run(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    definition: TaskDefinition,
    inputs: HashMap<String, serde_json::Value>,
) -> Result<WorkflowRunResult, AppError> {
    let workflow: Workflow = definition.into();
    let workflow_id = workflow.id.clone();

    let svc = WorkflowService::new(state.db.clone());
    svc.ensure_schema().await?;

    let run_id = uuid::Uuid::new_v4().to_string();
    let status = run_workflow(&app, state.inner(), &workflow, inputs, &run_id, &workflow_id, "task").await?;
    Ok(WorkflowRunResult { run_id, status })
}

#[tauri::command]
pub async fn task_rerun(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    run_id: String,
    inputs: Option<HashMap<String, serde_json::Value>>,
) -> Result<WorkflowRunResult, AppError> {
    // 从 workflow_runs 查回 workflow_id 与原输入
    let row = sqlx::query("SELECT workflow_id, inputs FROM workflow_runs WHERE id = ?")
        .bind(&run_id)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::Validation(format!("运行记录 '{run_id}' 不存在")))?;

    let workflow_id: String = row.get("workflow_id");
    let original_inputs: String = row.get("inputs");

    let wf_row: WorkflowRow = sqlx::query_as(
        "SELECT id, name, description, definition, created_at, updated_at FROM workflows WHERE id = ?",
    )
    .bind(&workflow_id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::Validation(format!("任务定义 '{workflow_id}' 不存在")))?;

    let workflow: Workflow = serde_json::from_str(&wf_row.definition)?;

    // 合并输入：新值覆盖原值
    let mut merged: HashMap<String, serde_json::Value> =
        serde_json::from_str(&original_inputs).unwrap_or_default();
    if let Some(new_inputs) = inputs {
        merged.extend(new_inputs);
    }

    let svc = WorkflowService::new(state.db.clone());
    svc.ensure_schema().await?;

    let new_run_id = uuid::Uuid::new_v4().to_string();
    let status = run_workflow(&app, state.inner(), &workflow, merged, &new_run_id, &workflow_id, "task").await?;
    Ok(WorkflowRunResult { run_id: new_run_id, status })
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

// ── 执行辅助 ──────────────────────────────────────────────

/// 执行工作流：写入运行记录、构建 Coordinator + GenericActor、运行引擎、
/// 写入最终状态并发出 workflow:stage / workflow:done 事件。
/// 返回最终状态（done / failed / cancelled）。
async fn run_workflow(
    app: &tauri::AppHandle,
    state: &crate::AppState,
    workflow: &Workflow,
    inputs: HashMap<String, serde_json::Value>,
    run_id: &str,
    workflow_id: &str,
    source: &str,
) -> Result<String, AppError> {
    // 用模板默认值补齐缺失输入
    let mut merged_inputs = inputs;
    for input in &workflow.inputs {
        merged_inputs
            .entry(input.key.clone())
            .or_insert_with(|| {
                input
                    .default
                    .clone()
                    .unwrap_or(serde_json::Value::Null)
            });
    }

    let pool = state.db.pool.clone();

    // 写入运行记录
    let now = chrono::Utc::now().timestamp_millis();
    let inputs_json = serde_json::to_string(&merged_inputs)?;
    sqlx::query(
        "INSERT INTO workflow_runs (id, workflow_id, status, inputs, outputs, error, created_at, finished_at, source) VALUES (?, ?, 'running', ?, NULL, NULL, ?, NULL, ?)",
    )
    .bind(run_id)
    .bind(workflow_id)
    .bind(&inputs_json)
    .bind(now)
    .bind(source)
    .execute(&pool)
    .await?;

    // 构建默认 Coordinator（每角色一个 GenericActor，工具注册表为空）
    let coordinator = build_coordinator(&pool, workflow).await?;

    let engine = WorkflowEngine::new(coordinator).on_stage({
        let app = app.clone();
        move |rid: &str, stage_id: &str, status: &StageStatus| {
            let _ = app.emit("workflow:stage", serde_json::json!({
                "run_id": rid,
                "stage_id": stage_id,
                "status": status.as_str(),
            }));
        }
    });

    let _permit = global_scheduler().acquire().await;
    let result = engine.run(workflow, merged_inputs, run_id).await;
    drop(_permit);

    let (final_status, outputs_json, error_msg) = match result {
        Ok(res) => {
            let failed = res
                .stage_results
                .iter()
                .any(|s| matches!(&s.status, StageStatus::Failed));
            let status = if failed { "failed" } else { "done" };
            let out = serde_json::to_string(&res.outputs).unwrap_or_else(|_| "{}".into());
            let err = res
                .stage_results
                .iter()
                .find(|s| matches!(&s.status, StageStatus::Failed))
                .and_then(|s| s.error.clone());
            (status.to_string(), Some(out), err)
        }
        Err(e) => ("failed".to_string(), None, Some(e.to_string())),
    };

    let finished_at = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "UPDATE workflow_runs SET status = ?, outputs = ?, error = ?, finished_at = ? WHERE id = ?",
    )
    .bind(&final_status)
    .bind(&outputs_json)
    .bind(&error_msg)
    .bind(finished_at)
    .bind(run_id)
    .execute(&pool)
    .await?;

    let _ = app.emit("workflow:done", serde_json::json!({
        "run_id": run_id,
        "status": final_status,
    }));

    Ok(final_status)
}

/// 构建默认 Coordinator：为工作流的每个角色注册一个 GenericActor（内部用 RigAgent），
/// 使用默认模型对应的 Provider，工具注册表为空。
async fn build_coordinator(
    pool: &sqlx::SqlitePool,
    workflow: &Workflow,
) -> Result<Arc<Coordinator>, AppError> {
    let mut roles: Vec<String> = workflow.stages.iter().map(|s| s.role.clone()).collect();
    roles.sort();
    roles.dedup();

    let model_row = sqlx::query_as::<_, ModelRow>(
        "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Validation("未配置默认模型。请在设置中添加 Provider 并设置默认模型。".into()))?;

    let provider_row = sqlx::query_as::<_, ProviderRow>(
        "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?",
    )
    .bind(&model_row.provider_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider(format!("Provider 不存在: {}", model_row.provider_id)))?;

    let base_url = provider_row.base_url.unwrap_or_else(|| {
        match provider_row.kind.as_str() {
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        }
    });
    let api_key = provider_row.api_key_enc.unwrap_or_default();

    let provider: Arc<dyn ModelProvider> = Arc::new(OpenAiProvider::new(
        model_row.provider_id.clone(),
        model_row
            .display_name
            .clone()
            .unwrap_or_else(|| model_row.model_id.clone()),
        api_key,
        base_url,
        model_row.model_id.clone(),
    ));

    let coordinator = Arc::new(Coordinator::new());
    for role in roles {
        let system_prompt = format!(
            "你是一个「{role}」角色的专业助手。请严格按照任务提示完成工作，只输出结果内容本身。"
        );
        let actor = Arc::new(GenericActor::new(
            role.clone(),
            role,
            provider.clone(),
            system_prompt,
            ToolRegistry::new(),
        ));
        coordinator.register(actor).await;
    }
    Ok(coordinator)
}
