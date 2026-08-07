use std::sync::Arc;
use std::time::Duration;

use tauri::State;

use crate::core::adk::model::ModelProvider;
use crate::core::rig::judge::{aggregate_stats, AgentJudge, AgentStats, ComparisonResult, JudgeResult};
use crate::core::rig::provider::OpenAiProvider;
use crate::data::models::{ModelRow, ProviderRow};
use crate::data::services::trace_service::TraceService;
use crate::utils::error::AppError;

/// 解析 provider/model 构建评审模型（温度 0，供 LLM-as-Judge 使用）
async fn build_judge_model(state: &State<'_, crate::AppState>) -> Result<AgentJudge, AppError> {
    let model_row = sqlx::query_as::<_, ModelRow>(
        "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
    )
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider("未配置默认模型，无法执行评估".into()))?;

    let provider_row = sqlx::query_as::<_, ProviderRow>(
        "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
    )
    .bind(&model_row.provider_id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider(format!("Provider not found: {}", model_row.provider_id)))?;

    let base_url = provider_row.base_url.unwrap_or_else(|| {
        match provider_row.kind.as_str() {
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        }
    });
    let api_key = provider_row
        .api_key_enc
        .as_deref()
        .map(crate::commands::settings::decrypt_provider_key)
        .unwrap_or_default();

    let provider: Arc<dyn ModelProvider> = Arc::new(OpenAiProvider::new(
        model_row.provider_id.clone(),
        model_row.display_name.clone().unwrap_or_else(|| model_row.model_id.clone()),
        api_key,
        base_url,
        model_row.model_id.clone(),
    ));
    Ok(AgentJudge::new(provider))
}

/// §10.13.2 LLM-as-Judge：评估输出质量
#[tauri::command]
pub async fn agent_judge_evaluate(
    state: State<'_, crate::AppState>,
    task: String,
    output: String,
    criteria: Option<Vec<String>>,
) -> Result<JudgeResult, AppError> {
    let judge = build_judge_model(&state).await?;
    let criteria = criteria.unwrap_or_else(|| vec!["准确性".into(), "完整性".into(), "清晰度".into()]);
    let result = tokio::time::timeout(
        Duration::from_secs(120),
        judge.evaluate(&task, &output, &criteria),
    )
    .await
    .map_err(|_| AppError::LlmProvider("评估超时".into()))?
    .map_err(|e| AppError::LlmProvider(e.to_string()))?;
    Ok(result)
}

/// §10.13.2 对比两个版本输出
#[tauri::command]
pub async fn agent_judge_compare(
    state: State<'_, crate::AppState>,
    task: String,
    output_a: String,
    output_b: String,
    criteria: Option<Vec<String>>,
) -> Result<ComparisonResult, AppError> {
    let judge = build_judge_model(&state).await?;
    let criteria = criteria.unwrap_or_else(|| vec!["准确性".into(), "完整性".into()]);
    let result = tokio::time::timeout(
        Duration::from_secs(120),
        judge.compare(&task, &output_a, &output_b, &criteria),
    )
    .await
    .map_err(|_| AppError::LlmProvider("评估超时".into()))?
    .map_err(|e| AppError::LlmProvider(e.to_string()))?;
    Ok(result)
}

/// §10.13.3 性能仪表盘：按 session/agent 聚合轨迹统计
#[tauri::command]
pub async fn agent_stats(
    state: State<'_, crate::AppState>,
    session_id: Option<String>,
    limit: Option<i64>,
) -> Result<AgentStats, AppError> {
    let svc = TraceService::new(state.db.clone());
    let traces = if let Some(sid) = &session_id {
        svc.list_traces(sid, limit).await?
    } else {
        // 全量（最多 1000 条）
        let rows = sqlx::query("SELECT * FROM agent_traces ORDER BY started_at DESC LIMIT 1000")
            .fetch_all(&state.db.pool)
            .await?;
        let mut out = Vec::new();
        use sqlx::Row;
        for row in rows {
            let id: String = row.try_get("id")?;
            let session_id: String = row.try_get("session_id")?;
            let agent_id: String = row.try_get("agent_id")?;
            let trace_id: String = row.try_get("trace_id")?;
            let started_at: i64 = row.try_get("started_at")?;
            let finished_at: Option<i64> = row.try_get("finished_at")?;
            let steps_json: String = row.try_get("steps")?;
            let total_cost: f64 = row.try_get("total_cost")?;
            let outcome: String = row.try_get("outcome")?;
            let steps: Vec<crate::data::services::trace_service::TraceStep> = serde_json::from_str(&steps_json).unwrap_or_default();
            out.push(crate::data::services::trace_service::AgentTrace {
                id, session_id, agent_id, trace_id, started_at, finished_at, steps,
                total_prompt_tokens: 0, total_completion_tokens: 0, total_cost, outcome,
            });
        }
        out
    };
    Ok(aggregate_stats(&traces))
}
