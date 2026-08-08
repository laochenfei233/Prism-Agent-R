use std::sync::Arc;

use sqlx::Row;
use tauri::{Emitter, State};

use crate::core::adk::model::ModelProvider;
use crate::core::budget::config::BudgetConfig;
use crate::core::budget::policy::BudgetPolicy;
use crate::core::budget::tracker::BudgetTracker;
use crate::core::guardrails::tool_guard::{ToolGuardrail, ToolPolicy};
use crate::core::observability::exception::{ExceptionRecorder, ExceptionQuery};
use crate::core::orchestrator::session::OrchestratorSession;
use crate::core::orchestrator::engine::OrchestratorEngine;
use crate::core::rig::provider::OpenAiProvider;
use crate::data::models::{ModelRow, ProviderRow};
use crate::utils::error::AppError;

// ── 预算命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn budget_get_config(
    _state: State<'_, crate::AppState>,
) -> Result<BudgetConfig, AppError> {
    // Return default config for now
    Ok(BudgetConfig::default())
}

#[tauri::command]
pub async fn budget_get_status(
    _state: State<'_, crate::AppState>,
) -> Result<BudgetStatusDto, AppError> {
    let tracker = BudgetTracker::new(BudgetConfig::default(), BudgetPolicy::default());
    let snapshot = tracker.snapshot().await;
    Ok(BudgetStatusDto {
        daily_tokens_used: snapshot.daily_tokens_used,
        daily_tokens_limit: 1_000_000,
        daily_cost_used: snapshot.daily_cost_used,
        daily_cost_limit: 10.0,
        monthly_cost_used: snapshot.monthly_cost_used,
        monthly_cost_limit: 200.0,
        active_workflows: snapshot.active_workflows,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetStatusDto {
    pub daily_tokens_used: u64,
    pub daily_tokens_limit: u64,
    pub daily_cost_used: f64,
    pub daily_cost_limit: f64,
    pub monthly_cost_used: f64,
    pub monthly_cost_limit: f64,
    pub active_workflows: u32,
}

// ── 异常命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn exception_list(
    state: State<'_, crate::AppState>,
    session_id: Option<String>,
    agent_id: Option<String>,
    exception_type: Option<String>,
    severity: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<crate::core::observability::exception::AgentException>, AppError> {
    let recorder = ExceptionRecorder::new(state.db.clone());
    let query = ExceptionQuery {
        session_id,
        agent_id,
        exception_type,
        severity,
        limit,
    };
    recorder.query(&query).await
}

#[tauri::command]
pub async fn exception_resolve(
    state: State<'_, crate::AppState>,
    exception_id: String,
    resolution: String,
) -> Result<(), AppError> {
    let recorder = ExceptionRecorder::new(state.db.clone());
    recorder.resolve(&exception_id, "user", &resolution).await
}

/// §26.4 清除已处理的异常（可保留未处理的）
#[tauri::command]
pub async fn exception_clear(
    state: State<'_, crate::AppState>,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM agent_exceptions WHERE resolved_at IS NOT NULL")
        .execute(&state.db.pool)
        .await?;
    Ok(())
}

/// §26.4 导出结构化日志（JSON Lines）
#[tauri::command]
pub async fn log_export(
    state: State<'_, crate::AppState>,
) -> Result<String, AppError> {
    let rows = sqlx::query(
        "SELECT id, session_id, agent_id, exception_type, severity, message, created_at FROM agent_exceptions ORDER BY created_at DESC LIMIT 500",
    )
    .fetch_all(&state.db.pool)
    .await?;

    let lines: Vec<String> = rows.iter().map(|row| {
        let id: String = row.get("id");
        let session_id: String = row.get("session_id");
        let agent_id: String = row.get("agent_id");
        let exception_type: String = row.get("exception_type");
        let severity: String = row.get("severity");
        let message: String = row.get("message");
        let created_at: i64 = row.get("created_at");
        serde_json::json!({
            "id": id,
            "session_id": session_id,
            "agent_id": agent_id,
            "exception_type": exception_type,
            "severity": severity,
            "message": message,
            "created_at": created_at,
        }).to_string()
    }).collect();

    Ok(lines.join("\n"))
}

/// §26.4 手动切换模型（返回可用模型列表）
#[tauri::command]
pub async fn model_switch_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let rows = sqlx::query(
        "SELECT m.model_id, m.display_name, m.max_tokens, p.name as provider_name, m.is_default FROM models m JOIN providers p ON p.id = m.provider_id WHERE m.kind = 'chat' ORDER BY m.is_default DESC",
    )
    .fetch_all(&state.db.pool)
    .await?;

    let models = rows.iter().map(|row| {
        let model_id: String = row.get("model_id");
        let display_name: Option<String> = row.get("display_name");
        let provider_name: String = row.get("provider_name");
        let is_default: i64 = row.get("is_default");
        serde_json::json!({
            "model_id": model_id,
            "display_name": display_name,
            "provider_name": provider_name,
            "is_default": is_default == 1,
        })
    }).collect();

    Ok(models)
}

// ── 监控命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn monitor_get_budget(
    state: State<'_, crate::AppState>,
) -> Result<BudgetStatusDto, AppError> {
    budget_get_status(state).await
}

#[tauri::command]
pub async fn monitor_get_exceptions(
    state: State<'_, crate::AppState>,
    limit: Option<i64>,
) -> Result<Vec<crate::core::observability::exception::AgentException>, AppError> {
    let recorder = ExceptionRecorder::new(state.db.clone());
    let query = ExceptionQuery {
        session_id: None,
        agent_id: None,
        exception_type: None,
        severity: None,
        limit: Some(limit.unwrap_or(20)),
    };
    recorder.query(&query).await
}

// ── 护栏命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn guardrail_check_tool(
    tool_name: String,
    args: serde_json::Value,
) -> Result<crate::core::guardrails::tool_guard::GuardrailDecision, AppError> {
    let guard = ToolGuardrail::new(ToolPolicy::default());
    Ok(guard.check_tool_call(&tool_name, &args).await)
}

// ── 自主编排命令 ──────────────────────────────────────────

#[tauri::command]
pub async fn orchestrator_start(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    user_request: String,
) -> Result<OrchestratorSession, AppError> {
    let session = OrchestratorSession::new(user_request, 5);
    let tracker = Arc::new(BudgetTracker::new(BudgetConfig::default(), BudgetPolicy::default()));
    let mut engine = OrchestratorEngine::new(tracker)
        .with_db(state.db.pool.clone())
        .on_event({
            let app = app.clone();
            move |event| {
                let _ = app.emit("orchestrator:event", serde_json::json!({
                    "event_type": event.event_type,
                    "message": event.message,
                    "timestamp": event.timestamp,
                }));
            }
        });

    // §27.3 配置 Planner/Reviewer 模型（默认模型）
    match build_default_provider(&state.db.pool).await {
        Ok(provider) => {
            engine = engine.with_planner_provider(provider);
        }
        Err(e) => {
            tracing::warn!("编排未配置默认模型，将使用骨架执行: {e}");
        }
    }

    // §27.2 初始持久化（崩溃可恢复起点）
    if let Err(e) = session.save(&state.db.pool).await {
        tracing::warn!("编排会话初始持久化失败: {e}");
    }

    // Run in background
    let mut session_clone = session.clone();
    tokio::spawn(async move {
        let _ = engine.run(&mut session_clone).await;
    });

    Ok(session)
}

/// §27.2 恢复已持久化的编排会话
#[tauri::command]
pub async fn orchestrator_resume(
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<OrchestratorSession, AppError> {
    let session = OrchestratorSession::load(&state.db.pool, &session_id)
        .await
        .map_err(|e| AppError::Internal(e))?
        .ok_or_else(|| AppError::Validation(format!("编排会话 '{session_id}' 不存在")))?;
    Ok(session)
}

/// §27.2 列出已持久化的编排会话
#[tauri::command]
pub async fn orchestrator_list(
    state: State<'_, crate::AppState>,
    limit: Option<i64>,
) -> Result<Vec<OrchestratorSession>, AppError> {
    let limit = limit.unwrap_or(20);
    let rows = sqlx::query(
        "SELECT id, user_request, spec, plan, status, cycle_count, max_cycles, history, created_at, updated_at FROM orchestrator_sessions ORDER BY updated_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db.pool)
    .await?;

    let mut sessions = Vec::new();
    for row in rows {
        let session_id: String = row.try_get("id")?;
        if let Some(s) = OrchestratorSession::load(&state.db.pool, &session_id).await.map_err(|e| AppError::Internal(e))? {
            sessions.push(s);
        }
    }
    Ok(sessions)
}

// ── 工作流交互命令 ────────────────────────────────────────

/// 暂停工作流
#[tauri::command]
pub async fn workflow_pause(
    state: State<'_, crate::AppState>,
    run_id: String,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE workflow_runs SET status = 'paused', finished_at = ? WHERE id = ? AND status = 'running'",
    )
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(&run_id)
    .execute(&state.db.pool)
    .await?;
    Ok(())
}

/// 恢复工作流
#[tauri::command]
pub async fn workflow_resume(
    state: State<'_, crate::AppState>,
    run_id: String,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE workflow_runs SET status = 'running', finished_at = NULL WHERE id = ? AND status = 'paused'",
    )
    .bind(&run_id)
    .execute(&state.db.pool)
    .await?;
    Ok(())
}

/// 获取活跃工作流列表
#[tauri::command]
pub async fn monitor_list_active_workflows(
    state: State<'_, crate::AppState>,
) -> Result<Vec<ActiveWorkflowDto>, AppError> {
    let rows = sqlx::query(
        "SELECT id, workflow_id, status, source, created_at, finished_at FROM workflow_runs WHERE status IN ('running', 'paused') ORDER BY created_at DESC"
    )
    .fetch_all(&state.db.pool)
    .await?;

    let mut results = Vec::new();
    for row in rows {
        let id: String = row.try_get("id")?;
        let workflow_id: String = row.try_get("workflow_id")?;
        let status: String = row.try_get("status")?;
        let source: String = row.try_get("source").unwrap_or_default();
        let created_at: i64 = row.try_get("created_at")?;
        let finished_at: Option<i64> = row.try_get("finished_at")?;

        results.push(ActiveWorkflowDto {
            id,
            workflow_id,
            status,
            source,
            created_at,
            finished_at,
        });
    }
    Ok(results)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActiveWorkflowDto {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub source: String,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

// ── 辅助 ──────────────────────────────────────────────────

/// 构建默认模型 Provider（§27.3 Planner/Reviewer 模型）
async fn build_default_provider(
    pool: &sqlx::SqlitePool,
) -> Result<Arc<dyn ModelProvider>, AppError> {
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

    Ok(Arc::new(OpenAiProvider::new(
        model_row.provider_id.clone(),
        model_row
            .display_name
            .clone()
            .unwrap_or_else(|| model_row.model_id.clone()),
        api_key,
        base_url,
        model_row.model_id.clone(),
    )))
}
