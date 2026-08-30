use std::collections::HashSet;
use std::future::pending;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::core::adk::error::AgentError;
use crate::core::adk::memory::{MemoryStore, MessageExchange};
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider, StreamEvent, ToolCall,
    ToolOutput, Usage,
};
use crate::core::adk::router::{RouteItem, RouteKind, RouteResult, ToolRouter};
use crate::core::adk::tool::{
    assess_risk, RiskLevel, ToolApprovalRequest, ToolApprovalResponse, ToolApprovalStore,
    ToolExecutor, ToolRegistry,
};
use crate::core::rig::compaction::Compactor;
use crate::core::rig::compaction::{estimate_tokens, pressure_level, soft_trim};
use crate::core::rig::guardrails::{FilterResult, GuardrailPipeline};
use crate::core::rig::reflection::{run_reflection_loop, ReflectionConfig};
use crate::data::services::trace_service::{AgentTrace, TraceStep};
use crate::mcp::McpRuntime;

// ── Rig Agent ─────────────────────────────────────────────

/// Streamed-delta callback shared by text/reasoning streams.
pub type DeltaCallback = Arc<dyn Fn(&str) + Send + Sync>;
/// Streamed tool-call callback.
pub type ToolCallCallback = Arc<dyn Fn(&ToolCall) + Send + Sync>;
/// Tool execution-result callback (call_id, tool_name, output).
pub type ToolResultCallback = Arc<dyn Fn(&str, &str, &ToolOutput) + Send + Sync>;

/// 已启用技能的路由条目 + 内容（用于 build_router 索引与按轮注入）
#[derive(Debug, Clone)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub keywords: Vec<String>,
}

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
    pub on_delta: Option<DeltaCallback>,
    /// Invoked for every streamed reasoning/thinking delta.
    pub on_reasoning: Option<DeltaCallback>,
    /// Invoked for every streamed tool call.
    pub on_tool_call: Option<ToolCallCallback>,
    /// Invoked after each tool execution (call_id, tool name, output).
    pub on_tool_result: Option<ToolResultCallback>,
    /// 记忆系统：运行前注入上下文，完成后自动记录。
    pub memory: Option<Arc<dyn MemoryStore>>,
    /// 长会话摘要压缩器（超过阈值时压缩历史）。
    pub compaction: Option<Compactor>,
    /// 已启用技能（用于路由索引 + 按轮动态注入）。
    pub skills: Vec<AgentSkill>,
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
            on_reasoning: None,
            on_tool_call: None,
            on_tool_result: None,
            memory: None,
            compaction: None,
            skills: Vec::new(),
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

    pub fn with_on_reasoning(mut self, cb: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.on_reasoning = Some(Arc::new(cb));
        self
    }

    pub fn with_on_tool_call(mut self, cb: impl Fn(&ToolCall) + Send + Sync + 'static) -> Self {
        self.on_tool_call = Some(Arc::new(cb));
        self
    }

    pub fn with_on_tool_result(
        mut self,
        cb: impl Fn(&str, &str, &ToolOutput) + Send + Sync + 'static,
    ) -> Self {
        self.on_tool_result = Some(Arc::new(cb));
        self
    }

    pub fn with_memory(mut self, memory: Arc<dyn MemoryStore>) -> Self {
        self.memory = Some(memory);
        self
    }

    pub fn with_compaction(mut self, compactor: Compactor) -> Self {
        self.compaction = Some(compactor);
        self
    }

    pub fn with_skills(mut self, skills: Vec<AgentSkill>) -> Self {
        self.skills = skills;
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
                    grade_score: None,
                    grade_reason: None,
                    graded_at: None,
                });
                return Err(AgentError::Guardrail(reason));
            }
        }

        // ── 记忆上下文注入（全局/项目记忆 → 追加到 system prompt） ──
        let mut system_prompt = self.system_prompt.clone();
        if let Some(memory) = &self.memory {
            let session_id = self.session_id.clone().unwrap_or_default();
            let agent_id = self.agent_id.clone().unwrap_or_default();
            match memory.build_context(&session_id, &agent_id).await {
                Ok(ctx) => {
                    if !ctx.summary.trim().is_empty() {
                        system_prompt.push_str(&format!("\n\n---\n# 记忆上下文\n{}", ctx.summary));
                    }
                    for item in &ctx.items {
                        system_prompt
                            .push_str(&format!("\n\n[记忆: {}]\n{}", item.path, item.body));
                    }
                }
                Err(e) => tracing::warn!("memory context build failed: {e}"),
            }
        }

        for _ in 0..self.max_iterations {
            if self.is_cancelled() {
                return Err(AgentError::Internal("生成已中止".into()));
            }

            let iter_started = Instant::now();

            // ── 长会话压缩（超过触发阈值时先用摘要替换历史） ──
            if let Some(compactor) = &self.compaction {
                let history_json = serde_json::to_string(&current.messages).unwrap_or_default();
                let current_tokens = estimate_tokens(&history_json) + system_prompt.len() / 4;
                if compactor.needs_compaction(current_tokens) && current.messages.len() > 6 {
                    match compactor
                        .compact(self.model_provider.as_ref(), &current.messages)
                        .await
                    {
                        Ok(compacted) => {
                            tracing::info!(
                                "session {}: compacted {} messages -> {}",
                                self.session_id.as_deref().unwrap_or("-"),
                                current.messages.len(),
                                compacted.len()
                            );
                            current.messages = compacted;
                        }
                        Err(e) => tracing::warn!("compaction failed: {e}"),
                    }
                }
            }

            // Build full request with system prompt
            let mut req = current.clone();
            if !system_prompt.is_empty() {
                req.system = Some(system_prompt.clone());
            }

            // ── 工具路由注入（只暴露 top-N 相关工具 + 命中技能动态注入） ──
            let (tool_specs, skill_section) =
                self.routed_tool_specs(&current, req.system.as_deref().unwrap_or(""));
            req.tools = tool_specs;
            if !skill_section.is_empty() {
                let base = req.system.clone().unwrap_or_default();
                req.system = Some(format!("{base}\n{skill_section}"));
            }

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
                            Some(StreamEvent::Reasoning(t)) => {
                                if let Some(cb) = &self.on_reasoning {
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

                // ── 记忆记录（完成后将本轮 user/assistant 交换追加到会话记忆） ──
                if let (Some(memory), Some(session_id)) = (&self.memory, &self.session_id) {
                    let exchange = MessageExchange {
                        user_message: last_user_text(&current),
                        assistant_message: text.clone(),
                    };
                    let agent_id = self.agent_id.clone().unwrap_or_default();
                    let sid = session_id.clone();
                    let mem = memory.clone();
                    tokio::spawn(async move {
                        if let Err(e) = mem.record(&sid, &agent_id, exchange).await {
                            tracing::warn!("memory record failed: {e}");
                        }
                    });
                }

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
                    grade_score: None,
                    grade_reason: None,
                    graded_at: None,
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
                // ── 工具结果事件（供前端 ToolCallCard 展示） ──
                if let Some(cb) = &self.on_tool_result {
                    cb(&call.id, &call.name, &output);
                }
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
            grade_score: None,
            grade_reason: None,
            graded_at: None,
        });

        Err(AgentError::MaxIterations)
    }

    /// Router-filtered tool specs + 命中技能内容注入。
    ///
    /// 返回 (工具 specs, 本轮需追加到 system prompt 的技能内容)。
    /// 无 router 时返回全部工具；未匹配任何工具时回退全部工具。
    /// 已在基础 system prompt 中注入过的技能（含 `<!-- skill:<id> -->` 标记）跳过，避免重复。
    fn routed_tool_specs(
        &self,
        request: &GenerationRequest,
        system_prompt: &str,
    ) -> (Vec<crate::core::adk::model::ToolSpec>, String) {
        let Some(router) = &self.router else {
            return (self.tools.specs(), String::new());
        };
        let query = last_user_text(request);
        let top_k = 8usize;
        let RouteResult { skills, tools, .. } = router.route(&query, 3, top_k);
        let names: HashSet<String> = tools.iter().map(|t| t.id.clone()).collect();
        let filtered = self.tools.specs_filtered(&names);
        let specs = if filtered.is_empty() {
            self.tools.specs()
        } else {
            filtered
        };

        // ── 命中技能动态注入 ──
        let mut skill_section = String::new();
        for skill in &skills {
            let marker = format!("<!-- skill:{} -->", skill.id);
            if system_prompt.contains(&marker) {
                continue;
            }
            if let Some(agent_skill) = self.skills.iter().find(|s| s.id == skill.id) {
                skill_section.push_str(&format!(
                    "\n---\n# Skill: {}\n{}\n<!-- skill:{} -->",
                    agent_skill.name, agent_skill.content, agent_skill.id
                ));
            }
        }
        (specs, skill_section)
    }

    /// Build a router index from the current tool registry + enabled skills
    /// (call after registering tools & skills).
    pub fn build_router(&self, _top_k_tools: usize) -> ToolRouter {
        let mut router = ToolRouter::new();
        let mut items: Vec<RouteItem> = self
            .skills
            .iter()
            .map(|s| RouteItem {
                id: s.id.clone(),
                kind: RouteKind::Skill,
                name: s.name.clone(),
                description: s.description.clone(),
                keywords: s.keywords.clone(),
                server_id: None,
            })
            .collect();
        items.extend(self.tools.tool_names().into_iter().map(|name| {
            RouteItem {
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
            }
        }));
        router.refresh(items);
        router
    }

    fn emit_trace(&self, trace: AgentTrace) {
        if let Some(cb) = &self.on_trace {
            cb(trace);
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .map(|t| t.is_cancelled())
            .unwrap_or(false)
    }

    /// Run one tool call, gated by HITL approval for High/Critical risk.
    async fn execute_tool(&self, call: &ToolCall) -> ToolOutput {
        let risk = assess_risk(&call.name, &call.arguments);

        if matches!(&risk, RiskLevel::High | RiskLevel::Critical) {
            // Already always-approved: skip the gate.
            if let Some(store) = &self.approval_store {
                if store
                    .is_always_approved(
                        self.agent_id.as_deref().unwrap_or_default(),
                        &call.name,
                        &call.arguments,
                    )
                    .await
                {
                    return self.run_tool(call).await;
                }
            }

            if risk == RiskLevel::Critical {
                let first = self.request_tool_approval(call, &risk, Some(1)).await;
                if !matches!(
                    first,
                    ToolApprovalResponse::Approved | ToolApprovalResponse::AlwaysApprove(_)
                ) {
                    return self.approval_error(call, &first);
                }
                let second = self.request_tool_approval(call, &risk, Some(2)).await;
                return match second {
                    ToolApprovalResponse::Approved => self.run_tool(call).await,
                    ToolApprovalResponse::AlwaysApprove(_) => ToolOutput::error(format!(
                        "工具「{}」二次确认不允许 AlwaysApprove，未执行",
                        call.name
                    )),
                    other => self.approval_error(call, &other),
                };
            }

            let response = self.request_tool_approval(call, &risk, None).await;
            match response {
                ToolApprovalResponse::Approved | ToolApprovalResponse::AlwaysApprove(_) => {
                    self.run_tool(call).await
                }
                other => self.approval_error(call, &other),
            }
        } else {
            self.run_tool(call).await
        }
    }

    /// 构建并发送审批请求，等待用户响应。
    async fn request_tool_approval(
        &self,
        call: &ToolCall,
        risk: &RiskLevel,
        confirm_step: Option<u32>,
    ) -> ToolApprovalResponse {
        let call_id = match confirm_step {
            Some(2) => format!("{}:confirm2", call.id),
            _ => call.id.clone(),
        };
        let request = ToolApprovalRequest {
            call_id: call_id.clone(),
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
            agent_id: self.agent_id.clone().unwrap_or_default(),
            risk_level: risk.clone(),
            description: match confirm_step {
                Some(2) => format!(
                    "工具「{}」为 Critical 操作，请再次确认执行（第 2 次）",
                    call.name
                ),
                _ => format!("工具「{}」请求执行（风险等级: {:?}）", call.name, risk),
            },
            kind: crate::core::adk::tool::ApprovalRequestKind::Tool,
            session_id: self.session_id.clone(),
            parent_session_id: None,
            parent_agent_id: None,
            child_agent_id: None,
            task_summary: None,
            capability_summary: None,
            confirm_step,
        };
        if let Some(app) = &self.app_handle {
            let _ = app.emit("tool:approval-request", &request);
        }

        match &self.approval_store {
            Some(store) => {
                let rx = store.request_approval(request).await;
                let timeout_secs = store.timeout_seconds().await;
                if timeout_secs == 0 {
                    return match rx.await {
                        Ok(resp) => resp,
                        Err(_) => ToolApprovalResponse::Defer,
                    };
                }
                match tokio::time::timeout(Duration::from_secs(timeout_secs as u64), rx).await {
                    Ok(Ok(resp)) => resp,
                    Ok(Err(_)) => ToolApprovalResponse::Defer,
                    Err(_) => {
                        store.abandon(&call_id, "expired", "审批超时").await;
                        ToolApprovalResponse::Defer
                    }
                }
            }
            None => ToolApprovalResponse::Defer,
        }
    }

    fn approval_error(&self, call: &ToolCall, response: &ToolApprovalResponse) -> ToolOutput {
        match response {
            ToolApprovalResponse::Rejected(reason) => {
                ToolOutput::error(format!("工具「{}」被用户拒绝: {}", call.name, reason))
            }
            _ => ToolOutput::error(format!("工具「{}」审批超时或已搁置，未执行", call.name)),
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
                    Some(server_id) => match rt
                        .call_tool(&server_id, &call.name, call.arguments.clone())
                        .await
                    {
                        Ok(result) => mcp_result_text(result),
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
        Self {
            server_id,
            tool_name,
            description,
            input_schema,
            runtime,
        }
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
        Ok(mcp_result_text(result))
    }
}

/// 提取 MCP 调用结果中的纯文本（`{"text": "..."}`），
/// 替代直接序列化整个结果 JSON，避免纯文本 provider 收到包装结构。
fn mcp_result_text(result: serde_json::Value) -> ToolOutput {
    match result.get("text").and_then(|t| t.as_str()) {
        Some(text) if !text.trim().is_empty() => ToolOutput::text(text.to_string()),
        _ => ToolOutput::text(format!(
            "[MCP 返回非文本内容，已降级为 JSON]\n{}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        )),
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
    let messages_len = serde_json::to_string(&request.messages)
        .unwrap_or_default()
        .len();
    let system_len = request.system.as_ref().map(|s| s.len()).unwrap_or(0);
    messages_len + system_len
}

/// Rough char-based token estimate (~4 chars/token) when the provider
/// reports no usage in the stream.
fn estimate_usage(prompt_chars: usize, completion_chars: usize) -> Usage {
    let prompt_tokens = prompt_chars.div_ceil(4) as u64;
    let completion_tokens = completion_chars.div_ceil(4) as u64;
    Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    }
}
