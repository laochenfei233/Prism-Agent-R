use std::sync::Arc;
use tauri::{Emitter, State};
use tokio_util::sync::CancellationToken;

use crate::core::adk::model::GenerationRequest;
use crate::core::adk::tool::{ToolApprovalResponse, ToolRegistry};
use crate::core::rig::agent::{McpToolExecutor, RigAgent};
use crate::core::rig::guardrails::GuardrailPipeline;
use crate::core::rig::provider::OpenAiProvider;
use crate::data::models::{MessageDto, ProviderRow};
use crate::data::services::trace_service::{AgentTrace, TraceService};
use crate::commands::file::read_attachment_text;
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
    attachments: Option<Vec<String>>, // 附件文件路径列表；缺省时为 None
) -> Result<MessageDto, AppError> {
    let svc = ChatService::new(state.db.pool.clone());

    // 0. 附件文本拼接到用户消息前
    let content = with_attachments(content, attachments).await;

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
    //    Try by agent.model_id (UUID → models.id), then fallback to model_id string (→ models.model_id),
    //    then fallback to default model.
    let model_row = if let Some(ref mid) = agent_row.model_id {
        // First: try exact match on models.id (UUID)
        let found = sqlx::query_as::<_, crate::data::models::ModelRow>(
            "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE id = ?"
        )
        .bind(mid)
        .fetch_optional(&state.db.pool)
        .await?;
        if found.is_some() {
            found
        } else {
            // Fallback: agent.model_id might be the model_id string (e.g. "gpt-4o") rather than UUID
            sqlx::query_as::<_, crate::data::models::ModelRow>(
                "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE model_id = ? LIMIT 1"
            )
            .bind(mid)
            .fetch_optional(&state.db.pool)
            .await?
        }
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

    let api_key = provider_row
        .api_key_enc
        .as_deref()
        .map(crate::commands::settings::decrypt_provider_key)
        .unwrap_or_default();

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

    // 6. Create provider and agent
    let provider = Arc::new(OpenAiProvider::new(
        model_row.provider_id.clone(),
        model_row.display_name.clone().unwrap_or_else(|| model_row.model_id.clone()),
        api_key,
        base_url,
        model_row.model_id.clone(),
    ));

    let message_id = uuid::Uuid::new_v4().to_string();
    let cancel = CancellationToken::new();
    state.active_cancels.lock().await.insert(session_id.clone(), cancel.clone());

    // Stream event forwarding callbacks
    let delta_app = app.clone();
    let delta_sid = session_id.clone();
    let delta_mid = message_id.clone();
    let on_delta = move |delta: &str| {
        let _ = delta_app.emit("chat:stream:delta", serde_json::json!({
            "session_id": delta_sid,
            "message_id": delta_mid,
            "delta": delta,
        }));
    };

    let call_app = app.clone();
    let call_sid = session_id.clone();
    let call_mid = message_id.clone();
    let on_tool_call = move |call: &crate::core::adk::model::ToolCall| {
        let _ = call_app.emit("chat:stream:tool_call", serde_json::json!({
            "session_id": call_sid,
            "message_id": call_mid,
            "call": {
                "id": call.id,
                "name": call.name,
                "arguments": call.arguments,
            },
        }));
    };

    // Register MCP tools bound to this agent
    let mut registry = ToolRegistry::new();
    let mcp_links: Vec<(String,)> = sqlx::query_as(
        "SELECT mcp_server_id FROM agent_mcp_servers WHERE agent_id = ?"
    )
    .bind(&session_row.agent_id)
    .fetch_all(&state.db.pool)
    .await?;
    for (server_id,) in mcp_links {
        for tool in state.mcp_runtime.get_tools(&server_id).await {
            registry.register(Box::new(McpToolExecutor::new(
                server_id.clone(),
                tool.name.clone(),
                tool.description.clone(),
                tool.input_schema.clone(),
                state.mcp_runtime.clone(),
            )));
        }
    }

    // §15 注册内置 web_search 工具
    {
        let search_config = crate::commands::search::get_search_config(&state.db.pool).await;
        let search_service = std::sync::Arc::new(
            crate::core::search::service::SearchService::from_config(&search_config)
        );
        registry.register(Box::new(
            crate::core::search::web_search::WebSearchTool::new(search_service)
        ));
    }

    // §10.1.1 对话内 wiki_write 工具（Agent 可将新知识写入知识库）
    registry.register(Box::new(
        crate::core::adk::wiki_tool::WikiWriteTool::new(state.db.clone())
    ));

    // ── 构建 Agent 运行时（护栏 + 路由 + 反思 + 轨迹） ──
    let mut agent = RigAgent::new(provider, system_prompt, registry)
        .with_approval_store(state.approval_store.clone())
        .with_app_handle(app.clone())
        .with_agent_id(session_row.agent_id.clone())
        .with_session_id(session_id.clone())
        .with_cancel_token(cancel.clone())
        .with_on_delta(on_delta)
        .with_on_tool_call(on_tool_call)
        .with_mcp_runtime(state.mcp_runtime.clone());

    // 护栏：默认启用注入检测 + 长度限制（阈值与开关可从设置页调整）
    {
        use crate::data::settings::prefs;
        let max_chars = prefs::get_i64(&state.db.pool, "guardrail.max_chars", 100_000).await as usize;
        let injection = prefs::get_bool(&state.db.pool, "guardrail.injection_enabled", true).await;
        agent = agent.with_guardrails(GuardrailPipeline::configured(max_chars, injection));
    }

    // 工具路由：按用户消息 BM25 注入 top-N 工具
    let router = agent.build_router(8);
    agent = agent.with_router(router);

    // Token 预算：工具输出裁剪阈值（默认约 100K tokens，可从设置页调整）
    {
        use crate::data::settings::prefs;
        let budget = prefs::get_i64(&state.db.pool, "token_budget.chat", 100_000).await as usize;
        agent = agent.with_token_budget(budget);
    }

    // 反思循环：设置页开关启用后接线（默认关闭）
    {
        use crate::data::settings::prefs;
        let reflection_enabled =
            prefs::get_bool(&state.db.pool, "reflection.enabled", false).await;
        if reflection_enabled {
            let max_iters =
                prefs::get_i64(&state.db.pool, "reflection.max_iterations", 3).await.clamp(1, 10) as u32;
            agent = agent.with_reflection(
                crate::core::rig::reflection::ReflectionConfig {
                    enabled: true,
                    max_iterations: max_iters,
                    ..Default::default()
                },
            );
        }
    }

    // 轨迹记录：完成后写入 agent_traces
    {
        let db = state.db.clone();
        agent = agent.with_on_trace(move |trace: AgentTrace| {
            let db = db.clone();
            tokio::spawn(async move {
                let svc = TraceService::new(db);
                if let Err(e) = svc.record_trace(&trace).await {
                    tracing::warn!("trace record failed: {e}");
                }
            });
        });
    }

    let request = GenerationRequest { messages, ..Default::default() };

    let session_id_clone = session_id.clone();
    let app_clone = app.clone();
    let pool = state.db.pool.clone();
    let model_id = model_row.model_id.clone();

    // Spawn task
    tokio::spawn(async move {
        let _ = app_clone.emit("chat:stream:start", serde_json::json!({
            "session_id": session_id_clone.clone(),
            "message_id": message_id.clone(),
            "model": model_id.clone(),
        }));

        match agent.run(request).await {
            Ok(result) => {
                // Aborted: do not persist a partial message.
                if cancel.is_cancelled() {
                    let _ = app_clone.emit("chat:stream:error", serde_json::json!({
                        "session_id": session_id_clone,
                        "message_id": message_id,
                        "message": "生成已中止",
                    }));
                    return;
                }

                let now = chrono::Utc::now().timestamp_millis();
                let usage_str = result.usage.as_ref().map(|u| serde_json::json!({
                    "prompt_tokens": u.prompt_tokens,
                    "completion_tokens": u.completion_tokens,
                    "total_tokens": u.total_tokens,
                    "cost": 0,
                }).to_string());

                let _ = sqlx::query(
                    "INSERT INTO messages (id, session_id, role, content, tool_calls, tool_call_id, model_id, usage, created_at) VALUES (?, ?, 'assistant', ?, NULL, NULL, ?, ?, ?)"
                )
                .bind(&message_id)
                .bind(&session_id_clone)
                .bind(&result.text)
                .bind(&model_id)
                .bind(usage_str.as_deref())
                .bind(now)
                .execute(&pool)
                .await;

                let _ = sqlx::query("UPDATE sessions SET updated_at = ? WHERE id = ?")
                    .bind(now).bind(&session_id_clone).execute(&pool).await;

                // Cumulative token usage for the session
                let cum: (i64, i64, i64) = sqlx::query_as(
                    "SELECT COALESCE(SUM(json_extract(usage, '$.prompt_tokens')), 0), COALESCE(SUM(json_extract(usage, '$.completion_tokens')), 0), COALESCE(SUM(json_extract(usage, '$.total_tokens')), 0) FROM messages WHERE session_id = ? AND usage IS NOT NULL"
                )
                .bind(&session_id_clone)
                .fetch_one(&pool)
                .await
                .unwrap_or((0, 0, 0));

                let _ = app_clone.emit("usage:updated", serde_json::json!({
                    "session_id": session_id_clone.clone(),
                    "prompt_tokens": cum.0,
                    "completion_tokens": cum.1,
                    "total_tokens": cum.2,
                }));

                let _ = app_clone.emit("chat:stream:done", serde_json::json!({
                    "session_id": session_id_clone,
                    "message_id": message_id,
                    "usage": result.usage,
                    "message": result.text,
                }));
            }
            Err(e) => {
                let message = if cancel.is_cancelled() {
                    "生成已中止".to_string()
                } else {
                    format!("AI 调用失败: {e}")
                };
                let _ = app_clone.emit("chat:stream:error", serde_json::json!({
                    "session_id": session_id_clone,
                    "message_id": message_id,
                    "message": message,
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
    response: String,
) -> Result<bool, AppError> {
    // The UI sends plain strings; map them onto the response enum.
    let parsed = match response.as_str() {
        "Approved" => ToolApprovalResponse::Approved,
        "AlwaysApprove" => ToolApprovalResponse::AlwaysApprove(String::new()),
        "Defer" => ToolApprovalResponse::Defer,
        other => ToolApprovalResponse::Rejected(other.to_string()),
    };

    // If always-approve was chosen, persist it
    if let ToolApprovalResponse::AlwaysApprove(tool_name) = &parsed {
        if !tool_name.is_empty() {
            state.approval_store.add_always_approve(tool_name).await;
        }
    }
    Ok(state.approval_store.respond(&call_id, parsed).await)
}

/// 将附件文本拼接到用户消息前：[附件: {path}\n{内容}]\n\n{content}
async fn with_attachments(content: String, attachments: Option<Vec<String>>) -> String {
    let Some(paths) = attachments else {
        return content;
    };
    if paths.is_empty() {
        return content;
    }

    let mut out = String::new();
    for path in paths {
        let text = read_attachment_text(&path).await;
        out.push_str(&format!("[附件: {path}\n{text}]\n"));
    }
    out.push('\n');
    out.push_str(&content);
    out
}
