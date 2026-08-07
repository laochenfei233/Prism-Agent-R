use std::future::pending;
use std::sync::Arc;
use std::time::Duration;

use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider, StreamEvent, ToolCall,
    ToolOutput, Usage,
};
use crate::core::adk::tool::{
    assess_risk, RiskLevel, ToolApprovalRequest, ToolApprovalResponse, ToolApprovalStore,
    ToolRegistry,
};

// ── Rig Agent ─────────────────────────────────────────────

pub struct RigAgent {
    pub model_provider: Arc<dyn ModelProvider>,
    pub system_prompt: String,
    pub tools: ToolRegistry,
    pub max_iterations: u32,
    /// HITL approval store; when present, High/Critical tools require approval.
    pub approval_store: Option<Arc<ToolApprovalStore>>,
    /// Used to emit `tool:approval-request` events to the UI.
    pub app_handle: Option<tauri::AppHandle>,
    /// Agent identifier attached to approval requests.
    pub agent_id: Option<String>,
    /// When cancelled, the agent loop aborts promptly.
    pub cancel_token: Option<CancellationToken>,
    /// Invoked for every streamed text delta.
    pub on_delta: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Invoked for every streamed tool call.
    pub on_tool_call: Option<Arc<dyn Fn(&ToolCall) + Send + Sync>>,
}

pub struct AgentRunResult {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    /// Aggregated usage across all model calls; estimated from streamed
    /// characters when the provider does not report usage.
    pub usage: Option<Usage>,
}

impl RigAgent {
    pub fn new(
        model_provider: Arc<dyn ModelProvider>,
        system_prompt: String,
        tools: ToolRegistry,
    ) -> Self {
        Self {
            model_provider,
            system_prompt,
            tools,
            max_iterations: 20,
            approval_store: None,
            app_handle: None,
            agent_id: None,
            cancel_token: None,
            on_delta: None,
            on_tool_call: None,
        }
    }

    pub fn with_approval_store(mut self, store: Arc<ToolApprovalStore>) -> Self {
        self.approval_store = Some(store);
        self
    }

    pub fn with_app_handle(mut self, app: tauri::AppHandle) -> Self {
        self.app_handle = Some(app);
        self
    }

    pub fn with_agent_id(mut self, id: String) -> Self {
        self.agent_id = Some(id);
        self
    }

    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    pub fn with_on_delta(mut self, cb: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.on_delta = Some(Arc::new(cb));
        self
    }

    pub fn with_on_tool_call(mut self, cb: impl Fn(&ToolCall) + Send + Sync + 'static) -> Self {
        self.on_tool_call = Some(Arc::new(cb));
        self
    }

    /// Execute agentic loop: generate → tool calls → fill results → regenerate
    pub async fn run(&self, request: GenerationRequest) -> Result<AgentRunResult, AgentError> {
        let prompt_len = estimate_prompt_len(&request);
        let mut current = request;
        let mut total_usage: Option<Usage> = None;
        let mut total_text_len: usize = 0;

        for _ in 0..self.max_iterations {
            if self.is_cancelled() {
                return Err(AgentError::Internal("生成已中止".into()));
            }

            // Build full request with system prompt
            let mut req = current.clone();
            if !self.system_prompt.is_empty() {
                req.system = Some(self.system_prompt.clone());
            }

            // Add tool specs
            req.tools = self.tools.specs();

            // Generate
            let mut handle = self.model_provider.stream(req).await?;
            let mut tool_calls = Vec::new();
            let mut final_text = String::new();

            // Consume stream, forwarding deltas/tool calls and checking cancel
            use futures::StreamExt;
            loop {
                let next = handle.next();
                tokio::pin!(next);
                tokio::select! {
                    _ = wait_cancel(&self.cancel_token) => {
                        return Err(AgentError::Internal("生成已中止".into()));
                    }
                    event = &mut next => {
                        match event {
                            Some(StreamEvent::Text(t)) => {
                                total_text_len += t.len();
                                final_text.push_str(&t);
                                if let Some(cb) = &self.on_delta {
                                    cb(&t);
                                }
                            }
                            Some(StreamEvent::ToolCall(call)) => {
                                tool_calls.push(call.clone());
                                if let Some(cb) = &self.on_tool_call {
                                    cb(&call);
                                }
                            }
                            Some(StreamEvent::Finish { usage }) => {
                                total_usage = merge_usage(total_usage, usage);
                                break;
                            }
                            Some(StreamEvent::Error(e)) => return Err(AgentError::Stream(e)),
                            None => break,
                        }
                    }
                }
            }

            // No tool calls = done
            if tool_calls.is_empty() {
                return Ok(AgentRunResult {
                    text: final_text,
                    tool_calls: Vec::new(),
                    usage: total_usage.or_else(|| Some(estimate_usage(prompt_len, total_text_len))),
                });
            }

            // Execute tools and fill results
            let mut tool_results = Vec::new();
            for call in &tool_calls {
                let output = self.execute_tool(call).await;
                tool_results.push(ChatMessage {
                    role: ChatRole::Tool,
                    content: MessageContent::ToolResult(output),
                    name: Some(call.name.clone()),
                });
            }

            // Add assistant message and tool results to conversation
            current.messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: MessageContent::Text(final_text),
                name: None,
            });
            current.messages.extend(tool_results);
        }

        Err(AgentError::MaxIterations)
    }

    fn is_cancelled(&self) -> bool {
        self.cancel_token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false)
    }

    /// Run one tool call, gated by HITL approval for High/Critical risk.
    async fn execute_tool(&self, call: &ToolCall) -> ToolOutput {
        let risk = assess_risk(&call.name, &call.arguments);

        if matches!(&risk, RiskLevel::High | RiskLevel::Critical) {
            // Already always-approved: skip the gate.
            if let Some(store) = &self.approval_store {
                if store.is_always_approved(&call.name).await {
                    return self.run_tool(call).await;
                }
            }

            let request = ToolApprovalRequest {
                call_id: call.id.clone(),
                tool_name: call.name.clone(),
                arguments: call.arguments.clone(),
                agent_id: self.agent_id.clone().unwrap_or_default(),
                risk_level: risk,
                description: format!("工具「{}」请求执行（风险等级: High/Critical）", call.name),
            };
            if let Some(app) = &self.app_handle {
                let _ = app.emit("tool:approval-request", &request);
            }

            // Wait for a response, or treat timeout / missing store as Defer.
            let response = match &self.approval_store {
                Some(store) => {
                    let rx = store.request_approval(call.id.clone()).await;
                    match tokio::time::timeout(Duration::from_secs(30), rx).await {
                        Ok(Ok(resp)) => resp,
                        _ => ToolApprovalResponse::Defer,
                    }
                }
                None => ToolApprovalResponse::Defer,
            };

            match response {
                ToolApprovalResponse::Approved => self.run_tool(call).await,
                ToolApprovalResponse::AlwaysApprove(_) => {
                    if let Some(store) = &self.approval_store {
                        store.add_always_approve(&call.name).await;
                    }
                    self.run_tool(call).await
                }
                ToolApprovalResponse::Rejected(reason) => ToolOutput::error(format!(
                    "工具「{}」被用户拒绝: {}",
                    call.name, reason
                )),
                ToolApprovalResponse::Defer => ToolOutput::error(format!(
                    "工具「{}」审批超时或已搁置，未执行",
                    call.name
                )),
            }
        } else {
            self.run_tool(call).await
        }
    }

    async fn run_tool(&self, call: &ToolCall) -> ToolOutput {
        match self.tools.get(&call.name) {
            Some(tool) => match tool.execute(call.arguments.clone()).await {
                Ok(output) => output,
                Err(e) => ToolOutput::error(format!("Tool error: {e}")),
            },
            None => ToolOutput::error(format!("Unknown tool: {}", call.name)),
        }
    }
}

/// Await cancellation, or never resolve when no token is configured.
async fn wait_cancel(cancel: &Option<CancellationToken>) {
    match cancel {
        Some(token) => token.cancelled().await,
        None => pending::<()>().await,
    }
}

fn merge_usage(acc: Option<Usage>, add: Option<Usage>) -> Option<Usage> {
    match (acc, add) {
        (Some(a), Some(b)) => Some(Usage {
            prompt_tokens: a.prompt_tokens + b.prompt_tokens,
            completion_tokens: a.completion_tokens + b.completion_tokens,
            total_tokens: a.total_tokens + b.total_tokens,
        }),
        (a, b) => a.or(b),
    }
}

fn estimate_prompt_len(request: &GenerationRequest) -> usize {
    let messages_len = serde_json::to_string(&request.messages).unwrap_or_default().len();
    let system_len = request.system.as_ref().map(|s| s.len()).unwrap_or(0);
    messages_len + system_len
}

/// Rough char-based token estimate (~4 chars/token) when the provider
/// reports no usage in the stream.
fn estimate_usage(prompt_chars: usize, completion_chars: usize) -> Usage {
    let prompt_tokens = (prompt_chars as u64 + 3) / 4;
    let completion_tokens = (completion_chars as u64 + 3) / 4;
    Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    }
}
