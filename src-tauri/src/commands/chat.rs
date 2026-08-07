use std::sync::Arc;
use tauri::{Emitter, State};
use tokio_util::sync::CancellationToken;

use crate::core::adk::model::GenerationRequest;
use crate::core::adk::tool::{ToolApprovalResponse, ToolRegistry};
use crate::core::rig::agent::RigAgent;
use crate::core::rig::provider::OpenAiProvider;
use crate::data::models::{MessageDto, ProviderRow};
use crate::data::services::ChatService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn chat_history(
    state: State<'_, crate::AppState>,
    session_id: String,
    limit: Option<i64>,
) -> Result<Vec<MessageDto>, AppError> {
    let svc = ChatService::new(state.db.pool.clone());
    svc.history(&session_id, limit).await
}

#[tauri::command]
pub async fn chat_send(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    session_id: String,
    content: String,
) -> Result<MessageDto, AppError> {
    let svc = ChatService::new(state.db.pool.clone());

    // 1. Save user message
    let user_msg = svc.save_message(&session_id, "user", &content, None, None, None, None).await?;

    // 2. Get session and agent config
    let session_row = sqlx::query_as::<_, crate::data::models::SessionRow>(
        "SELECT id, agent_id, title, pinned, created_at, updated_at FROM sessions WHERE id = ?"
    )
    .bind(&session_id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;

    let agent_row = sqlx::query_as::<_, crate::data::models::AgentRow>(
        "SELECT id, name, description, avatar, system_prompt, model_id, plan_model_id, small_model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at FROM agents WHERE id = ?"
    )
    .bind(&session_row.agent_id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::AgentNotFound(session_row.agent_id.clone()))?;

    // 3. Find model
    let model_row = if let Some(ref mid) = agent_row.model_id {
        sqlx::query_as::<_, crate::data::models::ModelRow>(
            "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE id = ?"
        )
        .bind(mid)
        .fetch_optional(&state.db.pool)
        .await?
    } else {
        sqlx::query_as::<_, crate::data::models::ModelRow>(
            "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
        )
        .fetch_optional(&state.db.pool)
        .await?
    };

    let model_row = match model_row {
        Some(m) => m,
        None => {
            let err_msg = "未配置模型。请在设置中添加 Provider 并设置默认模型。";
            let msg = svc.save_message(&session_id, "assistant", err_msg, None, None, None, None).await?;
            app.emit("chat:stream:done", serde_json::json!({
                "session_id": session_id,
                "message_id": msg.id,
                "usage": null,
            })).ok();
            return Ok(msg);
        }
    };

    // 4. Get provider
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

    let api_key = provider_row.api_key_enc.unwrap_or_default();

    // 5. Build history
    let history = svc.history(&session_id, Some(50)).await?;
    let mut messages = Vec::new();
    for msg in &history {
        use crate::core::adk::model::{ChatMessage, ChatRole, MessageContent};
        let role = match msg.role.as_str() {
            "user" => ChatRole::User,
            "assistant" => ChatRole::Assistant,
            "tool" => ChatRole::Tool,
            _ => ChatRole::User,
        };
        messages.push(ChatMessage {
            role,
            content: MessageContent::Text(msg.content.clone()),
            name: None,
        });
    }

    let system_prompt = agent_row.system_prompt.clone().unwrap_or_else(|| {
        "你是一个有用的 AI 助手。请用中文回答用户的问题。".to_string()
    });

    // 6. Create provider and run
    let provider = Arc::new(OpenAiProvider::new(
        model_row.provider_id.clone(),
        model_row.display_name.clone().unwrap_or_else(|| model_row.model_id.clone()),
        api_key,
        base_url,
        model_row.model_id.clone(),
    ));

    let agent = RigAgent::new(provider, system_prompt, ToolRegistry::new());
    let request = GenerationRequest { messages, ..Default::default() };

    let cancel = CancellationToken::new();
    state.active_cancels.lock().await.insert(session_id.clone(), cancel.clone());

    let session_id_clone = session_id.clone();
    let app_clone = app.clone();
    let pool = state.db.pool.clone();
    let model_id = model_row.model_id.clone();
    let _approval_store = state.approval_store.clone();
    let _agent_id = session_row.agent_id.clone();

    // Spawn task
    tokio::spawn(async move {
        let message_id = uuid::Uuid::new_v4().to_string();
        let _ = app_clone.emit("chat:stream:start", serde_json::json!({
            "session_id": session_id_clone,
            "message_id": message_id,
            "model": model_id,
        }));

        match agent.run(request).await {
            Ok(result) => {
                let now = chrono::Utc::now().timestamp_millis();
                let _ = sqlx::query(
                    "INSERT INTO messages (id, session_id, role, content, tool_calls, tool_call_id, model_id, usage, created_at) VALUES (?, ?, 'assistant', ?, NULL, NULL, ?, NULL, ?)"
                )
                .bind(&message_id)
                .bind(&session_id_clone)
                .bind(&result.text)
                .bind(&model_id)
                .bind(now)
                .execute(&pool)
                .await;

                let _ = sqlx::query("UPDATE sessions SET updated_at = ? WHERE id = ?")
                    .bind(now).bind(&session_id_clone).execute(&pool).await;

                let _ = app_clone.emit("chat:stream:done", serde_json::json!({
                    "session_id": session_id_clone,
                    "message_id": message_id,
                    "usage": null,
                }));
            }
            Err(e) => {
                let _ = app_clone.emit("chat:stream:error", serde_json::json!({
                    "session_id": session_id_clone,
                    "message_id": message_id,
                    "message": format!("AI 调用失败: {e}"),
                }));
            }
        }
    });

    Ok(user_msg)
}

#[tauri::command]
pub async fn chat_abort(
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<(), AppError> {
    if let Some(token) = state.active_cancels.lock().await.remove(&session_id) {
        token.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn tool_approval_respond(
    state: State<'_, crate::AppState>,
    call_id: String,
    response: ToolApprovalResponse,
) -> Result<bool, AppError> {
    // If always-approve was chosen, persist it
    if let ToolApprovalResponse::AlwaysApprove(ref tool_name) = response {
        state.approval_store.add_always_approve(tool_name).await;
    }
    Ok(state.approval_store.respond(&call_id, response).await)
}
