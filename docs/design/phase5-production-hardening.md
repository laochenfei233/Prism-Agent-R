# Prism Agent R — Phase 5（生产加固）详细设计

> **参考来源**：CrewAI（max_rpm/max_execution_time/guardrails/tracing/checkpointing）、AutoGen（GroupChat/安全审核）、LangGraph（状态机/检查点/human-in-the-loop）、OpenAI Codex（approval/audit）、Anthropic Claude Code（multi-agent orchestration）
> **适配原则**：借鉴成熟 Agent 框架的生产级能力，融入本项目三层架构（Phase 1）+ 工作流引擎（Phase 1 §10.6）+ 会话生命周期（Phase 4 §17.1），不重复造轮子
> **归属**：Phase 5（生产加固）· 由「面板控制多 Agent 并行运行」的生产需求驱动
> **总索引**：[`prism-agent-r.md`](../compose/specs/prism-agent-r.md) · **Phase 1-4**：[`phase1-core.md`](./phase1-core.md) · [`phase2-panel.md`](./phase2-panel.md) · [`phase3-extend.md`](./phase3-extend.md) · [`phase4-agentic.md`](./phase4-agentic.md)
> **Updated**：2026-08-08
> **读者假设**：面向熟悉 Rust（tokio/sqlx/serde）、Svelte 5（runes）、Tauri 2.x（IPC/WebView）的开发者；不解释语言/框架基础语法。
> **内容**：§22 预算监控与自动降级 · §23 越界拦截与安全护栏 · §24 异常记录与可观测性 · §25 工作流引擎重构 · §26 前端监控面板 · §27 自主编排循环（Spec→Plan→Execute→Review） · §28 任务清单（P5-T1~T20）

---

## 22. 预算监控与自动降级

> **问题**：当前 Phase 4 工作流执行无预算控制，Agent 可能无限消耗 token/费用；无自动降级机制（超预算直接失败）；参考 CrewAI `max_rpm`/`max_execution_time`/`usage_metrics` 与 OpenAI Codex 的 approval 机制。

### 22.1 预算层级设计

**三级预算体系**（参考 CrewAI Agent/Crew 两级 + 本项目扩展）：

```rust
// src-tauri/src/core/budget/mod.rs
pub struct BudgetConfig {
    pub global: GlobalBudget,        // 全局预算（跨所有工作流）
    pub crew: CrewBudget,            // 工作流级预算（单个工作流运行）
    pub agent: AgentBudget,          // Agent 级预算（单个 Agent 执行）
}

pub struct GlobalBudget {
    pub daily_token_limit: Option<u64>,      // 每日 token 上限
    pub daily_cost_limit: Option<f64>,       // 每日费用上限（美元）
    pub monthly_cost_limit: Option<f64>,     // 每月费用上限
    pub max_concurrent_workflows: usize,     // 最大并行工作流数（默认 4）
}

pub struct CrewBudget {
    pub max_tokens: Option<u64>,             // 单次运行 token 上限
    pub max_cost: Option<f64>,               // 单次运行费用上限
    pub max_execution_time_secs: Option<u64>, // 执行时间上限（秒）
    pub max_iterations: Option<u32>,          // 最大迭代次数
    pub max_rpm: Option<u32>,                // 每分钟请求上限（参考 CrewAI max_rpm）
}

pub struct AgentBudget {
    pub max_tokens: Option<u64>,             // 单 Agent token 上限
    pub max_iterations: Option<u32>,          // 单 Agent 迭代上限（参考 CrewAI max_iter，默认 20）
    pub max_execution_time_secs: Option<u64>, // 单 Agent 执行时间上限
    pub max_retry_limit: Option<u32>,         // 重试次数上限（参考 CrewAI max_retry_limit，默认 2）
}
```

**配置来源**（优先级从高到低）：
1. 工作流定义内联配置（`TaskDefinition.budget`）
2. Agent 配置（`agents.configuration.budget`）
3. 全局设置（`settings.budget`）
4. 默认值

### 22.2 预算追踪器

```rust
// src-tauri/src/core/budget/tracker.rs
pub struct BudgetTracker {
    global: Arc<RwLock<GlobalBudgetState>>,
    crew: RwLock<CrewBudgetState>,
    agents: RwLock<HashMap<String, AgentBudgetState>>,
}

pub struct GlobalBudgetState {
    pub daily_tokens_used: u64,
    pub daily_cost_used: f64,
    pub monthly_cost_used: f64,
    pub active_workflows: u32,
    pub last_reset: i64,
}

pub struct CrewBudgetState {
    pub tokens_used: u64,
    pub cost_used: f64,
    pub start_time: i64,
    pub iterations: u32,
    pub requests_made: Vec<i64>,  // 时间戳列表，用于 RPM 计算
}

pub struct AgentBudgetState {
    pub tokens_used: u64,
    pub iterations: u32,
    pub start_time: i64,
    pub retry_count: u32,
}
```

**追踪点**（与现有 RigAgent 循环集成）：

| 追踪点 | 位置 | 追踪内容 |
|--------|------|---------|
| LLM 调用完成 | `rig/agent.rs::run` | 累加 tokens_used / cost_used |
| 工具调用完成 | `rig/agent.rs::tool_call` | 累加 requests_made（RPM 计算） |
| 迭代完成 | `rig/agent.rs::loop` | 累加 iterations |
| 工作流阶段完成 | `workflow.rs::run` | 更新 CrewBudgetState |

### 22.3 自动降级策略

**策略引擎**（参考 CrewAI `respect_context_window` + 本项目扩展）：

```rust
pub enum BudgetAction {
    Continue,                    // 继续执行
    Warn { message: String },    // 警告但继续
    DowngradeModel,              // 切换到更便宜的模型
    PauseAndAsk,                 // 暂停并请求用户决策
    Terminate,                   // 终止执行
}

pub struct BudgetPolicy {
    pub on_token_warning: BudgetAction,     // token 达到 80% 时
    pub on_token_exceeded: BudgetAction,    // token 超限时
    pub on_cost_warning: BudgetAction,      // 费用达到 80% 时
    pub on_cost_exceeded: BudgetAction,     // 费用超限时
    pub on_time_exceeded: BudgetAction,     // 时间超限时
    pub on_rpm_exceeded: BudgetAction,      // RPM 超限时
    pub on_iteration_exceeded: BudgetAction, // 迭代超限时
}
```

**自动降级链**（参考 OpenAI Codex automatic fallbacks）：

```rust
pub struct ModelFallbackChain {
    pub models: Vec<ModelCandidate>,  // 按成本升序排列
    pub current_index: usize,
}

pub struct ModelCandidate {
    pub provider_id: String,
    pub model_id: String,
    pub cost_per_1k_tokens: f64,
    pub max_tokens: u64,
    pub capabilities: Vec<String>,  // ["chat", "tool_use", "reasoning"]
}

impl ModelFallbackChain {
    /// 超预算时切换到下一个更便宜的模型
    pub fn downgrade(&mut self) -> Option<&ModelCandidate> {
        if self.current_index + 1 < self.models.len() {
            self.current_index += 1;
            Some(&self.models[self.current_index])
        } else {
            None  // 无更便宜的模型可降级
        }
    }
}
```

**降级触发条件**：

| 条件 | 动作 | 参考 |
|------|------|------|
| token 使用量 ≥ 80% | 警告 + 日志 | CrewAI `respect_context_window` |
| token 使用量 ≥ 100% | 自动压缩上下文（Phase 4 §19.3.1）→ 失败则降级模型 | OpenAI compaction |
| 费用使用量 ≥ 80% | 警告 + 日志 | — |
| 费用使用量 ≥ 100% | 降级模型 或 PauseAndAsk | OpenAI automatic fallbacks |
| 执行时间 ≥ 阈值 | 降级模型 或 Terminate | CrewAI `max_execution_time` |
| RPM ≥ 阈值 | 暂停等待窗口重置 | CrewAI `max_rpm` |
| 迭代次数 ≥ 阈值 | 终止并返回当前最佳结果 | CrewAI `max_iter` |

### 22.4 预算事件与前端响应

**IPC 事件**：

| 事件 | 载荷 | 前端处理 |
|------|------|---------|
| `budget:warning` | `{ level, current, limit, entity_type, entity_id }` | 黄色警告徽标 |
| `budget:exceeded` | `{ level, action, entity_type, entity_id }` | 红色警告 + 操作按钮 |
| `budget:model-switched` | `{ old_model, new_model, reason }` | 模型切换通知 |
| `budget:paused` | `{ reason, entity_id }` | 暂停状态 + 恢复按钮 |

---

## 23. 越界拦截与安全护栏

> **问题**：当前 Phase 4 §10.12 护栏为输入级（prompt injection 检测），无运行时行为监控；参考 CrewAI `guardrails`、OpenAI Starlark 规则引擎、Anthropic multi-stage verification。

### 23.1 多层护栏架构

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: 输入级（现有 §10.12）                              │
│  - Prompt injection 检测                                     │
│  - 敏感词过滤                                                │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: 工具级（新增 §23.2）                               │
│  - 工具调用前权限校验                                        │
│  - 参数边界检查                                              │
│  - 工具白名单/黑名单                                         │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: 行为级（新增 §23.3）                               │
│  - 轨迹级监控（Phase 4 §19.3.5 增强）                       │
│  - 异常模式检测                                              │
│  - 越权访问拦截                                              │
├─────────────────────────────────────────────────────────────┤
│  Layer 4: 系统级（新增 §23.4）                               │
│  - 文件系统沙箱（路径白名单）                                │
│  - 网络访问控制（域名白名单）                                │
│  - 资源限制（CPU/内存/磁盘）                                 │
└─────────────────────────────────────────────────────────────┘
```

### 23.2 工具级护栏

```rust
// src-tauri/src/core/guardrails/tool_guard.rs
pub struct ToolGuardrail {
    pub policy: ToolPolicy,
}

pub struct ToolPolicy {
    pub allowed_tools: Option<Vec<String>>,    // 白名单（None = 全部允许）
    pub denied_tools: Vec<String>,             // 黑名单
    pub tool_configs: HashMap<String, ToolConfig>, // 各工具独立配置
}

pub struct ToolConfig {
    pub max_calls_per_run: Option<u32>,        // 单次运行最大调用次数
    pub require_approval: bool,                // 是否需要人工审批
    pub param_validators: Vec<ParamValidator>,  // 参数校验器
    pub timeout_secs: Option<u64>,             // 工具执行超时
}

pub enum ParamValidator {
    PathWhitelist(Vec<String>),    // 路径白名单
    PathBlacklist(Vec<String>),    // 路径黑名单
    Regex(String),                 // 正则校验
    LengthRange(usize, usize),     // 长度范围
    JsonSchema(serde_json::Value), // JSON Schema 校验
}
```

**工具调用拦截流程**：

```rust
pub async fn check_tool_call(
    &self,
    tool_name: &str,
    args: &serde_json::Value,
    context: &RunContext,
) -> Result<GuardrailDecision, AppError> {
    // 1. 黑名单检查
    if self.policy.denied_tools.contains(&tool_name.to_string()) {
        return Ok(GuardrailDecision::Deny {
            reason: format!("工具 '{}' 被策略禁止", tool_name),
        });
    }

    // 2. 白名单检查
    if let Some(allowed) = &self.policy.allowed_tools {
        if !allowed.contains(&tool_name.to_string()) {
            return Ok(GuardrailDecision::Deny {
                reason: format!("工具 '{}' 不在白名单中", tool_name),
            });
        }
    }

    // 3. 工具独立配置检查
    if let Some(config) = self.policy.tool_configs.get(tool_name) {
        // 调用次数检查
        if let Some(max) = config.max_calls_per_run {
            if context.tool_call_count(tool_name) >= max {
                return Ok(GuardrailDecision::Deny {
                    reason: format!("工具 '{}' 已达调用上限 {}", tool_name, max),
                });
            }
        }

        // 参数校验
        for validator in &config.param_validators {
            if !validator.validate(args) {
                return Ok(GuardrailDecision::Deny {
                    reason: format!("工具 '{}' 参数校验失败: {}", tool_name, validator.error_msg()),
                });
            }
        }

        // 审批检查
        if config.require_approval {
            return Ok(GuardrailDecision::NeedApproval {
                tool: tool_name.to_string(),
                args: args.clone(),
            });
        }
    }

    Ok(GuardrailDecision::Allow)
}
```

### 23.3 行为级护栏（轨迹监控增强）

```rust
// src-tauri/src/core/guardrails/trajectory.rs
pub struct TrajectoryGuardrail {
    pub checks: Vec<Box<dyn TrajectoryCheck>>,
    pub on_violation: ViolationHandler,
}

pub trait TrajectoryCheck: Send + Sync {
    fn name(&self) -> &str;
    fn check(&self, trajectory: &[AgentStep]) -> Option<Violation>;
}

pub struct Violation {
    pub check_name: String,
    pub severity: Severity,  // Low / Medium / High / Critical
    pub description: String,
    pub evidence: Vec<AgentStep>,  // 触发违规的步骤
}

pub enum Severity { Low, Medium, High, Critical }

pub enum ViolationHandler {
    LogOnly,                    // 仅记录
    PauseAndNotify,             // 暂停 + 通知用户
    Terminate,                  // 直接终止
    PauseAndAsk { timeout: u64 }, // 暂停 + 等待用户决策
}
```

**内置检查器**：

| 检查器 | 检测内容 | 严重级别 |
|--------|---------|---------|
| `CredentialConcatenationCheck` | 凭据/令牌拆分拼接重建 | Critical |
| `SandboxEscapeCheck` | 访问沙箱外路径/进程 | Critical |
| `PrivilegeEscalationCheck` | 多次失败后换路径重试 | High |
| `ResourceExhaustionCheck` | 循环调用同一工具无进展 | Medium |
| `DataLeakageCheck` | 输出包含敏感信息模式 | High |
| `PromptInjectionCheck` | Agent 输出包含指令模式 | Critical |

### 23.4 系统级护栏（沙箱）

```rust
// src-tauri/src/core/guardrails/sandbox.rs
pub struct SandboxPolicy {
    pub filesystem: FilesystemPolicy,
    pub network: NetworkPolicy,
    pub process: ProcessPolicy,
}

pub struct FilesystemPolicy {
    pub allowed_paths: Vec<String>,    // 允许访问的路径
    pub denied_paths: Vec<String>,     // 禁止访问的路径
    pub read_only_paths: Vec<String>,  // 只读路径
    pub max_file_size: u64,            // 最大文件大小
}

pub struct NetworkPolicy {
    pub allowed_domains: Vec<String>,  // 允许访问的域名
    pub denied_domains: Vec<String>,   // 禁止访问的域名
    pub allowed_ports: Vec<u16>,       // 允许的端口
    pub max_requests_per_minute: u32,  // 每分钟最大请求数
}

pub struct ProcessPolicy {
    pub allowed_commands: Vec<String>, // 允许执行的命令
    pub denied_commands: Vec<String>,  // 禁止执行的命令
    pub max_execution_time_secs: u64,  // 命令执行超时
    pub max_output_bytes: u64,         // 最大输出大小
}
```

---

## 24. 异常记录与可观测性

> **问题**：当前 Phase 4 §17.3 有 trace grading 但无结构化异常记录；无实时监控仪表盘；参考 CrewAI `tracing`（OpenTelemetry）+ `output_log_file` + `usage_metrics`。

### 24.1 结构化异常记录

```sql
-- 026_agent_exceptions.sql
CREATE TABLE IF NOT EXISTS agent_exceptions (
    id            TEXT PRIMARY KEY,
    session_id    TEXT NOT NULL,
    agent_id      TEXT NOT NULL,
    workflow_id   TEXT,
    run_id        TEXT,
    stage_id      TEXT,
    exception_type TEXT NOT NULL,     -- 'budget_exceeded' | 'guardrail_violation' | 'tool_error' | 'model_error' | 'timeout' | ...
    severity      TEXT NOT NULL,      -- 'low' | 'medium' | 'high' | 'critical'
    message       TEXT NOT NULL,
    context       TEXT,               -- JSON: 当前状态快照
    stack_trace   TEXT,
    tool_name     TEXT,
    tool_args     TEXT,
    model_id      TEXT,
    tokens_used   INTEGER,
    cost_used     REAL,
    created_at    INTEGER NOT NULL,
    resolved_at   INTEGER,
    resolved_by   TEXT,               -- 'auto' | 'user' | 'system'
    resolution    TEXT                -- 处理结果描述
);

CREATE INDEX IF NOT EXISTS idx_exceptions_session ON agent_exceptions(session_id);
CREATE INDEX IF NOT EXISTS idx_exceptions_agent ON agent_exceptions(agent_id);
CREATE INDEX IF NOT EXISTS idx_exceptions_type ON agent_exceptions(exception_type);
CREATE INDEX IF NOT EXISTS idx_exceptions_severity ON agent_exceptions(severity);
```

### 24.2 异常分类与处理

```rust
// src-tauri/src/core/observability/exception.rs
pub enum ExceptionType {
    BudgetExceeded { level: BudgetLevel },
    GuardrailViolation { check: String },
    ToolError { tool: String, error: String },
    ModelError { error: String },
    Timeout { duration_secs: u64 },
    ContextOverflow { current: usize, limit: usize },
    RateLimitExceeded { retry_after: Option<u64> },
    PermissionDenied { resource: String },
    ValidationError { field: String, message: String },
}

pub struct ExceptionRecorder {
    db: Database,
    on_exception: Option<Arc<dyn Fn(&AgentException) + Send + Sync>>,
}

impl ExceptionRecorder {
    pub async fn record(
        &self,
        session_id: &str,
        agent_id: &str,
        exception: ExceptionType,
        context: serde_json::Value,
    ) -> Result<AgentException, AppError> {
        let exc = AgentException {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            agent_id: agent_id.to_string(),
            exception_type: exception.type_name(),
            severity: exception.severity(),
            message: exception.message(),
            context: serde_json::to_string(&context)?,
            created_at: chrono::Utc::now().timestamp_millis(),
            // ...
        };

        // 写入数据库
        sqlx::query("INSERT INTO agent_exceptions ...")
            .execute(&self.db.pool)
            .await?;

        // 触发回调（通知前端）
        if let Some(f) = &self.on_exception {
            f(&exc);
        }

        Ok(exc)
    }
}
```

### 24.3 实时监控仪表盘

**IPC 事件**（实时推送）：

| 事件 | 载荷 | 频率 |
|------|------|------|
| `monitor:token-usage` | `{ agent_id, tokens_used, cost_used, timestamp }` | 每次 LLM 调用后 |
| `monitor:tool-call` | `{ agent_id, tool_name, duration_ms, success }` | 每次工具调用后 |
| `monitor:exception` | `{ agent_id, exception_type, severity, message }` | 异常发生时 |
| `monitor:guardrail` | `{ agent_id, check_name, action, reason }` | 护栏触发时 |
| `monitor:budget` | `{ entity_id, level, current, limit }` | 预算更新时 |

**前端仪表盘组件**（`DashboardPanel.svelte`）：

```
┌─────────────────────────────────────────────────────────────┐
│  📊 实时监控                                                │
├─────────────────────────────────────────────────────────────┤
│  Token 使用    ████████░░░░  67% (67k/100k)                 │
│  费用使用      ██████░░░░░░  52% ($5.2/$10)                 │
│  活跃工作流    2/4                                         │
│  活跃 Agent   4                                           │
├─────────────────────────────────────────────────────────────┤
│  ⚠️ 最近异常                                               │
│  [High] Token 超限 - Agent researcher (2 分钟前)            │
│  [Med]  工具超时 - web_search (5 分钟前)                    │
│  [Low]  重试成功 - Agent writer (8 分钟前)                  │
├─────────────────────────────────────────────────────────────┤
│  📈 趋势（最近 1 小时）                                     │
│  [Token 使用趋势图]                                        │
│  [费用使用趋势图]                                          │
│  [异常分布饼图]                                            │
└─────────────────────────────────────────────────────────────┘
```

### 24.4 日志系统

**分层日志**（参考 CrewAI `output_log_file`）：

```rust
pub enum LogLevel {
    Trace,   // 最详细：每次 LLM 请求/响应
    Debug,   // 调试：工具调用、模板渲染
    Info,    // 信息：工作流开始/结束、阶段切换
    Warn,    // 警告：预算接近上限、重试
    Error,   // 错误：工具失败、模型错误
    Fatal,   // 致命：系统级错误
}

pub struct AgentLogger {
    level: LogLevel,
    file_path: Option<String>,  // 日志文件路径
    json_format: bool,          // 是否 JSON 格式（便于解析）
}
```

**日志格式**（JSON Lines）：

```json
{
  "timestamp": "2026-08-08T10:30:00Z",
  "level": "info",
  "session_id": "sess_abc123",
  "agent_id": "researcher",
  "workflow_id": "wf_xyz",
  "run_id": "run_789",
  "stage_id": "research",
  "event": "llm_call",
  "model": "gpt-4o",
  "tokens": { "prompt": 1200, "completion": 800 },
  "cost": 0.012,
  "duration_ms": 2340,
  "message": "LLM 调用完成"
}
```

---

## 25. 工作流引擎重构

> **问题**：当前 Phase 1 §10.6 工作流引擎为简单线性执行，无预算/护栏/异常集成；参考 CrewAI sequential/hierarchical process + LangGraph state machine + 本项目 Phase 4 §17.2 AgentLoop。

### 25.1 工作流执行引擎重构

```rust
// src-tauri/src/core/workflow/engine.rs（重构）
pub struct WorkflowEngineV2 {
    coordinator: Arc<Coordinator>,
    budget_tracker: Arc<BudgetTracker>,
    tool_guard: Arc<ToolGuardrail>,
    trajectory_guard: Arc<TrajectoryGuardrail>,
    exception_recorder: Arc<ExceptionRecorder>,
    model_fallback: Arc<RwLock<ModelFallbackChain>>,
    on_stage: Option<Arc<dyn Fn(&StageEvent) + Send + Sync>>,
    on_exception: Option<Arc<dyn Fn(&AgentException) + Send + Sync>>,
    goal: Option<GoalMonitor>,
}

pub struct StageEvent {
    pub run_id: String,
    pub stage_id: String,
    pub agent_id: String,
    pub status: StageStatus,
    pub tokens_used: Option<u64>,
    pub cost_used: Option<f64>,
    pub duration_ms: Option<u64>,
}
```

**执行流程**（增强版）：

```rust
impl WorkflowEngineV2 {
    pub async fn run(
        &self,
        workflow: &Workflow,
        inputs: HashMap<String, serde_json::Value>,
        run_id: &str,
    ) -> Result<WorkflowResult, AppError> {
        // 1. 全局预算检查
        self.budget_tracker.check_global_budget().await?;

        // 2. 创建工作流级预算追踪
        let crew_budget = self.budget_tracker.create_crew_budget(run_id, &workflow.budget)?;

        // 3. 拓扑排序阶段
        let sorted_stages = topological_sort(&workflow.stages)?;

        // 4. 为每个角色创建 Actor（带预算/护栏）
        let actors = self.build_actors(workflow, &crew_budget).await?;

        // 5. 执行阶段
        for stage in &sorted_stages {
            // 5.1 检查工作流级预算
            if crew_budget.is_exceeded().await {
                self.record_exception(run_id, &stage.id, ExceptionType::BudgetExceeded { level: BudgetLevel::Crew }).await;
                break;
            }

            // 5.2 渲染模板
            let prompt = render_template(&stage.prompt_template, &inputs, &outputs)?;

            // 5.3 构建消息
            let msg = ActorMessage { /* ... */ };

            // 5.4 派发任务（带护栏检查）
            self.emit_stage(run_id, &stage.id, &StageStatus::Running);
            match self.execute_stage(run_id, stage, msg, &actors).await {
                Ok(reply) => {
                    outputs.insert(stage.id.clone(), reply.output.clone());
                    self.emit_stage(run_id, &stage.id, &StageStatus::Completed);
                }
                Err(e) => {
                    self.record_exception(run_id, &stage.id, ExceptionType::ToolError { error: e.to_string() }).await;
                    self.emit_stage(run_id, &stage.id, &StageStatus::Failed);
                    break;
                }
            }
        }

        // 6. 评估目标
        let goal_status = self.evaluate_goal(&outputs);

        // 7. 生成运行报告
        let report = self.generate_report(run_id, &crew_budget).await;

        Ok(WorkflowResult {
            run_id: run_id.to_string(),
            outputs,
            stage_results,
            goal_status,
            report,
        })
    }

    async fn execute_stage(
        &self,
        run_id: &str,
        stage: &WorkflowStage,
        msg: ActorMessage,
        actors: &HashMap<String, Arc<dyn AgentActor>>,
    ) -> Result<ActorReply, AppError> {
        let actor = actors.get(&stage.role)
            .ok_or_else(|| AppError::Internal(format!("角色 '{}' 无对应 Actor", stage.role)))?;

        // 工具护栏检查
        for tool in &msg.tools {
            match self.tool_guard.check_tool_call(tool, &serde_json::Value::Null, &RunContext::default()).await? {
                GuardrailDecision::Allow => {}
                GuardrailDecision::Deny { reason } => {
                    self.record_exception(run_id, &stage.id, ExceptionType::GuardrailViolation { check: tool.clone() }).await;
                    return Err(AppError::Guardrail(reason));
                }
                GuardrailDecision::NeedApproval { .. } => {
                    // 请求用户审批
                }
            }
        }

        // 执行（带预算追踪）
        let result = actor.handle(msg).await?;

        // 更新预算
        self.budget_tracker.record_usage(run_id, &stage.role, &result).await;

        Ok(result)
    }
}
```

### 25.2 工作流定义扩展

```rust
// src-tauri/src/core/workflow/definition.rs（扩展）
pub struct WorkflowV2 {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub inputs: Vec<TaskInput>,
    pub stages: Vec<WorkflowStageV2>,
    pub budget: Option<CrewBudget>,           // 工作流级预算
    pub guardrails: Option<ToolPolicy>,       // 工作流级护栏
    pub model_fallback: Option<Vec<String>>,  // 模型降级链
    pub on_exception: ExceptionPolicy,        // 异常处理策略
}

pub struct WorkflowStageV2 {
    pub id: String,
    pub name: String,
    pub role: String,
    pub agent_id: Option<String>,             // 指定具体 Agent（可选）
    pub prompt_template: String,
    pub tools: Vec<String>,
    pub max_iterations: Option<u32>,          // 覆盖全局
    pub budget: Option<AgentBudget>,          // Agent 级预算
    pub guardrails: Option<ToolPolicy>,       // 阶段级护栏
    pub depends_on: Vec<String>,
    pub retry_on_failure: Option<RetryPolicy>, // 失败重试策略
}

pub struct RetryPolicy {
    pub max_retries: u32,
    pub delay_ms: u64,
    pub backoff_multiplier: f64,
    pub retry_on_exceptions: Vec<String>,     // 仅特定异常类型重试
}

pub enum ExceptionPolicy {
    Terminate,                    // 立即终止
    ContinueAndLog,               // 继续执行并记录
    SkipStageAndContinue,         // 跳过当前阶段继续
    PauseAndAsk,                  // 暂停并请求用户
}
```

---

## 26. 前端监控面板

> **问题**：当前 Phase 4 前端无实时监控仪表盘；工作流运行状态显示简单；参考 CrewAI `verbose` + `stream` + `usage_metrics`。

### 26.1 监控面板布局

**位置**：侧边栏「监控」Tab 或独立视图（复用 §9.10 侧边栏组件结构）。

**组件结构**：

```
MonitorPanel.svelte
├── BudgetOverview.svelte          // 预算概览卡片
│   ├── TokenUsageBar.svelte       // Token 使用进度条
│   ├── CostUsageBar.svelte        // 费用使用进度条
│   └── ActiveWorkflows.svelte     // 活跃工作流数
├── ActiveWorkflows.svelte         // 活跃工作流列表
│   └── WorkflowCard.svelte        // 单个工作流卡片
│       ├── StageProgress.svelte   // 阶段进度
│       ├── TokenUsage.svelte      // Token 使用
│       └── ExceptionBadge.svelte  // 异常徽标
├── ExceptionLog.svelte            // 异常日志
│   └── ExceptionItem.svelte       // 单条异常
├── TrendCharts.svelte             // 趋势图表
│   ├── TokenTrendChart.svelte     // Token 使用趋势
│   ├── CostTrendChart.svelte      // 费用趋势
│   └── ExceptionPieChart.svelte   // 异常分布
└── ModelStatus.svelte             // 模型状态
    ├── CurrentModel.svelte        // 当前模型
    └── FallbackChain.svelte       // 降级链
```

### 26.2 实时数据流

```
后端事件 → Tauri IPC → Svelte Store → UI 组件

events:
  monitor:token-usage    → budgetStore.updateTokenUsage()
  monitor:exception      → exceptionStore.addException()
  monitor:guardrail      → guardrailStore.addEvent()
  workflow:stage         → workflowStore.updateStage()
  workflow:done          → workflowStore.markComplete()
```

### 26.3 前端 Store 设计

```typescript
// src/lib/stores/monitor.svelte.ts
interface MonitorState {
  budget: {
    global: GlobalBudgetState;
    activeWorkflows: WorkflowBudgetState[];
  };
  exceptions: AgentException[];
  guardrailEvents: GuardrailEvent[];
  trends: {
    tokenUsage: TrendPoint[];
    costUsage: TrendPoint[];
    exceptions: ExceptionTrend[];
  };
  models: {
    current: ModelInfo;
    fallbackChain: ModelInfo[];
    switchedAt: number | null;
  };
}

function createMonitorStore() {
  let state = $state<MonitorState>({ /* ... */ });

  // 监听后端事件
  listen('monitor:token-usage', (event) => {
    state.budget.global.tokensUsed = event.payload.tokens_used;
    state.trends.tokenUsage.push({ time: Date.now(), value: event.payload.tokens_used });
  });

  listen('monitor:exception', (event) => {
    state.exceptions.unshift(event.payload);
    // 最多保留 100 条
    if (state.exceptions.length > 100) state.exceptions.pop();
  });

  // 轮询刷新（备用）
  setInterval(async () => {
    const budget = await invoke('monitor:get-budget');
    state.budget = budget;
  }, 5000);

  return { get state() { return state; } };
}

export const monitorStore = createMonitorStore();
```

### 26.4 交互操作

| 操作 | 按钮 | 命令 | 说明 |
|------|------|------|------|
| 暂停工作流 | `⏸` | `workflow:pause` | 暂停当前工作流 |
| 继续工作流 | `▶` | `workflow:resume` | 继续已暂停工作流 |
| 终止工作流 | `⏹` | `workflow:stop` | 终止工作流 |
| 切换模型 | `🔄` | `model:switch` | 手动切换到降级链中的模型 |
| 查看异常详情 | `📋` | — | 展开异常上下文 |
| 导出日志 | `📥` | `log:export` | 导出 JSON Lines 日志 |
| 清除异常 | `🗑️` | `exception:clear` | 清除已处理异常 |

---

## 28. 任务清单（P5-T1~T20）

### §22 预算监控（T1-T3）

- [ ] **T1**: BudgetConfig + BudgetTracker + 预算事件
  - 验收：三级预算配置生效；超预算触发事件；前端收到 warning/exceeded 事件
  - covers: §22.1-22.4

- [ ] **T2**: ModelFallbackChain + 自动降级策略
  - 验收：超预算时自动切换到更便宜模型；无更便宜模型时 PauseAndAsk
  - covers: §22.3; depends: T1

- [ ] **T3**: WorkflowEngineV2 集成预算追踪
  - 验收：工作流执行时实时追踪 token/费用；超预算自动暂停/降级
  - covers: §22.2, §25.1; depends: T1, T2

### §23 越界拦截（T4-T6）

- [ ] **T4**: ToolGuardrail + 工具级护栏配置
  - 验收：工具调用前校验白名单/黑名单/参数；需审批工具触发 approval
  - covers: §23.2

- [ ] **T5**: TrajectoryGuardrail + 行为级检查器
  - 验收：凭据拼接/沙箱逃逸/越权访问触发拦截；违规记录入库
  - covers: §23.3

- [ ] **T6**: SandboxPolicy + 系统级沙箱
  - 验收：文件/网络/进程访问受白名单限制；越界访问被拦截
  - covers: §23.4

### §24 异常记录（T7-T9）

- [ ] **T7**: 异常数据库表 + ExceptionRecorder
  - 验收：异常记录入库；支持按 session/agent/type 查询
  - covers: §24.1-24.2

- [ ] **T8**: AgentLogger + 结构化日志
  - 验收：日志按级别输出；支持 JSON Lines 格式；可配置文件输出
  - covers: §24.4

- [ ] **T9**: 监控仪表盘前端组件
  - 验收：实时显示预算/异常/趋势；支持交互操作
  - covers: §24.3, §26

### §25 工作流重构（T10-T12）

- [ ] **T10**: WorkflowEngineV2 核心重构
  - 验收：集成预算/护栏/异常记录；支持重试策略
  - covers: §25.1; depends: T1, T4, T7

- [ ] **T11**: WorkflowV2 定义扩展
  - 验收：支持工作流/阶段级预算+护栏配置；支持重试策略
  - covers: §25.2

- [ ] **T12**: 工作流命令迁移（workflow_run → V2）
  - 验收：现有工作流使用新引擎；无回归
  - covers: §25.1; depends: T10, T11

### §26 前端监控（T13-T15）

- [ ] **T13**: monitorStore + IPC 事件监听
  - 验收：实时接收并存储监控数据；支持轮询备用
  - covers: §26.3

- [ ] **T14**: MonitorPanel 主面板组件
  - 验收：展示预算/活跃工作流/异常/趋势；布局对齐 §18.7
  - covers: §26.1

- [ ] **T15**: 交互操作（暂停/继续/终止/切换模型）
  - 验收：按钮触发对应命令；状态实时更新
  - covers: §26.4; depends: T14

### §27 自主编排循环（T16-T20）

- [ ] **T16**: OrchestratorSession + 数据结构 + SQLite 表
  - 验收：会话状态可持久化；崩溃后可恢复
  - covers: §27.2

- [ ] **T17**: SpecGenerator（需求分析 + 任务拆解）
  - 验收：输入模糊需求 → 输出 SPEC（任务清单 + 验收标准 + 依赖）
  - covers: §27.3

- [ ] **T18**: PlanGenerator（任务分组 + Agent 分配 + 并行识别）
  - 验收：SPEC → 执行计划（并行组 + 顺序依赖 + 模型分配）
  - covers: §27.3; depends: T17

- [ ] **T19**: OrchestratorEngine 主循环（Spec→Plan→Execute→Review→循环）
  - 验收：完整循环执行；支持暂停/恢复；预算耗尽自动停止
  - covers: §27.3, §27.6; depends: T16, T17, T18

- [ ] **T20**: 前端自主编排界面（输入/SPEC预览/执行监控/审查结果）
  - 验收：用户可输入需求、查看 SPEC、监控执行、查看审查结果
  - covers: §27.4; depends: T19

---

## 27. 自主编排循环（Spec → Plan → Execute → Review）

> **问题**：当前工作流为预定义模板，用户需手动设计阶段；缺少「输入模糊需求 → 自动生成计划 → 多 Agent 并行执行 → 主 Agent 审查 → 循环直至完成」的自主能力；参考 MiMoCode 的 compose 模式（brainstorm → spec → plan → execute → verify → review）。

### 27.1 核心流程

```
┌─────────────────────────────────────────────────────────────────┐
│  用户输入模糊需求                                                │
│  "帮我实现一个用户认证系统，包含登录、注册、JWT"                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Phase 1: Spec 生成（主 Agent - Planner）                        │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ 1. 分析需求，拆解为可执行任务                               │  │
│  │ 2. 生成 SPEC.md（任务清单 + 验收标准 + 依赖关系）           │  │
│  │ 3. 用户确认（可选）                                        │  │
│  └──────────────────────────────────────────────────────────┘  │
│  输出: { tasks: [...], dependencies: {...}, acceptance: {...} }  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Phase 2: Plan 生成（主 Agent - Planner）                        │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ 1. 识别可并行任务（无依赖关系的）                            │  │
│  │ 2. 为每个任务分配 Agent（选择模型/角色/工具）                │  │
│  │ 3. 生成执行计划（并行组 + 顺序依赖）                        │  │
│  └──────────────────────────────────────────────────────────┘  │
│  输出: { groups: [ {parallel: [T1, T2]}, {sequential: [T3]} ] } │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Phase 3: Execute（多 Agent 并行）                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │ Agent A     │  │ Agent B     │  │ Agent C     │            │
│  │ (gpt-4o)   │  │ (claude-3)  │  │ (local-7b) │            │
│  │ T1: 数据库   │  │ T2: API     │  │ T3: 前端    │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
│       │                │                │                      │
│       ▼                ▼                ▼                      │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ 收集结果 + 记录异常 + 更新进度                              │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Phase 4: Review（主 Agent - Reviewer）                          │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ 1. 检查每个任务的产出是否符合验收标准                        │  │
│  │ 2. 检查跨任务一致性（接口/类型/命名）                       │  │
│  │ 3. 标记需要修复的任务                                       │  │
│  └──────────────────────────────────────────────────────────┘  │
│  输出: { passed: [T1, T2], failed: [T3], reasons: {...} }       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  循环判断                                                        │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ if 所有任务通过: → 完成                                     │  │
│  │ if 有失败任务:   → 生成修复计划 → 返回 Phase 3              │  │
│  │ if 用户暂停:     → 保存状态 → 等待恢复                      │  │
│  │ if 预算耗尽:     → 暂停 + 通知用户                         │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 27.2 数据结构

```rust
// src-tauri/src/core/orchestrator/mod.rs

/// 自主编排会话
pub struct OrchestratorSession {
    pub id: String,
    pub user_request: String,           // 用户原始输入
    pub spec: Option<SpecDocument>,     // 生成的 SPEC
    pub plan: Option<ExecutionPlan>,    // 执行计划
    pub status: OrchestratorStatus,     // 当前状态
    pub budget: BudgetTracker,          // 预算追踪
    pub history: Vec<OrchestrationEvent>, // 事件历史
    pub created_at: i64,
    pub updated_at: i64,
}

pub enum OrchestratorStatus {
    SpecGenerating,     // 正在生成 SPEC
    SpecReviewing,      // 等待用户确认 SPEC（可选）
    PlanGenerating,     // 正在生成执行计划
    Executing,          // 正在执行
    Reviewing,          // 正在审查
    Repairing,          // 正在修复失败任务
    Completed,          // 全部完成
    Paused,             // 用户暂停
    BudgetExhausted,    // 预算耗尽
    Failed(String),     // 失败
}

/// SPEC 文档（类似 MiMoCode 的 spec）
#[derive(Serialize, Deserialize)]
pub struct SpecDocument {
    pub id: String,
    pub summary: String,                // 需求摘要
    pub tasks: Vec<SpecTask>,           // 任务清单
    pub acceptance_criteria: HashMap<String, Vec<String>>, // 任务→验收标准
    pub dependencies: HashMap<String, Vec<String>>,        // 任务→依赖
    pub out_of_scope: Vec<String>,      // 明确排除的内容
}

#[derive(Serialize, Deserialize)]
pub struct SpecTask {
    pub id: String,                     // T1, T2, ...
    pub title: String,
    pub description: String,
    pub acceptance: Vec<String>,        // 验收标准列表
    pub estimated_complexity: Complexity,
    pub required_tools: Vec<String>,
    pub suggested_model: Option<String>, // 建议使用的模型
}

pub enum Complexity { Low, Medium, High }

/// 执行计划
#[derive(Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub groups: Vec<ExecutionGroup>,    // 执行组（按顺序）
    pub total_tasks: u32,
    pub estimated_tokens: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct ExecutionGroup {
    pub id: String,
    pub kind: GroupKind,                // Parallel 或 Sequential
    pub tasks: Vec<PlannedTask>,
}

pub enum GroupKind { Parallel, Sequential }

#[derive(Serialize, Deserialize)]
pub struct PlannedTask {
    pub spec_task_id: String,           // 关联 SPEC 任务
    pub agent_config: AgentConfig,      // Agent 配置
    pub prompt: String,                 // 完整提示词
    pub tools: Vec<String>,
    pub timeout_secs: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct AgentConfig {
    pub role: String,
    pub model_provider: String,         // "openai" | "anthropic" | "ollama"
    pub model_id: String,               // "gpt-4o" | "claude-3-opus" | "qwen-7b"
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}
```

### 27.3 编排引擎

```rust
// src-tauri/src/core/orchestrator/engine.rs

pub struct OrchestratorEngine {
    db: Database,
    planner_model: Arc<dyn ModelProvider>,   // 规划用模型（强推理）
    reviewer_model: Arc<dyn ModelProvider>,  // 审查用模型（强推理）
    worker_pool: Arc<TaskScheduler>,         // Worker 调度器
    budget_tracker: Arc<BudgetTracker>,
    exception_recorder: Arc<ExceptionRecorder>,
    on_event: Option<Arc<dyn Fn(&OrchestrationEvent) + Send + Sync>>,
}

impl OrchestratorEngine {
    /// 主循环：Spec → Plan → Execute → Review → 循环
    pub async fn run(&self, session: &mut OrchestratorSession) -> Result<(), AppError> {
        loop {
            match &session.status {
                // Phase 1: 生成 SPEC
                OrchestratorStatus::SpecGenerating => {
                    let spec = self.generate_spec(&session.user_request, &session.budget).await?;
                    session.spec = Some(spec);
                    session.status = OrchestratorStatus::PlanGenerating;
                    self.emit_event(session, "spec_generated");
                }

                // Phase 2: 生成执行计划
                OrchestratorStatus::PlanGenerating => {
                    let spec = session.spec.as_ref().unwrap();
                    let plan = self.generate_plan(spec, &session.budget).await?;
                    session.plan = Some(plan);
                    session.status = OrchestratorStatus::Executing;
                    self.emit_event(session, "plan_generated");
                }

                // Phase 3: 并行执行
                OrchestratorStatus::Executing => {
                    let results = self.execute_plan(session).await?;
                    self.record_results(session, &results).await?;
                    session.status = OrchestratorStatus::Reviewing;
                    self.emit_event(session, "execution_completed");
                }

                // Phase 4: 审查
                OrchestratorStatus::Reviewing => {
                    let review = self.review_results(session).await?;
                    if review.all_passed() {
                        session.status = OrchestratorStatus::Completed;
                        self.emit_event(session, "review_passed");
                        break;
                    } else {
                        // 生成修复计划，重新执行
                        self.generate_repair_plan(session, &review).await?;
                        session.status = OrchestratorStatus::Executing;
                        self.emit_event(session, "review_failed");
                    }
                }

                // 用户暂停 / 预算耗尽
                OrchestratorStatus::Paused | OrchestratorStatus::BudgetExhausted => {
                    self.save_session(session).await?;
                    break;
                }

                _ => break,
            }

            // 检查预算
            if session.budget.is_exceeded().await {
                session.status = OrchestratorStatus::BudgetExhausted;
                self.emit_event(session, "budget_exhausted");
                break;
            }
        }

        Ok(())
    }

    /// 生成 SPEC（需求分析 + 任务拆解）
    async fn generate_spec(
        &self,
        user_request: &str,
        budget: &BudgetTracker,
    ) -> Result<SpecDocument, AppError> {
        let prompt = format!(
            r#"你是一个专业的软件架构师。请分析以下需求，生成详细的 SPEC 文档。

用户需求：
{user_request}

请输出 JSON 格式的 SPEC，包含：
1. summary: 需求摘要（1-2 句话）
2. tasks: 任务清单（每个任务包含 id, title, description, acceptance, estimated_complexity, required_tools, suggested_model）
3. acceptance_criteria: 任务→验收标准映射
4. dependencies: 任务→依赖关系
5. out_of_scope: 明确排除的内容

要求：
- 任务拆解粒度适中（不要太细也不要太粗）
- 每个任务必须有可验证的验收标准
- 依赖关系必须无环
- 为每个任务建议合适的模型（简单任务用小模型，复杂任务用大模型）"#
        );

        let request = GenerationRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: MessageContent::Text(prompt),
                name: None,
            }],
            model: Some(self.planner_model.model_id()),
            ..Default::default()
        };

        let result = self.planner_model.generate(request).await?;
        let spec: SpecDocument = serde_json::from_str(&result.text)?;
        Ok(spec)
    }

    /// 生成执行计划（任务分组 + Agent 分配）
    async fn generate_plan(
        &self,
        spec: &SpecDocument,
        budget: &BudgetTracker,
    ) -> Result<ExecutionPlan, AppError> {
        // 拓扑排序任务
        let sorted_tasks = self.topological_sort_tasks(spec)?;

        // 识别可并行组
        let groups = self.identify_parallel_groups(&sorted_tasks, spec)?;

        // 为每个任务分配 Agent
        let mut planned_groups = Vec::new();
        for group in groups {
            let mut planned_tasks = Vec::new();
            for task_id in &group.tasks {
                let spec_task = spec.tasks.iter().find(|t| &t.id == task_id).unwrap();
                let agent_config = self.assign_agent(spec_task)?;
                let prompt = self.build_task_prompt(spec_task, spec)?;

                planned_tasks.push(PlannedTask {
                    spec_task_id: task_id.clone(),
                    agent_config,
                    prompt,
                    tools: spec_task.required_tools.clone(),
                    timeout_secs: Some(300), // 默认 5 分钟
                });
            }

            planned_groups.push(ExecutionGroup {
                id: uuid::Uuid::new_v4().to_string(),
                kind: if planned_tasks.len() > 1 { GroupKind::Parallel } else { GroupKind::Sequential },
                tasks: planned_tasks,
            });
        }

        Ok(ExecutionPlan {
            groups: planned_groups,
            total_tasks: spec.tasks.len() as u32,
            estimated_tokens: None,
        })
    }

    /// 并行执行计划
    async fn execute_plan(
        &self,
        session: &OrchestratorSession,
    ) -> Result<Vec<TaskResult>, AppError> {
        let plan = session.plan.as_ref().unwrap();
        let mut all_results = Vec::new();

        for group in &plan.groups {
            match &group.kind {
                GroupKind::Parallel => {
                    // 并行执行组内所有任务
                    let handles: Vec<_> = group.tasks.iter().map(|task| {
                        let engine = self.clone();
                        let task = task.clone();
                        async move {
                            engine.execute_task(&task).await
                        }
                    }).collect();

                    let results = futures::future::join_all(handles).await;
                    for result in results {
                        all_results.push(result?);
                    }
                }
                GroupKind::Sequential => {
                    // 顺序执行
                    for task in &group.tasks {
                        let result = self.execute_task(task).await?;
                        all_results.push(result);
                    }
                }
            }
        }

        Ok(all_results)
    }

    /// 执行单个任务
    async fn execute_task(&self, task: &PlannedTask) -> Result<TaskResult, AppError> {
        let start_time = chrono::Utc::now().timestamp_millis();

        // 创建 Actor
        let actor = GenericActor::new(
            task.spec_task_id.clone(),
            task.agent_config.role.clone(),
            self.create_provider(&task.agent_config)?,
            task.agent_config.system_prompt.clone().unwrap_or_default(),
            self.create_tool_registry(&task.tools)?,
        );

        // 执行
        let msg = ActorMessage {
            task_id: task.spec_task_id.clone(),
            prompt: task.prompt.clone(),
            tools: task.tools.clone(),
            context: None,
        };

        match actor.handle(msg).await {
            Ok(reply) => {
                let duration = chrono::Utc::now().timestamp_millis() - start_time;
                Ok(TaskResult {
                    task_id: task.spec_task_id.clone(),
                    status: TaskStatus::Completed,
                    output: reply.output,
                    tokens_used: reply.tokens_used,
                    duration_ms: duration,
                    error: None,
                })
            }
            Err(e) => {
                let duration = chrono::Utc::now().timestamp_millis() - start_time;
                Ok(TaskResult {
                    task_id: task.spec_task_id.clone(),
                    status: TaskStatus::Failed,
                    output: String::new(),
                    tokens_used: None,
                    duration_ms: duration,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// 审查结果
    async fn review_results(
        &self,
        session: &OrchestratorSession,
    ) -> Result<ReviewResult, AppError> {
        let spec = session.spec.as_ref().unwrap();
        let results = self.get_task_results(session).await?;

        let prompt = format!(
            r#"你是一个严格的代码审查员。请审查以下任务的输出是否符合 SPEC 中的验收标准。

SPEC 任务清单：
{spec_json}

任务输出：
{results_json}

请为每个任务给出：
1. passed: bool（是否通过）
2. reasons: Vec<String>（不通过的原因）
3. suggestions: Vec<String>（改进建议）

输出 JSON 格式。"#
        );

        let request = GenerationRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: MessageContent::Text(prompt),
                name: None,
            }],
            model: Some(self.reviewer_model.model_id()),
            ..Default::default()
        };

        let result = self.reviewer_model.generate(request).await?;
        let review: ReviewResult = serde_json::from_str(&result.text)?;
        Ok(review)
    }
}
```

### 27.4 前端交互

**输入界面**：

```
┌─────────────────────────────────────────────────────────────────┐
│  🚀 自主编排                                                     │
├─────────────────────────────────────────────────────────────────┤
│  请输入你的需求：                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ 帮我实现一个用户认证系统，包含登录、注册、JWT             │   │
│  │ token 刷新、权限中间件                                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  [生成 SPEC]                                                   │
└─────────────────────────────────────────────────────────────────┘
```

**SPEC 预览**：

```
┌─────────────────────────────────────────────────────────────────┐
│  📋 SPEC 预览                                                    │
├─────────────────────────────────────────────────────────────────┤
│  需求摘要：实现完整的用户认证系统，含登录/注册/JWT/刷新/权限     │
│                                                                 │
│  任务清单：                                                     │
│  ☐ T1: 数据库设计（用户表 + token 表）           [Low]         │
│  ☐ T2: 注册 API（邮箱验证 + 密码哈希）           [Medium]      │
│  ☐ T3: 登录 API（JWT 签发）                      [Medium]      │
│  ☐ T4: Token 刷新 API                            [Medium]      │
│  ☐ T5: 权限中间件（RBAC）                         [High]        │
│  ☐ T6: 前端登录/注册页面                          [Medium]      │
│                                                                 │
│  依赖关系：T1 → T2, T3, T4 → T5, T6                           │
│                                                                 │
│  [确认并执行]  [修改 SPEC]                                      │
└─────────────────────────────────────────────────────────────────┘
```

**执行监控**：

```
┌─────────────────────────────────────────────────────────────────┐
│  ⚡ 执行进度                                                     │
├─────────────────────────────────────────────────────────────────┤
│  第 1 组（并行）：                                               │
│  ✅ T1: 数据库设计     [gpt-4o]     2.3s    1.2k tokens        │
│                                                                 │
│  第 2 组（并行）：                                               │
│  ⏳ T2: 注册 API       [claude-3]   执行中...                   │
│  ⏳ T3: 登录 API       [gpt-4o]     执行中...                   │
│  ⏳ T4: Token 刷新     [local-7b]   排队中                      │
│                                                                 │
│  第 3 组（待执行）：                                             │
│  ⏸ T5: 权限中间件     [gpt-4o]     等待中                      │
│  ⏸ T6: 前端页面       [claude-3]   等待中                      │
│                                                                 │
│  总进度：██████░░░░░░░░░░ 1/6 (17%)                            │
│  Token 使用：1.2k / 100k                                       │
│  费用：$0.02 / $10.00                                           │
│                                                                 │
│  [暂停]  [终止]                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**审查结果**：

```
┌─────────────────────────────────────────────────────────────────┐
│  🔍 审查结果                                                     │
├─────────────────────────────────────────────────────────────────┤
│  ✅ T1: 数据库设计     通过                                    │
│  ✅ T2: 注册 API       通过                                    │
│  ❌ T3: 登录 API       未通过                                   │
│     原因：缺少密码错误次数限制                                   │
│     建议：添加 login_attempts 表 + 锁定机制                     │
│  ⏳ T4: Token 刷新     待审查                                   │
│  ⏳ T5: 权限中间件     待执行                                   │
│  ⏳ T6: 前端页面       待执行                                   │
│                                                                 │
│  [自动修复 T3]  [跳过]  [手动处理]                              │
└─────────────────────────────────────────────────────────────────┘
```

### 27.5 关键特性

| 特性 | 实现 | 参考 |
|------|------|------|
| **SPEC 自动生成** | 主 Agent 分析需求 → 生成任务清单 + 验收标准 | MiMoCode compose:brainstorm |
| **智能任务分组** | 拓扑排序 + 依赖分析 → 识别可并行组 | MiMoCode compose:parallel |
| **模型智能分配** | 简单任务用小模型，复杂任务用大模型 | CrewAI function_calling_llm |
| **并行执行** | tokio::spawn + join_all | 本项目 Phase 1 §10.6 |
| **自动审查** | Reviewer Agent 检查验收标准 | MiMoCode compose:review |
| **自动修复** | 失败任务生成修复计划 → 重新执行 | MiMoCode compose:debug |
| **循环控制** | 最大循环次数 + 预算上限 + 用户暂停 | — |
| **状态持久化** | SQLite 保存会话状态，崩溃可恢复 | Phase 4 §17.1 会话状态机 |

### 27.6 循环控制规则

```rust
pub struct LoopControl {
    pub max_cycles: u32,              // 最大循环次数（默认 5）
    pub max_tasks_per_cycle: u32,     // 每轮最大任务数（默认 20）
    pub budget_limit: BudgetLimit,    // 预算限制
    pub pause_on_user: bool,          // 用户请求时暂停
    pub pause_on_budget: bool,        // 预算耗尽时暂停
}

impl LoopControl {
    pub fn should_continue(&self, session: &OrchestratorSession) -> LoopDecision {
        // 1. 检查循环次数
        if session.cycle_count >= self.max_cycles {
            return LoopDecision::Stop("达到最大循环次数".into());
        }

        // 2. 检查预算
        if self.pause_on_budget && session.budget.is_exceeded_sync() {
            return LoopDecision::Pause("预算耗尽".into());
        }

        // 3. 检查用户暂停
        if self.pause_on_user && session.user_requested_pause {
            return LoopDecision::Pause("用户请求暂停".into());
        }

        // 4. 检查所有任务是否完成
        if session.all_tasks_passed() {
            return LoopDecision::Complete;
        }

        LoopDecision::Continue
    }
}

pub enum LoopDecision {
    Continue,
    Complete,
    Pause(String),
    Stop(String),
}
```

---

## 附录：与现有架构的关系

| 本阶段新增 | 复用 | 不重复造 |
|-----------|------|---------|
| BudgetTracker | Phase 4 §17.1 会话状态机 | 不另建预算体系 |
| ToolGuardrail | Phase 4 §10.12 护栏 / §19.3.5 轨迹监控 | 不重写输入级护栏 |
| ExceptionRecorder | Phase 4 §17.3 trace grading | 不另建评测体系 |
| WorkflowEngineV2 | Phase 1 §10.6 工作流引擎 / Phase 4 §17.2 AgentLoop | 不重写编排核心 |
| MonitorPanel | Phase 4 §18.5 Loop 设计区 / §18.6 轨迹回放 | 不另建前端壳 |
| OrchestratorEngine | Phase 4 §17.2 AgentLoop（Goal/Timer/Maker-Checker） | 不重写循环调度 |
| SpecGenerator | Phase 4 §17.2 GoalMonitor（目标定义） | 不另建目标体系 |
| ReviewerAgent | Phase 4 §10.9 反思（生产者-评审者） | 不重写审查逻辑 |
