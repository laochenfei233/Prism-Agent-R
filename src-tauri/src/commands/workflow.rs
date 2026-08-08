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
use crate::core::autoagents::workflow_v2::{WorkflowV2, StageStatus as StageStatusV2};
use crate::core::autoagents::workflow_engine_v2::WorkflowEngineV2;
use crate::core::autoagents::{Coordinator, GenericActor};
use crate::core::budget::config::BudgetConfig;
use crate::core::budget::policy::BudgetPolicy;
use crate::core::budget::tracker::BudgetTracker;
use crate::core::observability::exception::ExceptionRecorder;
use crate::core::observability::logger::{AgentLogger, LogLevel};
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

    let svc = WorkflowService::new(state.db.clone());
    svc.ensure_schema().await?;

    let run_id = uuid::Uuid::new_v4().to_string();

    // 尝试解析为 V2 工作流（优先），失败则回退到 V1
    let status = if let Ok(workflow_v2) = serde_json::from_str::<WorkflowV2>(&row.definition) {
        run_workflow_v2(&app, state.inner(), &workflow_v2, inputs, &run_id, &workflow_id, "workflow").await?
    } else {
        let workflow: Workflow = serde_json::from_str(&row.definition)?;
        run_workflow(&app, state.inner(), &workflow, inputs, &run_id, &workflow_id, "workflow").await?
    };

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
    let api_key = provider_row.api_key_enc.as_deref().map(crate::commands::settings::decrypt_provider_key).unwrap_or_default();

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

    // §15 构建 web_search 工具（供工作流角色使用）
    let search_config = crate::commands::search::get_search_config(pool).await;
    let search_service = std::sync::Arc::new(
        crate::core::search::service::SearchService::from_config(&search_config)
    );

    for role in roles {
        let system_prompt = format!(
            "你是一个「{role}」角色的专业助手。请严格按照任务提示完成工作，只输出结果内容本身。"
        );
        let mut registry = ToolRegistry::new();
        // 注册 web_search 工具
        registry.register(Box::new(
            crate::core::search::web_search::WebSearchTool::new(search_service.clone())
        ));
        let actor = Arc::new(GenericActor::new(
            role.clone(),
            role,
            provider.clone(),
            system_prompt,
            registry,
        ));
        coordinator.register(actor).await;
    }
    Ok(coordinator)
}

// ── V2 工作流执行 ────────────────────────────────────────

/// 执行 V2 工作流：集成预算追踪、工具护栏、异常记录、重试策略
async fn run_workflow_v2(
    app: &tauri::AppHandle,
    state: &crate::AppState,
    workflow: &WorkflowV2,
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
                input.default.clone().unwrap_or(serde_json::Value::Null)
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

    // 构建 Coordinator
    let coordinator = build_coordinator_v2(&pool, workflow).await?;

    // 构建预算追踪器（§22.4 预算事件 → 前端）
    let budget_tracker = Arc::new(
        BudgetTracker::new(BudgetConfig::default(), BudgetPolicy::default()).on_event({
            let app = app.clone();
            move |event| {
                let event_name = match event.event_type.as_str() {
                    "warning" => "budget:warning",
                    "exceeded" => "budget:exceeded",
                    "model_switched" => "budget:model-switched",
                    "paused" => "budget:paused",
                    _ => return,
                };
                let _ = app.emit(event_name, serde_json::json!({
                    "level": event.level,
                    "current": event.current,
                    "limit": event.limit,
                    "entity_type": event.entity_type,
                    "entity_id": event.entity_id,
                    "action": event.action,
                    "message": event.message,
                    "timestamp": event.timestamp,
                }));
            }
        }),
    );

    // 构建异常记录器（§24.3 monitor:exception → 前端）
    let exception_recorder = Arc::new(
        ExceptionRecorder::new(state.db.clone()).on_exception({
            let app = app.clone();
            move |exc: &crate::core::observability::exception::AgentException| {
                let _ = app.emit("monitor:exception", serde_json::json!({
                    "id": exc.id,
                    "session_id": exc.session_id,
                    "agent_id": exc.agent_id,
                    "exception_type": exc.exception_type,
                    "severity": exc.severity,
                    "message": exc.message,
                    "created_at": exc.created_at,
                }));
            }
        }),
    );

    // 构建日志器
    let logger = Arc::new(AgentLogger::new(LogLevel::Info));

    // §22.3 模型降级链：按成本升序排列候选模型
    let fallback_chain = Arc::new(std::sync::RwLock::new(
        crate::core::budget::fallback::ModelFallbackChain::new(
            model_candidates(&pool, workflow).await?,
        ),
    ));

    // §23.4 系统级沙箱：默认策略
    let sandbox = Arc::new(crate::core::guardrails::sandbox::SandboxPolicy::default());

    // §23.3 行为级护栏：默认轨迹检查
    let trajectory_guard = Arc::new(
        crate::core::guardrails::trajectory::TrajectoryGuardrail::new(
            crate::core::guardrails::trajectory::ViolationHandler::LogOnly,
        ),
    );

    // 构建 V2 引擎
    let engine = WorkflowEngineV2::new(coordinator, budget_tracker)
        .with_exception_recorder(exception_recorder)
        .with_logger(logger)
        .with_model_fallback(fallback_chain)
        .with_sandbox(sandbox)
        .with_trajectory_guard(trajectory_guard)
        .on_stage({
            let app = app.clone();
            move |rid: &str, stage_id: &str, status: &StageStatusV2| {
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
                .any(|s| matches!(&s.status, StageStatusV2::Failed));
            let status = if failed { "failed" } else { "done" };
            let out = serde_json::to_string(&res.outputs).unwrap_or_else(|_| "{}".into());
            let err = res
                .stage_results
                .iter()
                .find(|s| matches!(&s.status, StageStatusV2::Failed))
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

/// §22.3 构建模型降级链候选：默认模型优先，其余模型按成本估算升序排列
async fn model_candidates(
    pool: &sqlx::SqlitePool,
    workflow: &WorkflowV2,
) -> Result<Vec<crate::core::budget::fallback::ModelCandidate>, AppError> {
    use crate::core::budget::fallback::ModelCandidate;

    // 工作流显式指定的降级链（model_fallback: Vec<String> 为 model_id 列表）
    let explicit = workflow.model_fallback.clone().unwrap_or_default();

    let rows = sqlx::query(
        "SELECT m.id, m.provider_id, m.model_id, m.display_name, m.max_tokens, m.is_default FROM models m WHERE m.kind = 'chat' ORDER BY m.is_default DESC, m.created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut candidates: Vec<ModelCandidate> = rows.iter().map(|row| {
        let model_id: String = row.get("model_id");
        let provider_id: String = row.get("provider_id");
        let display_name: Option<String> = row.get("display_name");
        let max_tokens: Option<i64> = row.get("max_tokens");
        // 成本估算：没有单价表时按模型名启发式（name 含 mini/lite/7b 更便宜）
        let cost_per_1k_tokens = estimate_model_cost(&model_id);
        let display = display_name.unwrap_or_else(|| model_id.clone());
        ModelCandidate {
            provider_id,
            model_id,
            display_name: display,
            cost_per_1k_tokens,
            max_tokens: max_tokens.unwrap_or(8192) as u64,
            capabilities: vec!["chat".into(), "tool_use".into()],
        }
    }).collect();

    // 显式降级链排在最前（按指定顺序），且不在数据库中的兜底
    for model_id in explicit {
        if !candidates.iter().any(|c| c.model_id == model_id) {
            candidates.insert(0, ModelCandidate {
                provider_id: "default".into(),
                model_id: model_id.clone(),
                display_name: model_id,
                cost_per_1k_tokens: 0.0,
                max_tokens: 8192,
                capabilities: vec!["chat".into(), "tool_use".into()],
            });
        }
    }

    // ModelFallbackChain::new 会按成本升序排序
    Ok(candidates)
}

/// 无单价表时的成本启发式估算（越低越便宜）
fn estimate_model_cost(model_id: &str) -> f64 {
    let lower = model_id.to_lowercase();
    if lower.contains("mini") || lower.contains("lite") || lower.contains("flash")
        || lower.contains("nano") || lower.contains("-7b") || lower.contains("8b") {
        0.001
    } else if lower.contains("haiku") || lower.contains("sonnet") || lower.contains("4o") {
        0.01
    } else if lower.contains("opus") || lower.contains("pro") || lower.contains("turbo")
        || lower.contains("deepseek-r1") {
        0.05
    } else {
        0.02
    }
}

/// 构建 V2 Coordinator：为工作流的每个角色注册一个 GenericActor
async fn build_coordinator_v2(
    pool: &sqlx::SqlitePool,
    workflow: &WorkflowV2,
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
    let api_key = provider_row.api_key_enc.as_deref().map(crate::commands::settings::decrypt_provider_key).unwrap_or_default();

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

    // §15 构建 web_search 工具（供工作流角色使用，对齐 V1 build_coordinator）
    let search_config = crate::commands::search::get_search_config(pool).await;
    let search_service = std::sync::Arc::new(
        crate::core::search::service::SearchService::from_config(&search_config)
    );

    for role in roles {
        let system_prompt = format!(
            "你是一个「{role}」角色的专业助手。请严格按照任务提示完成工作，只输出结果内容本身。"
        );
        let mut registry = ToolRegistry::new();
        // 注册 web_search 工具
        registry.register(Box::new(
            crate::core::search::web_search::WebSearchTool::new(search_service.clone())
        ));
        let actor = Arc::new(GenericActor::new(
            role.clone(),
            role,
            provider.clone(),
            system_prompt,
            registry,
        ));
        coordinator.register(actor).await;
    }
    Ok(coordinator)
}

// ── 目标监控（§10.11） ────────────────────────────────────

/// 用给定输出快照评估目标达成度（工作流运行面板 /goal-monitor）
#[tauri::command]
pub async fn goal_evaluate(
    description: String,
    criteria: Vec<serde_json::Value>,
    outputs: HashMap<String, String>,
) -> Result<crate::core::autoagents::goal::GoalStatus, AppError> {
    use crate::core::autoagents::goal::{CriterionOp, GoalCriterion, GoalMonitor, TaskGoal};

    let mut parsed = Vec::new();
    for c in criteria {
        let metric = c["metric"].as_str().unwrap_or("").to_string();
        let weight = c["weight"].as_f64().unwrap_or(0.0) as f32;
        let value = c["value"].clone();
        let operator = match c["operator"].as_str().unwrap_or("contains") {
            "gt" => CriterionOp::Gt,
            "lt" => CriterionOp::Lt,
            "eq" => CriterionOp::Eq,
            "not_contains" | "notcontains" => CriterionOp::NotContains,
            "regex" | "regex_match" => CriterionOp::RegexMatch,
            "llm_judge" | "llm" => CriterionOp::LlmJudge,
            _ => CriterionOp::Contains,
        };
        parsed.push(GoalCriterion { metric, operator, value, weight });
    }

    let goals = vec![TaskGoal { description, criteria: parsed, timeout_secs: None }];
    let monitor = GoalMonitor::new(goals);
    let state = crate::core::autoagents::goal::WorkflowState {
        stage_outputs: outputs.clone(),
        accumulated_text: outputs.values().cloned().collect::<Vec<_>>().join("\n\n"),
    };
    Ok(monitor.evaluate(&state))
}
