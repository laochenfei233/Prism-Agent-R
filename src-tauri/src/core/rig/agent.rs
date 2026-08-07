use std::collections::HashSet;
use std::future::pending;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider, StreamEvent, ToolCall,
    ToolOutput, Usage,
};
use crate::core::adk::router::{RouteKind, RouteResult, RouteItem, ToolRouter};
use crate::core::adk::tool::{
    assess_risk, RiskLevel, ToolApprovalRequest, ToolApprovalResponse, ToolApprovalStore,
    ToolExecutor, ToolRegistry,
};
use crate::core::rig::compaction::{estimate_tokens, pressure_level, soft_trim};
use crate::core::rig::guardrails::{FilterResult, GuardrailPipeline};
use crate::core::rig::reflection::{run_reflection_loop, ReflectionConfig};
use crate::data::services::trace_service::{AgentTrace, TraceStep};
use crate::mcp::McpRuntime;

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
    /// Session identifier attached to approval requests & traces.
    pub session_id: Option<String>,
    /// When cancelled, the agent loop aborts promptly.
    pub cancel_token: Option<CancellationToken>,
    /// Invoked for every streamed text delta.
    pub on_delta: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Invoked for every streamed tool call.
    pub on_tool_call: Option<Arc<dyn Fn(&ToolCall) + Send + Sync>>,
    /// Optional MCP runtime; enables MCP tool fallback when a tool is not in the registry.
    pub mcp_runtime: Option<Arc<McpRuntime>>,
    /// L1 input guardrails (prompt injection / length limits).
    pub guardrails: Option<GuardrailPipeline>,
    /// Skill/MCP tool router: when present, only top-N relevant tool specs are injected.
    pub router: Option<ToolRouter>,
    /// Reflection loop config: when enabled, final outputs are critiqued & refined.
    pub reflection: Option<ReflectionConfig>,
    /// Token budget for context pressure checks / tool-output pruning.
    pub token_budget: Option<usize>,
    /// Invoked once with the completed execution trace (agent_traces persistence).
    pub on_trace: Option<Arc<dyn Fn(AgentTrace) + Send + Sync>>,
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
            session_id: None,
            cancel_token: None,
            on_delta: None,
            on_tool_call: None,
            mcp_runtime: None,
            guardrails: None,
            router: None,
            reflection: None,
            token_budget: None,
            on_trace: None,
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

    pub fn with_mcp_runtime(mut self, runtime: Arc<McpRuntime>) -> Self {
        self.mcp_runtime = Some(runtime);
        self
    }

    pub fn with_session_id(mut self, id: String) -> Self {
        self.session_id = Some(id);
        self
    }

    pub fn with_guardrails(mut self, guardrails: GuardrailPipeline) -> Self {
        self.guardrails = Some(guardrails);
        self
    }

    pub fn with_router(mut self, router: ToolRouter) -> Self {
        self.router = Some(router);
        self
    }

    pub fn with_reflection(mut self, config: ReflectionConfig) -> Self {
        self.reflection = Some(config);
        self
    }

    pub fn with_token_budget(mut self, budget: usize) -> Self {
        self.token_budget = Some(budget);
        self
    }

    pub fn with_on_trace(mut self, cb: impl Fn(AgentTrace) + Send + Sync + 'static) -> Self {
        self.on_trace = Some(Arc::new(cb));
        self
    }

    /// Execute agentic loop: generate → tool calls → fill results → regenerate
    pub async fn run(&self, request: GenerationRequest) -> Result<AgentRunResult, AgentError> {
        let started_at = chrono::Utc::now().timestamp();
        let prompt_len = estimate_prompt_len(&request);
        let mut current = request;
        let mut total_usage: Option<Usage> = None;
        let mut total_text_len: usize = 0;
        let mut steps: Vec<TraceStep> = Vec::new();
        let trace_id = uuid::Uuid::new_v4().to_string();

        // ── L1 输入护栏（入口一次性检查） ──
        if let Some(pipeline) = &self.guardrails {
            let input_text = last_user_text(&current);
            if let FilterResult::Block(reason) = pipeline.check_input(&input_text).await {
                self.emit_trace(AgentTrace {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: self.session_id.clone().unwrap_or_default(),
                    agent_id: self.agent_id.clone().unwrap_or_default(),
                    trace_id: trace_id.clone(),
                    started_at,
                    finished_at: Some(chrono::Utc::now().timestamp()),
                    steps: steps.clone(),
                    total_prompt_tokens: 0,
                    total_completion_tokens: 0,
                    total_cost: 0.0,
                    outcome: "blocked".into(),
                });
                return Err(AgentError::Guardrail(reason));
            }
        }

        for _ in 0..self.max_iterations {
            if self.is_cancelled() {
                return Err(AgentError::Internal("生成已中止".into()));
            }

            let iter_started = Instant::now();

            // Build full request with system prompt
            let mut req = current.clone();
            if !self.system_prompt.is_empty() {
                req.system = Some(self.system_prompt.clone());
            }

            // ── 工具路由注入（只暴露 top-N 相关工具） ──
            req.tools = self.routed_tool_specs(&current);

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

            let iter_latency = iter_started.elapsed().as_millis() as u64;

            // Record LLM step in trace
            let last_input = last_user_text(&current);
            steps.push(TraceStep {
                step_index: steps.len() as u32,
                kind: "llm_call".into(),
                input_summary: truncate(&last_input, 200),
                output_summary: truncate(&final_text, 200),
                latency_ms: iter_latency,
                tool_name: None,
                error: None,
            });

            // No tool calls = done
            if tool_calls.is_empty() {
                // ── 反思循环（最终输出评审改进） ──
                let text = if let Some(config) = &self.reflection {
                    if config.enabled {
                        let original_task = last_user_text(&current);
                        match run_reflection_loop(
                            self.model_provider.clone(),
                            &self.system_prompt,
                            &original_task,
                            &final_text,
                            config,
                        )
                        .await
                        {
                            Ok(r) => {
                                for (i, hist) in r.history.iter().enumerate() {
                                    steps.push(TraceStep {
                                        step_index: steps.len() as u32,
                                        kind: "reflection".into(),
                                        input_summary: format!("reflection iter {i}"),
                                        output_summary: truncate(hist, 200),
                                        latency_ms: 0,
                                        tool_name: None,
                                        error: None,
                                    });
                                }
                                r.text
                            }
                            Err(_) => final_text.clone(),
                        }
                    } else {
                        final_text.clone()
                    }
                } else {
                    final_text.clone()
                };

                let usage = total_usage
                    .clone()
                    .or_else(|| Some(estimate_usage(prompt_len, total_text_len)));
                let (p, c) = usage
                    .as_ref()
                    .map(|u| (u.prompt_tokens, u.completion_tokens))
                    .unwrap_or((0, 0));

                self.emit_trace(AgentTrace {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: self.session_id.clone().unwrap_or_default(),
                    agent_id: self.agent_id.clone().unwrap_or_default(),
                    trace_id: trace_id.clone(),
                    started_at,
                    finished_at: Some(chrono::Utc::now().timestamp()),
                    steps: steps.clone(),
                    total_prompt_tokens: p as i64,
                    total_completion_tokens: c as i64,
                    total_cost: 0.0,
                    outcome: "success".into(),
                });

                return Ok(AgentRunResult {
                    text,
                    tool_calls: Vec::new(),
                    usage,
                });
            }

            // Execute tools and fill results
            let mut tool_results = Vec::new();
            for call in &tool_calls {
                let started = Instant::now();
                let output = self.execute_tool(call).await;
                let latency = started.elapsed().as_millis() as u64;
                steps.push(TraceStep {
                    step_index: steps.len() as u32,
                    kind: "tool_call".into(),
                    input_summary: format!("{} {}", call.name, call.arguments),
                    output_summary: truncate(&output.content, 200),
                    latency_ms: latency,
                    tool_name: Some(call.name.clone()),
                    error: output.is_error.then(|| truncate(&output.content, 200)),
                });
                // ── 上下文压力裁剪（工具输出过长时软裁剪） ──
                let content = if let Some(budget) = self.token_budget {
                    let used = estimate_tokens(&output.content);
                    if pressure_level(used, budget) >= 1 {
                        soft_trim(&output.content)
                    } else {
                        output.content.clone()
                    }
                } else {
                    output.content.clone()
                };
                tool_results.push(ChatMessage {
                    role: ChatRole::Tool,
                    content: MessageContent::ToolResult(ToolOutput::text(content)),
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

        self.emit_trace(AgentTrace {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: self.session_id.clone().unwrap_or_default(),
            agent_id: self.agent_id.clone().unwrap_or_default(),
            trace_id: trace_id.clone(),
            started_at,
            finished_at: Some(chrono::Utc::now().timestamp()),
            steps: steps.clone(),
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_cost: 0.0,
            outcome: "timeout".into(),
        });

        Err(AgentError::MaxIterations)
    }

    /// Router-filtered tool specs: top-N by BM25 over the latest user message.
    /// Falls back to all specs when no router is configured or nothing matched.
    fn routed_tool_specs(&self, request: &GenerationRequest) -> Vec<crate::core::adk::model::ToolSpec> {
        let Some(router) = &self.router else {
            return self.tools.specs();
        };
        let query = last_user_text(request);
        let top_k = 8usize;
        let RouteResult { tools, .. } = router.route(&query, 3, top_k);
        let names: HashSet<String> = tools.iter().map(|t| t.id.clone()).collect();
        let filtered = self.tools.specs_filtered(&names);
        if filtered.is_empty() {
            self.tools.specs()
        } else {
            filtered
        }
    }

    /// Build a router index from the current tool registry (call after registering tools).
    pub fn build_router(&self, _top_k_tools: usize) -> ToolRouter {
        let mut router = ToolRouter::new();
        let items: Vec<RouteItem> = self
            .tools
            .tool_names()
            .into_iter()
            .map(|name| RouteItem {
                id: name.clone(),
                kind: RouteKind::McpTool,
                name: name.clone(),
                description: self
                    .tools
                    .get(&name)
                    .map(|t| t.description().to_string())
                    .unwrap_or_default(),
                keywords: keywordize(&name),
                server_id: None,
            })
            .collect();
        router.refresh(items);
        router
    }

    fn emit_trace(&self, trace: AgentTrace) {
        if let Some(cb) = &self.on_trace {
            cb(trace);
        }
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
            None => match &self.mcp_runtime {
                Some(rt) => match rt.find_tool_server(&call.name).await {
                    Some(server_id) => match rt.call_tool(&server_id, &call.name, call.arguments.clone()).await {
                        Ok(result) => ToolOutput::text(serde_json::to_string(&result).unwrap_or_default()),
                        Err(e) => ToolOutput::error(format!("MCP tool error: {e}")),
                    },
                    None => ToolOutput::error(format!("Unknown tool: {}", call.name)),
                },
                None => ToolOutput::error(format!("Unknown tool: {}", call.name)),
            },
        }
    }
}

/// Tool executor that routes execution to a registered MCP server.
pub struct McpToolExecutor {
    server_id: String,
    tool_name: String,
    description: String,
    input_schema: serde_json::Value,
    runtime: Arc<McpRuntime>,
}

impl McpToolExecutor {
    pub fn new(
        server_id: String,
        tool_name: String,
        description: String,
        input_schema: serde_json::Value,
        runtime: Arc<McpRuntime>,
    ) -> Self {
        Self { server_id, tool_name, description, input_schema, runtime }
    }
}

#[async_trait]
impl ToolExecutor for McpToolExecutor {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn schema(&self) -> serde_json::Value {
        self.input_schema.clone()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let result = self
            .runtime
            .call_tool(&self.server_id, &self.tool_name, args)
            .await
            .map_err(|e| AgentError::Tool(format!("MCP tool error: {e}")))?;
        Ok(ToolOutput::text(serde_json::to_string(&result).unwrap_or_default()))
    }
}

/// Await cancellation, or never resolve when no token is configured.
async fn wait_cancel(cancel: &Option<CancellationToken>) {
    match cancel {
        Some(token) => token.cancelled().await,
        None => pending::<()>().await,
    }
}

/// Extract the latest user text from a request (used for guardrails & routing).
fn last_user_text(request: &GenerationRequest) -> String {
    request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == ChatRole::User)
        .map(|m| match &m.content {
            MessageContent::Text(t) => t.clone(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

/// Truncate a string to a maximum number of characters.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}

/// Tokenize a tool name into searchable keywords (snake_case / camelCase split).
fn keywordize(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in name.chars() {
        if c == '_' || c == '-' || c == ':' {
            if !current.is_empty() {
                out.push(current.clone());
                current.clear();
            }
        } else if c.is_uppercase() && !current.is_empty() {
            out.push(current.clone());
            current.clear();
            current.push(c.to_ascii_lowercase());
        } else {
            current.push(c.to_ascii_lowercase());
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
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
