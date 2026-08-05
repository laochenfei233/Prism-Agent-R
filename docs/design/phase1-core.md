# Prism Agent R — Phase 1（Agent 核心闭环）详细设计

> **归属**：Phase 1（MVP Agent 核心闭环）· 本文件来自 `prism-agent-r` 设计文档按阶段拆分
> **总索引**：[`prism-agent-r.md`](../compose/specs/prism-agent-r.md) · **Phase 2**：[`phase2-panel.md`](./phase2-panel.md) · **Phase 3**：[`phase3-extend.md`](./phase3-extend.md)
> **Updated**：2026-08-05
> **内容**：§3 后端三层架构 · §4 目录结构 · §5 数据库 · §6 MCP · §7 流式响应 · §8 IPC 命令 · §9.1-9.8 前端基础 · §10.4 Skill · §10.6 工作流引擎 · §10.7 记忆基础 · §10.8 文件 · §11 错误日志 · §12 安全 · §13 性能 · §14 旧版规避

---

## 3. Rust 后端分层架构（三层 Agent 框架）

### 3.1 分层职责与依赖方向

```
┌────────────────────────────────────────────────┐
│  Orchestration Layer (AutoAgents)               │  ← 最上层：编排
│  多 Agent 协作 · 工作流 · 任务分发               │
│  depends_on: Rig Layer, ADK Layer               │
├────────────────────────────────────────────────┤
│  Core Layer (Rig)                               │  ← 核心：单 Agent 执行
│  LLM 调用 · 流式生成 · 工具循环                 │
│  depends_on: ADK Layer (Model/Tool 抽象)        │
├────────────────────────────────────────────────┤
│  Component Layer (ADK-Rust)                     │  ← 基础：组件抽象
│  Model Trait · Tool Trait · Memory · Prompt     │
│  不依赖上层，可独立测试                          │
└────────────────────────────────────────────────┘
```

**关键原则**：
- **依赖单向**：AutoAgents → Rig → ADK，禁止反向依赖
- **抽象下沉**：Rig 的 Provider 适配器实现 ADK 的 `ModelProvider` Trait
- **编排解耦**：单 Agent 场景直接走 Rig（跳过 AutoAgents），零编排开销

### 3.2 ADK-Rust 组件层（最底层，抽象定义）

职责：定义所有跨层复用的 Trait 与数据结构，不依赖任何具体实现。

```rust
// core/adk/model.rs — 模型抽象
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// 模型唯一标识（provider:model_id）
    fn id(&self) -> &str;
    /// 非流式生成（用于工具结果处理、小模型）
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AgentError>;
    /// 流式生成（用于对话主流程）
    async fn stream(&self, request: GenerationRequest) -> Result<StreamHandle, AgentError>;
    /// 返回该模型支持的配置项（温度、max_tokens 等）
    fn capabilities(&self) -> ModelCapabilities;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub messages: Vec<ChatMessage>,
    pub system: Option<String>,
    pub tools: Vec<ToolSpec>,          // 本次注入的工具
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatRole { System, User, Assistant, Tool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: MessageContent,       // Text | ToolCall | ToolResult
    pub name: Option<String>,
}

// core/adk/tool.rs — 工具抽象
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;         // JSON Schema 用于 LLM
    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError>;
}

// core/adk/memory.rs — 记忆抽象
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// 从会话记忆中构建注入上下文
    async fn build_context(&self, session_id: &str, agent_id: &str) -> Result<MemoryContext, AgentError>;
    /// 对话结束后更新记忆
    async fn record(&self, session_id: &str, agent_id: &str, exchange: MessageExchange) -> Result<(), AgentError>;
    /// 全局/项目级长期记忆查询
    async fn search(&self, query: &str, scope: MemoryScope) -> Result<Vec<MemoryItem>, AgentError>;
}
```

### 3.3 Rig 核心层（单 Agent 执行）

职责：实现单个 Agent 的完整推理循环（agentic loop）。

```
                    ┌──────────────────────────┐
                    │   RigAgent (rig 框架)     │
                    └───────────┬──────────────┘
                                │ agent_loop
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌───────────────┐    ┌───────────────────┐    ┌─────────────────┐
│ Rig Provider   │    │ ToolCallHandler   │    │ StreamPipeline   │
│ (实现 ADK      │    │ (内置/MCP 分发)    │    │ (Token 汇聚/     │
│  ModelProvider)│    │                   │    │  事件发射)       │
└───────────────┘    └───────────────────┘    └─────────────────┘
```

核心实现：

```rust
// core/rig/agent.rs
pub struct RigAgent {
    pub model_provider: Arc<dyn ModelProvider>,      // ADK 抽象
    pub system_prompt: String,
    pub tools: Vec<Arc<dyn ToolExecutor>>,
    pub max_iterations: u32,                          // 最大工具循环轮数
}

impl RigAgent {
    /// 执行 agentic loop：生成 → 工具调用 → 回填 → 再生成
    pub async fn run(&self, request: GenerationRequest) -> Result<AgentRunResult, AgentError> {
        let mut current = request;
        for _ in 0..self.max_iterations {
            let mut handle = self.model_provider.stream(current.clone()).await?;
            let mut tool_calls = Vec::new();
            let mut final_text = String::new();

            while let Some(event) = handle.next().await {
                match event {
                    StreamEvent::Text(t) => final_text.push_str(&t),
                    StreamEvent::ToolCall(call) => tool_calls.push(call),
                    StreamEvent::Finish => break,
                }
            }

            if tool_calls.is_empty() {
                return Ok(AgentRunResult { text: final_text, tool_calls: Vec::new() });
            }

            // 执行工具并回填
            let mut tool_results = Vec::new();
            for call in tool_calls {
                let out = self.execute_tool(&call).await?;
                tool_results.push(ChatMessage {
                    role: ChatRole::Tool,
                    content: MessageContent::ToolResult(out),
                    name: Some(call.name),
                });
            }
            current.messages.extend(tool_results);
        }
        Err(AgentError::MaxIterations)
    }

    async fn execute_tool(&self, call: &ToolCall) -> Result<ToolOutput, AgentError> {
        // 1. 查找内置工具
        if let Some(tool) = self.tools.iter().find(|t| t.name() == call.name) {
            return tool.execute(call.arguments.clone()).await;
        }
        // 2. 查找 MCP 工具
        mcp_runtime::call_tool(call.name, call.arguments).await
    }
}
```

Provider 适配器实现（Rig 原生 Provider → ADK Trait）：

```rust
// core/rig/provider.rs — 用 Rig 的 Client 实现 ADK ModelProvider
pub struct RigProviderAdapter {
    pub id: String,                        // "openai:gpt-4o"
    pub inner: Arc<RigClient>,             // rig::providers 客户端
}

#[async_trait]
impl ModelProvider for RigProviderAdapter {
    async fn stream(&self, req: GenerationRequest) -> Result<StreamHandle, AgentError> {
        // rig::completion::Prompt → 转换成异步 Stream
        // 通过 tokio::sync::mpsc channel 桥接 Rig 的回调式流
    }
}
```

### 3.4 AutoAgents 编排层（多 Agent 协作）

职责：管理多个 Agent 实例的创建、调度、协作与工作流执行。基于 **Actor 模型**：每个 Agent 是一个独立 Actor，通过消息传递通信。

```rust
// core/autoagents/actor.rs — Actor 抽象
pub trait AgentActor: Send + Sync {
    fn actor_id(&self) -> ActorId;
    fn role(&self) -> &str;                 // 如 "researcher" / "writer" / "reviewer"
    /// 接收一个任务消息，返回处理结果
    async fn handle(&self, msg: ActorMessage) -> Result<ActorReply, AgentError>;
}

// core/autoagents/coordinator.rs — 协调器
pub struct Coordinator {
    pub registry: HashMap<ActorId, Box<dyn AgentActor>>,
    pub scheduler: TaskScheduler,
}

impl Coordinator {
    /// 注册一个 Agent Actor
    pub fn register(&mut self, actor: Box<dyn AgentActor>) { ... }

    /// 派发任务到指定角色
    pub async fn dispatch(&self, role: &str, task: AgentTask) -> Result<AgentTaskResult, AgentError> { ... }

    /// 执行预定义工作流
    pub async fn run_workflow(&self, wf: &Workflow) -> Result<WorkflowResult, AgentError> { ... }
}

// core/autoagents/workflow.rs — 工作流引擎
#[derive(Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub stages: Vec<WorkflowStage>,     // 顺序阶段
}

pub struct WorkflowStage {
    pub id: String,
    pub role: String,                   // 由哪个角色 Actor 执行
    pub prompt_template: String,        // 模板（可引用前一阶段输出）
    pub depends_on: Vec<String>,        // 依赖阶段 ID
    pub tools: Vec<String>,             // 允许使用的工具白名单
}
```

**内置工作流示例 — 深度研究**：

```
stage1: researcher  → 搜索资料（web_search / knowledge_lookup）→ 输出研究报告
stage2: analyst     → 基于报告 + 横纵对比 → 输出分析结论
stage3: writer      → 生成最终文档（markdown）→ 输出成品
```

**内置工作流示例 — Compose-Next 开发管线**（参考 MiMo-Code compose workflow）：

```
stage1: orient     → 扫描仓库/指令/最近变更，决定工作形状 → 输出上下文摘要
stage2: design     → 应用 compose:plan 或 compose:brainstorm → 输出设计文档（spec）
stage3: implement  → 按依赖序执行 tasks，并行任务分发 → 输出代码变更
stage4: verify     → 运行测试/typecheck/build → 输出验证报告
stage5: review     → 分派 critic agent 审查 → 输出评审意见
stage6: report     → 生成最终报告（journey log + 验证证据）→ 输出交付文档
```

执行流程：Coordinator 按 `depends_on` 拓扑排序 → 逐阶段派发 → 每个阶段结果写入 `WorkflowResult.stage_outputs` → 下一阶段模板可引用 `{{stage1.output}}`。

### 3.5 层间交互时序图

```
Svelte 前端                    Rust 后端
    │  invoke('workflow:run', {workflow_id, inputs})
    │───────────────────────────────▶ Coordinator
    │                                    │ 解析工作流拓扑
    │                                    ├─▶ dispatch(role="researcher", task)
    │                                    │     └─▶ RigAgent.run()
    │                                    │          └─▶ ModelProvider.stream()
    │                                    │                └─▶ LLM API
    │                                    │          ◀── tool_call
    │                                    │          ├─▶ ToolExecutor.execute()
    │                                    │          ◀── tool_result
    │                                    │          └─▶ 继续生成 → finish
    │  emit('workflow:stage', {...})  ◀──┤
    │  emit('workflow:stage', {...})  ◀──┤  (每阶段完成时推送)
    │  emit('workflow:done', result) ◀───┘
```

---

## 4. 完整目录结构

### 4.1 Rust 后端 (src-tauri/)

```
src-tauri/
├── Cargo.toml
├── build.rs
├── tauri.conf.json
├── capabilities/default.json          # Tauri 权限
├── icons/                             # 应用图标
└── src/
    ├── main.rs                        # 入口：初始化 logger、注册命令、启动
    ├── lib.rs                         # Tauri Builder 组装
    ├── commands/                      # IPC 命令层（薄，仅做参数校验 + 调用服务）
    │   ├── mod.rs
    │   ├── agent.rs                   # agent:list/create/update/delete/get/stats
    │   ├── session.rs                 # session:list/create/delete/rename/history
    │   ├── chat.rs                    # chat:send/abort/regenerate/history
    │   ├── model.rs                   # model:list/get-config/set-default/test
    │   ├── mcp.rs                     # mcp:list/add/update/remove/test/tools/call-tool
    │   ├── skill.rs                   # skill:list/install/uninstall/toggle/search-market/list-local
    │   ├── wiki.rs                    # wiki:create/list/delete/read-page/write-page/list-pages/write-ai/ingest-ai/apply-plan/restore-trash
    │   ├── rag.rs                     # rag:ingest/list-documents/delete-document/search
    │   ├── meeting.rs                 # meeting:create/list/get/delete/start-recording/stop-recording/pause/resume/cancel/retranscribe/clean/summary/qa/push-to-agent/export
    │   ├── asr.rs                     # asr:backends/model-catalog/model-installed/model-download/model-remove/test
    │   ├── file.rs                    # file:pick/read-text/write/list/parse
    │   ├── translate.rs               # translate:translate/batch/file/history/detect
    │   ├── glossary.rs                # glossary:list/add/remove/import-csv
    │   ├── ocr.rs                    # ocr:recognize/providers
    │   ├── workflow.rs                # workflow:run/list/get/stop/result
    │   ├── task.rs                    # task:save-template/run/validate/rerun
    │   ├── memory.rs                  # memory:search/read/write/append-notes/reconcile/context-dump
    │   ├── dashboard.rs               # dashboard:overview/usage:stats/usage:trend/mcp:status-all
    │   ├── workspace.rs               # workspace:get/set/instructions/write-instructions/tree/read-file/open-file
    │   ├── lsp.rs                     # lsp:list/diagnostics/start/stop/detect
    │   ├── context.rs                 # context:agent
    │   ├── settings.rs                # settings:get/set/providers/save-provider-key
    │   └── system.rs                  # system:info/open-external
    ├── core/                          # 三层 Agent 框架
    │   ├── mod.rs
    │   ├── adk/
    │   │   ├── mod.rs
    │   │   ├── model.rs               # ModelProvider Trait + 数据结构
    │   │   ├── tool.rs                # ToolExecutor Trait + ToolSpec/ToolOutput
    │   │   ├── memory.rs              # MemoryStore Trait + MemoryItem
    │   │   ├── prompt.rs              # PromptBuilder（技能注入/记忆注入/上下文组装）
    │   │   └── error.rs               # AgentError 统一错误
    │   ├── rig/
    │   │   ├── mod.rs
    │   │   ├── agent.rs               # RigAgent（agentic loop）
    │   │   ├── provider.rs            # Provider 工厂：OpenAI/Anthropic/Google/MiMo/DashScope/Ollama
    │   │   ├── provider/              # 各 Provider 适配器实现
    │   │   │   ├── mod.rs
    │   │   │   ├── openai.rs
    │   │   │   ├── anthropic.rs
    │   │   │   ├── google.rs
    │   │   │   ├── mimo.rs
    │   │   │   ├── dashscope.rs
    │   │   │   └── ollama.rs
    │   │   ├── stream.rs              # StreamPipeline（Token 汇聚、事件发射）
    │   │   └── tools.rs               # 内置工具：file/knowledge/web/translate
    │   └── autoagents/
    │       ├── mod.rs
    │       ├── actor.rs               # AgentActor Trait
    │       ├── coordinator.rs         # Coordinator + ActorRegistry
    │       ├── scheduler.rs           # TaskScheduler（tokio 任务池）
    │       └── workflow.rs            # Workflow 引擎
    ├── mcp/
    │   ├── mod.rs
    │   ├── runtime.rs                 # McpRuntime：连接管理、工具调用、状态
    │   ├── catalog.rs                 # McpCatalog：工具缓存、刷新
    │   ├── transport.rs               # 传输层封装（stdio/SSE/HTTP）
    │   ├── server_log.rs              # 服务器日志缓冲
    │   └── oauth.rs                   # OAuth 回调支持（远程 MCP）
    ├── data/
    │   ├── mod.rs
    │   ├── db.rs                      # DatabasePool（sqlx SqlitePool + 迁移）
    │   ├── migrations/                # 001_init / 002_rag / 003_meeting / 004_workflow / 005_glossary / 006_memory / 007_workflow_templates / 008_agent_traces / 009_message_search / 010_indexes / 011_asr
    │   ├── models.rs                  # 全部数据模型（AgentRow/SessionRow/...）
    │   ├── cache.rs                   # LruCache 服务
    │   ├── rag/
    │   │   ├── mod.rs
    │   │   ├── chunker.rs             # 分块（按段落/句子/固定窗口）
    │   │   ├── embedding.rs           # 嵌入（本地/API 两种模式）
    │   │   ├── store.rs               # 向量存储（SQLite JSON + 余弦计算）
    │   │   └── searcher.rs            # 检索（向量 + 关键词混合）
    │   └── services/                  # 业务服务（持 DB 句柄，被 commands 调用）
    │       ├── mod.rs
    │       ├── agent_service.rs
    │       ├── session_service.rs
    │       ├── chat_service.rs
    │       ├── model_service.rs
    │       ├── skill_service.rs
    │       ├── wiki_service.rs
    │       ├── rag_service.rs
    │       ├── meeting_service.rs
    │       ├── meeting/                 # 会议子模块
    │       │   ├── mod.rs
    │       │   ├── audio_stream.rs      # AudioStreamManager（双写 + ASR 转发 + pending 时序缓冲）
    │       │   ├── export.rs            # 导出（Markdown/DOCX/TXT）
    │       │   └── push_to_agent.rs     # 会议推送 Agent
    │       ├── asr/                     # ASR 可插拔层
    │       │   ├── mod.rs               # AsrBackend Trait + 工厂 + AsrEventSink
    │       │   ├── dashscope_funasr.rs  # DashScope FunASR Realtime（WS）
    │       │   ├── mimo_http.rs         # MiMo ASR（HTTP OpenAI 兼容）
    │       │   ├── sherpa_onnx.rs       # 本地 sherpa-onnx（SenseVoice/Paraformer/Whisper）
    │       │   ├── local_funasr_ws.rs   # 本地 FunASR WebSocket 服务
    │       │   ├── whisper_api.rs       # Whisper API（分片上传）
    │       │   ├── vosk.rs              # 本地 Vosk
    │       │   ├── azure_speech.rs      # Azure Speech（可选）
    │       │   ├── custom_openai.rs     # 自定义 OpenAI 兼容端点
    │       │   └── model_manager.rs     # ASR 模型下载/管理（断点续传 + 校验）
    │       ├── file_service.rs
    │       ├── translate_service.rs
    │       ├── ocr_service.rs
    │       ├── glossary_service.rs      # 翻译术语表
    │       ├── memory_service.rs        # 记忆服务（FTS 索引/搜索/注入/recall）
    │       ├── workflow_service.rs
    │       └── settings_service.rs
    └── utils/
        ├── mod.rs
        ├── error.rs                   # AppError + 序列化
        ├── logger.rs                  # tracing 初始化
        ├── paths.rs                   # app_data_dir / wiki_dir / skill_dir
        ├── crypto.rs                  # AES-GCM 加密 API Key
        └── result.rs                  # TauriCommandResult 包装
```

### 4.2 Svelte 5 前端 (src/)

```
src/
├── app.html
├── app.css                           # 全局样式入口（导入 tokens）
├── lib/
│   ├── design-system/                # Apple Design 系统
│   │   ├── tokens/
│   │   │   ├── colors.ts             # 语义色板（light/dark）
│   │   │   ├── typography.ts         # 字体层级（SF Pro 替代方案）
│   │   │   ├── spacing.ts            # 间距体系（4pt 基准）
│   │   │   ├── radius.ts             # 圆角体系
│   │   │   ├── elevation.ts          # 阴影/层级
│   │   │   └── motion.ts             # 动画曲线与时长
│   │   ├── styles/
│   │   │   ├── reset.css
│   │   │   ├── semantic.css          # 语义别名 CSS 变量（--color-*，§9.1）
│   │   │   ├── tokens.css            # CSS 变量（.light / .dark）
│   │   │   ├── glass.css             # 毛玻璃工具类
│   │   │   └── utilities.css         # 布局/间距工具类
│   │   └── index.ts
│   ├── components/
│   │   ├── base/                     # 基础原子组件
│   │   │   ├── Button.svelte         # 样式变体: primary/secondary/text/gray/destructive（见 §9.7）
│   │   │   ├── IconButton.svelte
│   │   │   ├── Input.svelte
│   │   │   ├── Textarea.svelte
│   │   │   ├── Select.svelte
│   │   │   ├── Switch.svelte         # iOS 风格开关
│   │   │   ├── Slider.svelte
│   │   │   ├── Checkbox.svelte
│   │   │   ├── Modal.svelte          # 毛玻璃弹窗
│   │   │   ├── Sheet.svelte          # 底部抽屉（iOS 风格）
│   │   │   ├── Toast.svelte
│   │   │   ├── Tooltip.svelte
│   │   │   ├── Popover.svelte
│   │   │   ├── Tabs.svelte
│   │   │   ├── Badge.svelte
│   │   │   ├── Avatar.svelte
│   │   │   ├── Progress.svelte
│   │   │   ├── Skeleton.svelte
│   │   │   └── EmptyState.svelte
│   │   ├── layout/                   # Codex 风格布局组件
│   │   │   ├── AppShell.svelte       # 三栏主框架
│   │   │   ├── SideNav.svelte        # 左侧导航（图标 + 会话列表）
│   │   │   ├── ContentArea.svelte    # 中央内容区
│   │   │   ├── RightPanel.svelte     # 右侧工具面板（可折叠）
│   │   │   ├── StatusBar.svelte      # 底部状态栏
│   │   │   ├── Splitter.svelte       # 可拖拽分隔条
│   │   │   └── CommandPalette.svelte # ⌘K 命令面板
│   │   ├── dashboard/                # 主页面板（Home Dashboard）
│   │   │   ├── HomePage.svelte       # 主页面板容器（Agent 功能区 + 状态区 + 任务区）
│   │   │   ├── AgentLauncher.svelte  # 多 Agent 调用入口卡片网格
│   │   │   ├── AgentLauncherCard.svelte # 单个 Agent 卡片（头像/描述/状态/启动）
│   │   │   ├── UsageStats.svelte     # 用量统计卡（今日/本周/本月 token、费用）
│   │   │   ├── UsageChart.svelte     # 用量趋势图（轻量 SVG）
│   │   │   ├── SkillOverview.svelte  # 技能总览卡（已启用/总数/快捷开关）
│   │   │   ├── McpOverview.svelte    # MCP 服务器状态卡（连接/错误/工具数）
│   │   │   ├── RecentSessions.svelte # 最近会话列表
│   │   │   ├── ModelStatus.svelte    # 模型/Provider 状态卡
│   │   │   ├── WorkflowLauncher.svelte # 预置工作流快捷入口
│   │   │   ├── StatCard.svelte       # 通用统计卡片（图标+数值+趋势）
│   │   │   ├── task/                 # 多 Agent 任务设计区（子目录）
│   │   │   │   ├── TaskBoard.svelte        # 任务看板容器（列表/画布视图切换）
│   │   │   │   ├── TaskDesigner.svelte     # 任务设计画布（拖拽编排阶段）
│   │   │   │   ├── TaskStageNode.svelte    # 阶段节点（角色/Agent/提示模板/工具）
│   │   │   │   ├── TaskStageConnector.svelte # 阶段连线（依赖关系）
│   │   │   │   ├── TaskAgentPicker.svelte  # Agent 选择器（拖入画布）
│   │   │   │   ├── TaskPromptEditor.svelte # 阶段提示词模板编辑器（{{stage.x}} 变量）
│   │   │   │   ├── TaskRunnerPanel.svelte  # 任务运行面板（启动/暂停/停止/进度）
│   │   │   │   ├── TaskRunTimeline.svelte  # 任务运行时间线（阶段流 + 状态着色）
│   │   │   │   ├── TaskStageResult.svelte  # 阶段结果查看（输出/工具调用展开）
│   │   │   │   ├── TaskHistory.svelte      # 任务历史列表（结果/重跑/复用）
│   │   │   │   ├── TaskTemplateCard.svelte # 任务模板卡片（预置工作流）
│   │   │   │   ├── TaskNodeInspector.svelte # 节点属性面板（选中节点编辑）
│   │   │   │   └── TaskSaveDialog.svelte   # 保存任务为模板
│   │   │   └── TaskRunner.svelte    # 任务区入口（看板 + 运行器）
│   │   ├── sidebar/                  # Agent 侧边栏（运行时上下文）
│   │   │   ├── AgentSidebar.svelte   # 侧边栏容器（分 Tab）
│   │   │   ├── SidebarUsage.svelte   # 用量 Tab：会话 token/费用/上下文窗口
│   │   │   ├── SidebarWorkdir.svelte # 工作目录 Tab：路径/切换/最近目录
│   │   │   ├── SidebarInstructions.svelte # 指令文件 Tab：CLAUDE.md/AGENTS.md 查看编辑
│   │   │   ├── SidebarMcp.svelte     # MCP Tab：绑定服务器状态/工具列表
│   │   │   ├── SidebarLsp.svelte     # LSP Tab：语言服务器状态/诊断
│   │   │   ├── SidebarFiles.svelte   # 目录树 Tab：文件浏览/打开
│   │   │   └── SidebarTabs.svelte    # 侧边栏 Tab 切换栏
│   │   ├── chat/                     # 对话组件
│   │   │   ├── MessageList.svelte
│   │   │   ├── MessageBubble.svelte  # 支持 Markdown/代码块/工具调用卡片
│   │   │   ├── MarkdownViewer.svelte
│   │   │   ├── CodeBlock.svelte      # 高亮 + 复制
│   │   │   ├── ToolCallCard.svelte   # 工具调用过程展示
│   │   │   ├── ToolApprovalDialog.svelte # 工具审批弹窗（§10.10，风险工具调用确认）
│   │   │   ├── Composer.svelte       # 输入区（多行/Shift+Enter/附件）
│   │   │   ├── ModelSelector.svelte  # 模型切换下拉
│   │   │   └── StopButton.svelte
│   │   ├── agent/                    # Agent 组件
│   │   │   ├── AgentCard.svelte
│   │   │   ├── AgentGrid.svelte
│   │   │   ├── AgentEditor.svelte    # 配置表单（模型/提示词/工具/技能）
│   │   │   └── AgentMarket.svelte
│   │   ├── skill/                    # 技能组件
│   │   │   ├── SkillCard.svelte
│   │   │   ├── SkillMarket.svelte
│   │   │   └── SkillDetail.svelte
│   │   ├── knowledge/                # 知识库组件
│   │   │   ├── WikiList.svelte
│   │   │   ├── WikiEditor.svelte     # Markdown 编辑器 + 预览
│   │   │   ├── WikiTree.svelte       # 页面树
│   │   │   ├── RagManager.svelte     # 文档管理/摄取状态
│   │   │   └── DocumentList.svelte
│   │   ├── meeting/                  # 会议组件
│   │   │   ├── MeetingList.svelte
│   │   │   ├── MeetingSetup.svelte   # 会议配置（标题/参会人/ASR 后端选择）
│   │   │   ├── MeetingRecorder.svelte# 录音 + 实时转写
│   │   │   ├── TranscriptView.svelte # 转写视图（中间结果灰显/最终定稿）
│   │   │   ├── MeetingQA.svelte      # 会议问答
│   │   │   ├── MeetingSummary.svelte # 摘要展示
│   │   │   ├── AsrBackendSelector.svelte # ASR 后端选择器（含健康检查徽标）
│   │   │   ├── AsrModelManager.svelte    # ASR 模型管理（下载/删除/进度）
│   │   │   ├── RetranscribeDialog.svelte # 换模型重新转写
│   │   │   └── MeetingExportDialog.svelte # 导出选项对话框
│   │   ├── files/                    # 文件组件
│   │   │   ├── FileGrid.svelte
│   │   │   └── FilePreview.svelte
│   │   ├── translate/                # 翻译/OCR 组件
│   │   │   ├── TranslatePage.svelte  # 翻译主页（文本/批量/文件）
│   │   │   ├── OcrCard.svelte        # OCR 图片翻译卡（拖拽/预览/识别）
│   │   │   ├── GlossaryManager.svelte# 术语表管理
│   │   │   └── TranslateHistory.svelte # 翻译历史
│   │   ├── memory/                   # 记忆管理组件
│   │   │   ├── MemoryManager.svelte  # 记忆管理容器（全局/项目/会话/搜索 Tab）
│   │   │   ├── MemoryEditor.svelte   # MEMORY.md 可编辑视图
│   │   │   ├── CheckpointViewer.svelte # checkpoint 只读展示
│   │   │   ├── MemorySearch.svelte   # 记忆搜索（结果列表 + 命中高亮）
│   │   │   └── MemoryIndexStatus.svelte # 索引状态/重建按钮
│   │   ├── settings/                 # 设置组件
│   │   │   ├── ProviderSettings.svelte   # Provider API Key 管理
│   │   │   ├── GeneralSettings.svelte
│   │   │   ├── ShortcutSettings.svelte
│   │   │   └── AsrSettings.svelte    # ASR 后端预设管理
│   │   └── common/
│   │       ├── ThemeToggle.svelte
│   │       ├── SearchInput.svelte
│   │       └── ConfirmDialog.svelte
│   ├── routes/                       # SvelteKit 路由页面
│   │   ├── +layout.svelte            # AppShell 包裹
│   │   ├── +page.svelte              # / 主页面板（Home Dashboard）
│   │   ├── chat/[sessionId]/+page.svelte  # /chat/:id 对话页
│   │   ├── agents/+page.svelte
│   │   ├── agents/[id]/+page.svelte
│   │   ├── knowledge/+page.svelte
│   │   ├── knowledge/[wikiId]/+page.svelte
│   │   ├── meetings/+page.svelte
│   │   ├── meetings/[id]/+page.svelte
│   │   ├── skills/+page.svelte
│   │   ├── files/+page.svelte
│   │   └── settings/+page.svelte
│   ├── stores/                       # Runes 状态
│   │   ├── chat.svelte.ts            # 对话状态（消息/流式/正在生成）
│   │   ├── agents.svelte.ts          # Agent 列表/当前选中
│   │   ├── models.svelte.ts          # 模型列表/当前模型
│   │   ├── skills.svelte.ts
│   │   ├── settings.svelte.ts        # 设置/主题
│   │   ├── ui.svelte.ts              # 面板折叠/命令面板/快捷键
│   │   └── toasts.svelte.ts
│   ├── api/                          # IPC 客户端封装
│   │   ├── client.ts                 # invoke 封装（类型安全 + 错误处理）
│   │   ├── agent.ts
│   │   ├── chat.ts
│   │   ├── model.ts
│   │   ├── mcp.ts
│   │   ├── skill.ts
│   │   ├── wiki.ts
│   │   ├── meeting.ts
│   │   ├── translate.ts
│   │   ├── ocr.ts
│   │   ├── workflow.ts
│   │   └── events.ts                 # listen 封装（chat:stream:* 等）
│   ├── hooks/                        # Svelte 组合式逻辑
│   │   ├── use-streaming-chat.ts
│   │   ├── use-keyboard.ts           # 快捷键
│   │   └── use-theme.ts
│   └── utils/
│       ├── markdown.ts               # markdown 渲染（mdast 自定义）
│       ├── formatter.ts              # 时间/大小格式化
│       └── id.ts
├── vite.config.ts
├── svelte.config.js
└── package.json
```

---

## 5. 数据库 Schema 详细设计

数据库：SQLite 单文件 `{app_data}/prism.db`，使用 sqlx 异步连接池。所有表 ID 为 UUID v4 文本，时间戳为 INTEGER（毫秒）。

### 5.1 表结构总览

| 表 | 说明 | 关系 |
|----|------|------|
| `providers` | LLM Provider 配置（含加密 API Key） | 1:N models |
| `models` | 模型注册表 | N:1 providers |
| `agents` | Agent 定义 | 1:N sessions, N:N skills, N:N mcp_servers |
| `agent_mcp_servers` | Agent × MCP 关联 | N:N junction |
| `agent_skills` | Agent × 技能关联 | N:N junction |
| `sessions` | 会话 | N:1 agents, 1:N messages |
| `messages` | 消息（含工具调用） | N:1 sessions |
| `skills` | 技能元数据 | N:N agents |
| `mcp_servers` | MCP 服务器配置 | N:N agents |
| `wikis` | Wiki 知识库 | 1:N rag_documents |
| `rag_documents` | RAG 文档 | N:1 wikis, 1:N rag_chunks |
| `rag_chunks` | 分块（含向量） | N:1 rag_documents |
| `meetings` | 会议 | 1:N meeting_transcripts |
| `meeting_transcripts` | 转写片段 | N:1 meetings |
| `asr_configs` | ASR 后端配置（§10.3.8，phase3-extend.md） | - |
| `workflows` | 工作流定义 | 1:N workflow_runs |
| `workflow_runs` | 工作流运行 | N:1 workflows |
| `stage_templates` | 可复用阶段模板（§10.6.4） | - |
| `agent_traces` | Agent 执行轨迹（§10.13.1，phase3-extend.md） | N:1 sessions |
| `translate_history` | 翻译历史 | - |
| `glossary_terms` | 翻译术语表（§10.5.2，phase3-extend.md） | - |
| `preferences` | 键值偏好设置 | - |
| `memory_fts` | 记忆全文索引（FTS5，§10.7.2） | 虚拟表 |
| `messages_fts` | 消息全文索引（FTS5，§5.7.2，迁移 009） | 虚拟表 |
| `sessions_fts` | 会话标题索引（FTS5，§5.7.4，phase2-panel.md，迁移 012） | 虚拟表 |
| `translate_fts` | 翻译历史索引（FTS5，§5.7.5，phase3-extend.md，迁移 013） | 虚拟表 |

### 5.2 DDL（迁移 001_init.sql）

```sql
-- Provider 配置
CREATE TABLE providers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,                -- 显示名
    kind        TEXT NOT NULL,                -- openai|anthropic|google|mimo|dashscope|ollama|custom
    base_url    TEXT,
    api_key_enc TEXT,                         -- AES-GCM 加密后密文
    is_enabled  INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- 模型注册表
CREATE TABLE models (
    id           TEXT PRIMARY KEY,
    provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id     TEXT NOT NULL,               -- 供应商侧模型 ID
    display_name TEXT,
    kind         TEXT NOT NULL DEFAULT 'chat',-- chat|embedding|vision|asr
    max_tokens   INTEGER DEFAULT 8192,
    is_default   INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    UNIQUE (provider_id, model_id)
);

-- Agent 定义
CREATE TABLE agents (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    description    TEXT,
    avatar         TEXT,
    system_prompt  TEXT,
    model_id       TEXT REFERENCES models(id),
    plan_model_id  TEXT REFERENCES models(id),
    small_model_id TEXT REFERENCES models(id),
    temperature    REAL DEFAULT 0.7,
    max_tokens     INTEGER DEFAULT 8192,
    disabled_tools TEXT NOT NULL DEFAULT '[]',   -- JSON 数组
    configuration  TEXT NOT NULL DEFAULT '{}',   -- JSON（知识库绑定等）
    order_key      INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
);

-- Agent × MCP 关联
CREATE TABLE agent_mcp_servers (
    agent_id      TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    mcp_server_id TEXT NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    created_at    INTEGER NOT NULL,
    PRIMARY KEY (agent_id, mcp_server_id)
);

-- Agent × 技能关联
CREATE TABLE agent_skills (
    agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    skill_id   TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (agent_id, skill_id)
);

-- 会话
CREATE TABLE sessions (
    id           TEXT PRIMARY KEY,
    agent_id     TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    title        TEXT,
    pinned       INTEGER NOT NULL DEFAULT 0,
    order_key    INTEGER NOT NULL DEFAULT 0,       -- 会话列表排序（§1 新会话创建）
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);
CREATE INDEX idx_sessions_agent ON sessions(agent_id, updated_at DESC);

-- 消息
CREATE TABLE messages (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role         TEXT NOT NULL,                -- system|user|assistant|tool
    content      TEXT NOT NULL,                -- 文本（assistant 可为空，仅 tool_calls）
    tool_calls   TEXT,                         -- JSON: [{id,name,arguments}]
    tool_call_id TEXT,                         -- 关联 tool 角色消息
    model_id     TEXT,
    usage        TEXT,                         -- JSON: {prompt_tokens,completion_tokens}
    created_at   INTEGER NOT NULL
);
CREATE INDEX idx_messages_session ON messages(session_id, created_at);

-- 技能元数据
CREATE TABLE skills (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT,
    folder_name  TEXT NOT NULL UNIQUE,         -- 磁盘目录名
    source       TEXT NOT NULL,                -- builtin|marketplace|local|zip
    source_url   TEXT,
    namespace    TEXT,
    author       TEXT,
    tags         TEXT NOT NULL DEFAULT '[]',
    content_hash TEXT NOT NULL,
    is_enabled   INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

-- MCP 服务器
CREATE TABLE mcp_servers (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    type       TEXT NOT NULL,                  -- stdio|sse|http|inmemory
    command    TEXT,
    args       TEXT NOT NULL DEFAULT '[]',
    env        TEXT NOT NULL DEFAULT '{}',
    base_url   TEXT,
    headers    TEXT NOT NULL DEFAULT '{}',
    is_active  INTEGER NOT NULL DEFAULT 1,
    timeout_ms INTEGER DEFAULT 30000,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

### 5.3 迁移 002_rag.sql

```sql
CREATE TABLE wikis (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    schema      TEXT,                          -- SCHEMA.md 内容
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE rag_documents (
    id          TEXT PRIMARY KEY,
    wiki_id     TEXT NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    size        INTEGER NOT NULL DEFAULT 0,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'pending', -- pending|chunking|embedding|ready|error
    error_msg   TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX idx_rag_docs_wiki ON rag_documents(wiki_id);

CREATE TABLE rag_chunks (
    id          TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES rag_documents(id) ON DELETE CASCADE,
    wiki_id     TEXT NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    "index"     INTEGER NOT NULL,
    content     TEXT NOT NULL,
    embedding   BLOB,                          -- f32 小端打包
    created_at  INTEGER NOT NULL
);
CREATE INDEX idx_rag_chunks_doc ON rag_chunks(document_id);
CREATE INDEX idx_rag_chunks_wiki ON rag_chunks(wiki_id);
```

### 5.4 迁移 003_meeting.sql

```sql
CREATE TABLE meetings (
    id                 TEXT PRIMARY KEY,
    title              TEXT NOT NULL,
    date               TEXT NOT NULL,
    transcript         TEXT NOT NULL DEFAULT '',
    summary            TEXT NOT NULL DEFAULT '',
    participants       TEXT NOT NULL DEFAULT '[]',
    recording_duration INTEGER NOT NULL DEFAULT 0,
    audio_path         TEXT,
    folder_path        TEXT,
    source_lang        TEXT,                   -- ASR 检测语言
    target_lang        TEXT,                   -- 实时翻译目标语言（可选）
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL
);

CREATE TABLE meeting_transcripts (
    id         TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    "index"    INTEGER NOT NULL,
    text       TEXT NOT NULL,
    is_final   INTEGER NOT NULL DEFAULT 0,
    translated TEXT,
    created_at INTEGER NOT NULL
);
```

### 5.5 迁移 004_workflow.sql

```sql
CREATE TABLE workflows (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    definition  TEXT NOT NULL,                 -- JSON: {stages:[...]}
    source      TEXT NOT NULL DEFAULT 'builtin', -- builtin|user（§10.6.4）
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE workflow_runs (
    id          TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    status      TEXT NOT NULL DEFAULT 'running', -- running|done|failed|cancelled
    source      TEXT NOT NULL DEFAULT 'workflow',-- workflow|task（§9.9.1 自定义任务，phase2-panel.md）
    inputs      TEXT NOT NULL DEFAULT '{}',
    outputs     TEXT,                          -- JSON: {stage_id: output}
    error       TEXT,
    created_at  INTEGER NOT NULL,
    finished_at INTEGER
);

CREATE TABLE translate_history (
    id           TEXT PRIMARY KEY,
    source_text  TEXT NOT NULL,
    source_lang  TEXT NOT NULL,
    target_lang  TEXT NOT NULL,
    translated   TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

CREATE TABLE preferences (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,                  -- JSON 值
    updated_at INTEGER NOT NULL
);
```

### 5.6 sqlx 使用模式

```rust
// data/db.rs
pub struct Database { pool: SqlitePool }

impl Database {
    pub async fn new(app_data_dir: &Path) -> Result<Self, AppError> {
        let url = format!("sqlite://{}", app_data_dir.join("prism.db").display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url).await?;
        sqlx::migrate!("./src/data/migrations").run(&pool).await?;
        // SQLite PRAGMA 优化（§5.7）
        Self::optimize_pragmas(&pool).await?;
        Ok(Self { pool })
    }

    async fn optimize_pragmas(pool: &SqlitePool) -> Result<(), AppError> {
        sqlx::query("PRAGMA journal_mode=WAL").execute(pool).await?;     // 并发读写
        sqlx::query("PRAGMA synchronous=NORMAL").execute(pool).await?;   // 性能 vs 安全平衡
        sqlx::query("PRAGMA foreign_keys=ON").execute(pool).await?;      // FK 约束
        sqlx::query("PRAGMA cache_size=-8000").execute(pool).await?;     // 8MB 页缓存
        sqlx::query("PRAGMA temp_store=MEMORY").execute(pool).await?;    // 临时表在内存
        sqlx::query("PRAGMA mmap_size=268435456").execute(pool).await?;  // 256MB mmap
        sqlx::query("PRAGMA page_size=4096").execute(pool).await?;       // 匹配 OS 页大小（须在建库前设置）
        sqlx::query("PRAGMA busy_timeout=5000").execute(pool).await?;    // 写锁等待 5s（防忙等）
        Ok(())
    }
}
```

### 5.7 数据存储性能设计（跨阶段基础，建库即做）

**目标**：10 万+ 消息、1000+ 会话时保持流畅响应（搜索 < 200ms，列表加载 < 100ms）。

> **设计归属**：数据存储是**横切关注点**，每个阶段实现对应功能时同步落地，避免后期返工：
> - 🟦 Phase 1（本节）：PRAGMA 优化、消息全文搜索、游标分页、数据保留策略、关键索引、降级边界
> - 🟧 Phase 2：会话标题搜索（sessions_fts）→ 见 phase2-panel.md §5.7.4
> - 🟩 Phase 3：翻译历史搜索（translate_fts）→ 见 phase3-extend.md §5.7.5
> - 各阶段新增表/索引时，必须同步更新本节「5.7.7 关键索引」与迁移编号，禁止在已有迁移上追加（§14.3 #28）。

#### 5.7.1 SQLite PRAGMA 优化（Phase 1 建库即执行）

```sql
-- 性能关键 PRAGMA（数据库创建时执行）
PRAGMA journal_mode = WAL;        -- Write-Ahead Logging：并发读写，不阻塞
PRAGMA synchronous = NORMAL;      -- WAL 模式下 NORMAL 即可（非 FULL）
PRAGMA foreign_keys = ON;         -- 启用 FK 约束
PRAGMA cache_size = -8000;        -- 8MB 页缓存（默认 2MB 太小）
PRAGMA temp_store = MEMORY;       -- 临时表/索引在内存
PRAGMA mmap_size = 268435456;     -- 256MB mmap 映射（减少 read I/O）
PRAGMA page_size = 4096;          -- 匹配 OS 页大小
PRAGMA busy_timeout = 5000;       -- 写锁等待 5s（防忙等）
```

#### 5.7.2 消息全文搜索（FTS5，Phase 1 对话闭环即用）

```sql
-- 迁移 009_message_search.sql
-- 消息 FTS5 虚拟表（覆盖 session 内搜索 + 跨会话全局搜索）
CREATE VIRTUAL TABLE messages_fts USING fts5(
    content,                       -- 消息文本内容
    session_id UNINDEXED,          -- 会话 ID（不索引，仅过滤）
    role UNINDEXED,                -- 角色（不索引，仅过滤）
    created_at UNINDEXED,          -- 时间戳（不索引，仅过滤）
    content='messages',            -- 关联实体表
    content_rowid='rowid',         -- 行 ID 映射
    tokenize='unicode61'           -- Unicode 分词（CJK 安全）
);

-- 同步触发器：消息写入/更新/删除时自动维护 FTS
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content, session_id, role, created_at)
    VALUES (new.rowid, new.content, new.session_id, new.role, new.created_at);
END;

CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content, session_id, role, created_at)
    VALUES ('delete', old.rowid, old.content, old.session_id, old.role, old.created_at);
END;

CREATE TRIGGER messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content, session_id, role, created_at)
    VALUES ('delete', old.rowid, old.content, old.session_id, old.role, old.created_at);
    INSERT INTO messages_fts(rowid, content, session_id, role, created_at)
    VALUES (new.rowid, new.content, new.session_id, new.role, new.created_at);
END;
```

**搜索实现**：

```rust
/// 消息全文搜索（支持会话内搜索 + 跨会话全局搜索）
pub async fn search_messages(
    &self, query: &str, session_id: Option<&str>, limit: usize,
) -> Result<Vec<MessageSearchHit>, AppError> {
    let tokens = tokenize_fts(query);  // 按 Unicode 切分，OR 连接
    let match_expr = tokens.iter()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");

    let (sql, params): (String, Vec<String>) = if let Some(sid) = session_id {
        // 会话内搜索
        (
            "SELECT m.id, m.session_id, m.role, m.content, m.created_at,
                    snippet(messages_fts, 0, '<<', '>>', '...', 32) as snippet,
                    bm25(messages_fts) as score
             FROM messages_fts f
             JOIN messages m ON m.rowid = f.rowid
             WHERE messages_fts MATCH ? AND f.session_id = ?
             ORDER BY score
             LIMIT ?".into(),
            vec![match_expr, sid.to_string(), limit.to_string()],
        )
    } else {
        // 全局搜索
        (
            "SELECT m.id, m.session_id, m.role, m.content, m.created_at,
                    snippet(messages_fts, 0, '<<', '>>', '...', 32) as snippet,
                    bm25(messages_fts) as score
             FROM messages_fts f
             JOIN messages m ON m.rowid = f.rowid
             WHERE messages_fts MATCH ?
             ORDER BY score
             LIMIT ?".into(),
            vec![match_expr, limit.to_string()],
        )
    };

    let rows = sqlx::query_as::<_, MessageSearchHit>(&sql)
        .bind(params[0].clone())
        // ... bind remaining params
        .fetch_all(&self.pool).await?;
    Ok(rows)
}
```

#### 5.7.3 消息分页加载（Phase 1 消息列表即用）

```rust
/// 游标分页（避免 OFFSET 扫描，大数据量下 O(n) → O(log n)）
pub async fn list_messages_cursor(
    &self, session_id: &str, before: Option<&str>, limit: usize,
) -> Result<Vec<MessageRow>, AppError> {
    match before {
        Some(cursor_id) => {
            // 游标分页：找到 cursor 的 created_at，取其之前的 limit 条
            sqlx::query_as::<_, MessageRow>(
                "SELECT * FROM messages
                 WHERE session_id = ? AND created_at < (
                     SELECT created_at FROM messages WHERE id = ?
                 )
                 ORDER BY created_at DESC
                 LIMIT ?"
            )
            .bind(session_id).bind(cursor_id).bind(limit as i64)
            .fetch_all(&self.pool).await?
        }
        None => {
            // 首页：取最新的 limit 条
            sqlx::query_as::<_, MessageRow>(
                "SELECT * FROM messages
                 WHERE session_id = ?
                 ORDER BY created_at DESC
                 LIMIT ?"
            )
            .bind(session_id).bind(limit as i64)
            .fetch_all(&self.pool).await?
        }
    }
}
```

**前端虚拟滚动**（配合游标分页）：

```svelte
<!-- MessageList.svelte — 虚拟滚动 + 按需加载 -->
<script lang="ts">
    import { virtual } from '@tanstack/svelte-virtual';

    let messages = $state<Message[]>([]);
    let loading = $state(false);
    let hasMore = $state(true);

    // 滚动到顶部时加载更多历史消息
    async function loadMore() {
        if (loading || !hasMore) return;
        loading = true;
        const cursor = messages[0]?.id;
        const older = await api.chat.history(sessionId, { before: cursor, limit: 50 });
        if (older.length < 50) hasMore = false;
        messages = [...older, ...messages];
        loading = false;
    }
</script>
```

#### 5.7.6 数据保留策略（Phase 1 定义策略，后台定时任务）

```rust
/// 数据清理（后台定时任务，默认每周运行）
pub async fn cleanup_old_data(&self, config: &CleanupConfig) -> Result<CleanupResult, AppError> {
    let now = chrono::Utc::now().timestamp_millis();

    // 1. 清理过期消息（超过保留期的旧消息删除；置顶会话的消息受保护）
    //    注：当前策略为直接删除（仅保留最新 N 天）；「摘要归档」为可选增强
    //    （由记忆系统 checkpoint 机制承担会话级摘要，见 §10.7.3），默认关闭。
    let msg_cutoff = now - (config.keep_messages_days * 86400_000) as i64;
    let archived = sqlx::query(
        "DELETE FROM messages WHERE created_at < ? AND id NOT IN (
            SELECT id FROM messages WHERE session_id IN (
                SELECT id FROM sessions WHERE pinned = 1
            )
        )"
    ).bind(msg_cutoff).execute(&self.pool).await?.rows_affected();

    // 2. 清理已完成的工作流运行（保留最近 N 条）
    let wf_cutoff = now - (config.keep_workflow_runs_days * 86400_000) as i64;
    let wf_archived = sqlx::query(
        "DELETE FROM workflow_runs WHERE created_at < ? AND status IN ('done', 'failed', 'cancelled')"
    ).bind(wf_cutoff).execute(&self.pool).await?.rows_affected();

    // 3. 清理空会话（无消息的会话，创建超过 7 天）
    let empty_cutoff = now - (7 * 86400_000) as i64;
    let empty_archived = sqlx::query(
        "DELETE FROM sessions WHERE id NOT IN (SELECT DISTINCT session_id FROM messages)
         AND created_at < ? AND pinned = 0"
    ).bind(empty_cutoff).execute(&self.pool).await?.rows_affected();

    // 4. VACUUM（碎片整理，每月一次）
    if config.should_vacuum() {
        sqlx::query("VACUUM").execute(&self.pool).await?;
    }

    // 5. FTS 索引重建（碎片率 > 30% 时）
    if config.should_rebuild_fts() {
        sqlx::query("INSERT INTO messages_fts(messages_fts) VALUES('rebuild')").execute(&self.pool).await?;
    }

    Ok(CleanupResult { archived, wf_archived, empty_archived })
}

pub struct CleanupConfig {
    pub keep_messages_days: u32,        // 默认 365 天
    pub keep_workflow_runs_days: u32,   // 默认 90 天
    pub vacuum_interval_days: u32,      // 默认 30 天
    pub rebuild_fts_threshold: f64,     // 碎片率阈值，默认 0.3
}
```

#### 5.7.7 查询性能关键索引（Phase 1 建表即建）

```sql
-- 高频查询索引（迁移 010_indexes.sql）
-- 注：以下索引已在 001_init.sql / 002_rag.sql 定义，此处不重复：
--   idx_messages_session (session_id, created_at)     ← 001
--   idx_sessions_agent (agent_id, updated_at DESC)    ← 001
--   idx_rag_chunks_wiki (wiki_id)                     ← 002
-- 本迁移仅新增以下索引：

-- 消息查询：按 ID 查找（游标分页的 cursor lookup）
CREATE INDEX idx_messages_id ON messages(id);

-- 会话查询：按 pinned + 更新时间（置顶会话优先）
CREATE INDEX idx_sessions_pinned_updated ON sessions(pinned DESC, updated_at DESC);

-- 工作流运行：按状态 + 创建时间（后台任务轮询）
CREATE INDEX idx_workflow_runs_status ON workflow_runs(status, created_at DESC);

-- 翻译历史：按语言对 + 时间（最近翻译查询）
CREATE INDEX idx_translate_lang_time ON translate_history(source_lang, target_lang, created_at DESC);

-- Agent traces 索引在迁移 008_agent_traces.sql 中随表创建（见 phase3-extend.md §10.13.1），此处不重复。
--   008 已建：idx_agent_traces_session (session_id, started_at DESC) / idx_agent_traces_agent (agent_id, started_at DESC)
```

#### 5.7.8 大数据量边界与降级（Phase 1 定义策略）

| 数据量 | 策略 |
|--------|------|
| 消息 < 1 万 | 全量加载无压力 |
| 消息 1~10 万 | 游标分页 + 虚拟滚动（每次 50 条） |
| 消息 > 10 万 | 启用数据保留策略（自动归档旧消息） |
| 单会话消息 > 5000 | PromptBuilder 启用滑动窗口（§13.1），仅注入最近 200 条 |
| RAG chunks > 10 万 | 向量检索改用 HNSW 索引（`sqlite-vss` 扩展） |
| FTS 碎片率 > 30% | 自动 `rebuild` 命令整理索引 |

**性能监控**：

```rust
/// 查询性能监控（DEBUG 模式下记录慢查询）
pub async fn query_with_timing<T>(
    pool: &SqlitePool, sql: &str, slow_threshold_ms: u64,
) -> Result<T, AppError> {
    let start = Instant::now();
    let result = sqlx::query_scalar::<_, T>(sql).fetch_one(pool).await?;
    let elapsed = start.elapsed().as_millis() as u64;
    if elapsed > slow_threshold_ms {
        tracing::warn!(sql = %sql, elapsed_ms = elapsed, "Slow query detected");
    }
    Ok(result)
}
```

---

## 6. MCP 协议集成详细设计

### 6.1 架构

```
┌────────────────────────────────────────────────────────────┐
│                        McpRuntime                          │
│  clients:  HashMap<server_id, McpClient>                   │
│  pending:  HashMap<server_id, JoinHandle>                  │
│  status:   HashMap<server_id, McpStatus>                   │
│  active_calls: HashMap<call_id, CancellationToken>         │
│                                                            │
│   ┌───────────┐  ┌───────────┐  ┌───────────────┐          │
│   │ Stdio     │  │ Sse       │  │ StreamableHTTP │         │
│   │ (tokio    │  │ (reqwest  │  │ (reqwest      │         │
│   │  process) │  │  + SSE)   │  │  streamable)  │         │
│   └─────┬─────┘  └─────┬─────┘  └───────┬───────┘          │
│         │              │                │                  │
│   本地 MCP 进程    远程 SSE 服务器    远程 HTTP 服务器      │
└────────────────────────────────────────────────────────────┘
```

### 6.2 核心数据结构

```rust
// mcp/runtime.rs
pub enum McpStatus { Disconnected, Connecting, Connected, Error(String) }

pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub kind: TransportKind,     // Stdio | Sse | Http
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub base_url: Option<String>,
    pub headers: HashMap<String, String>,
    pub timeout_ms: u64,
}

pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// mcp/transport.rs — 传输抽象
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn initialize(&mut self, client_info: &ClientInfo) -> Result<(), McpError>;
    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError>;
    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<McpCallResult, McpError>;
    async fn list_resources(&self) -> Result<Vec<McpResource>, McpError>;
    async fn list_prompts(&self) -> Result<Vec<McpPrompt>, McpError>;
    async fn close(&mut self) -> Result<(), McpError>;
}
```

### 6.3 传输实现要点

**Stdio**：`tokio::process::Command` spawn 子进程，stdin/stdout 走 JSON-RPC 2.0 行协议；`next_id` 自增，`pending: Mutex<HashMap<u64, oneshot>>` 关联请求与响应；stderr 转发到 `ServerLogBuffer`。

**SSE**：`reqwest` GET 建立 SSE 流（`eventsource-stream` crate 解析），POST 发送请求。

**Streamable HTTP**：POST JSON-RPC 到 `base_url`，`Accept: application/json, text/event-stream`；响应为流时逐事件解析。

### 6.4 依赖选择

优先 `mcp-rust-sdk`（官方）。**回退方案**：若 API 不稳定，基于 `rmcp` crate 封装同样的 `McpTransport` Trait，上层零改动；若两者均不可用，自研轻量 JSON-RPC 客户端（~300 行，仅实现 initialize / tools / call / resources / prompts）。

### 6.5 工具调用与权限

```rust
pub async fn call_tool(
    &self, server_id: &str, tool_name: &str,
    args: serde_json::Value, agent_id: Option<&str>,
) -> Result<McpCallResult, AppError> {
    // 1. Agent 必须绑定该 MCP 服务器
    if let Some(aid) = agent_id {
        if !self.is_agent_bound(aid, server_id).await? {
            return Err(AppError::Forbidden("MCP server not bound to agent".into()));
        }
    }
    // 2. 工具未被 Agent 禁用
    if let Some(aid) = agent_id {
        if self.is_tool_disabled(aid, tool_name).await? {
            return Err(AppError::Forbidden(format!("Tool disabled: {tool_name}")));
        }
    }
    // 3. 获取/建立连接 → 调用 → 返回
    let client = self.connect(server_id).await?;
    Ok(client.call_tool(tool_name, args).await?)
}
```

### 6.6 工具目录缓存

- 启动时对所有 active 服务器 `tools/list` → 内存 LRU 缓存（TTL 1h）
- 监听 `notifications/tools/list_changed` → 失效 → 重新拉取
- 目录供前端工具面板 + RigAgent 工具注入共用

---

## 7. 流式响应架构详细设计

### 7.1 目标

- 首 Token 低延迟（<100ms）
- 支持中断（abort）
- 工具调用过程可视化
- WebView 重建后可恢复

### 7.2 事件模型

```
chat:stream:start      {session_id, message_id, model}
chat:stream:delta      {session_id, message_id, delta}            // 文本增量
chat:stream:tool_call  {session_id, message_id, call:{id,name,args}}
chat:stream:tool_result{session_id, message_id, call_id, ok, output}
chat:stream:done       {session_id, message_id, usage}
chat:stream:error      {session_id, message_id, message}
chat:stream:aborted    {session_id, message_id}
```

### 7.3 后端实现（StreamPipeline）

```rust
// core/rig/stream.rs
pub enum StreamEvent { Text(String), ToolCall(ToolCall), Finish{usage}, Error(String) }

pub struct StreamPipeline { app: AppHandle }

impl StreamPipeline {
    pub async fn run(&self, agent: &RigAgent, request: GenerationRequest,
                     cancel: CancellationToken) -> Result<(), AppError> {
        let mut handle = agent.model_provider.stream(request).await?;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => { emit(aborted); return Ok(()); }
                event = handle.next() => match event {
                    Some(StreamEvent::Text(t))    => emit(delta, t),
                    Some(StreamEvent::ToolCall(c))=> emit(tool_call, c),
                    Some(StreamEvent::Finish{u})  => { emit(done, u); break; }
                    Some(StreamEvent::Error(e))   => { emit(error, e); return Err(e.into()); }
                    None => break,
                }
            }
        }
        Ok(())
    }
}
```

### 7.4 取消机制

```rust
pub struct ChatService {
    active_streams: Mutex<HashMap<String, CancellationToken>>,   // session_id → token
}
pub async fn abort(&self, session_id: &str) -> Result<(), AppError> {
    if let Some(t) = self.active_streams.lock().await.remove(session_id) { t.cancel(); }
    Ok(())
}
```

### 7.5 前端消费

```ts
// lib/api/events.ts
export function onStreamEvents(sessionId: string, handlers: StreamHandlers) {
    const unsubs = [
        listen(`chat:stream:delta`, (e) => { if (e.payload.session_id === sessionId) handlers.onDelta(e.payload.delta); }),
        listen(`chat:stream:tool_call`, (e) => { if (e.payload.session_id === sessionId) handlers.onToolCall(e.payload.call); }),
        listen(`chat:stream:done`, (e) => { if (e.payload.session_id === sessionId) handlers.onDone(e.payload.usage); }),
        listen(`chat:stream:error`, (e) => { if (e.payload.session_id === sessionId) handlers.onError(e.payload.message); }),
    ];
    return () => unsubs.forEach((u) => u());
}
```

---

## 8. Tauri IPC 接口详细设计

### 8.1 通用返回包装

所有命令返回 `Result<T, AppError>`，前端统一封装：

```ts
// lib/api/client.ts
import { invoke as tauriInvoke } from "@tauri-apps/api/core";

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    try {
        return await tauriInvoke<T>(cmd, args);
    } catch (e) {
        toast.error(String(e));   // AppError 已序列化为字符串
        throw e;
    }
}
```

### 8.2 命令清单（按域分组）

**Agent 域** `commands/agent.rs`

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `agent:list` | `{query?, limit?}` | `Vec<AgentDto>` | 列表 |
| `agent:get` | `{id}` | `AgentDto` | 详情 |
| `agent:create` | `{name, description?, avatar?, system_prompt?, model_id?, plan_model_id?, small_model_id?, temperature?, max_tokens?, mcp_server_ids?, skill_ids?, disabled_tools?}` | `AgentDto` | 事务创建 + 关联 |
| `agent:update` | `{id, ...partial}` | `AgentDto` | 更新 |
| `agent:delete` | `{id, delete_sessions?}` | `()` | 删除 |
| `agent:stats` | `{agent_id?, range?}` | `AgentStats` | 成功率/延迟/Token 效率/失败分布（§10.13.3，见 phase3-extend.md） |

**会话域** `commands/session.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `session:list` | `{agent_id?}` | `Vec<SessionDto>` |
| `session:create` | `{agent_id, title?}` | `SessionDto` |
| `session:delete` | `{id}` | `()` |
| `session:rename` | `{id, title}` | `SessionDto` |
| `session:history` | `{id, before?, limit?}` | `Vec<MessageDto>` |
| `session:inject-file` | `{session_id, path}` | `()` | 指令文件注入会话（§9.10.7，phase2） |

**对话域** `commands/chat.rs`

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `chat:send` | `{session_id, content, attachments?}` | `MessageDto` | 立即返回 user 消息，流式走事件 |
| `chat:abort` | `{session_id}` | `()` | 中断当前流 |
| `chat:regenerate` | `{session_id, message_id}` | `()` | 重新生成最后一条助手消息 |
| `chat:history` | `{session_id, before?, limit?}` | `Vec<MessageDto>` | 消息历史（游标分页，§5.7.3） |

**模型域** `commands/model.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `model:list` | `{}` | `Vec<ModelDto>` |
| `model:get-config` | `{model_id}` | `ModelConfig` |
| `model:set-default` | `{model_id}` | `()` |
| `model:test` | `{model_id}` | `{ok, latency_ms, error?}` |

**MCP 域** `commands/mcp.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `mcp:list` | `{}` | `Vec<McpServerDto>` |
| `mcp:add` | `{name, type, command?, args?, env?, base_url?, headers?}` | `McpServerDto` |
| `mcp:update` | `{id, ...}` | `McpServerDto` |
| `mcp:remove` | `{id}` | `()` |
| `mcp:test` | `{id}` | `{ok, tools_count, error?}` |
| `mcp:tools` | `{server_id?}` | `Vec<McpToolDto>` |
| `mcp:call-tool` | `{server_id, tool_name, args, agent_id?}` | `McpCallResult` | 调用 MCP 工具（§6.5 权限校验） |

**技能域** `commands/skill.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `skill:list` | `{}` | `Vec<SkillDto>` |
| `skill:install` | `{source, source_url?}` | `SkillDto` | source: `skills.sh:x` / `github:owner/repo[/path]` / `zip` |
| `skill:uninstall` | `{id}` | `()` |
| `skill:toggle` | `{agent_id, skill_id, enabled}` | `()` |
| `skill:search-market` | `{query}` | `Vec<SkillSearchHit>` |
| `skill:list-local` | `{workdir}` | `Vec<LocalSkill>` |

**知识域** `commands/wiki.rs` + `commands/rag.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `wiki:create` | `{name, description?}` | `WikiDto` |
| `wiki:list` | `{}` | `Vec<WikiDto>` |
| `wiki:delete` | `{id}` | `()` |
| `wiki:read-page` | `{wiki_id, path}` | `{content}` |
| `wiki:write-page` | `{wiki_id, path, content}` | `()` |
| `wiki:list-pages` | `{wiki_id}` | `Vec<WikiPage>` |
| `wiki:write-ai` | `{wiki_id, info, preview?}` | `{plan?}` | LLM 自动入库（preview=true 仅返回计划，§10.1.1） |
| `wiki:ingest-ai` | `{wiki_id, file_path}` | `{summary}` | 导入文件 + 自动入库 |
| `wiki:apply-plan` | `{wiki_id, plan}` | `{result}` | 用户确认计划后执行 |
| `wiki:restore-trash` | `{wiki_id, path}` | `()` | 从 .trash 恢复已删页面 |
| `rag:ingest` | `{wiki_id, file_path}` | `{document_id, status}` |
| `rag:list-documents` | `{wiki_id}` | `Vec<RagDocumentDto>` |
| `rag:delete-document` | `{document_id}` | `()` |
| `rag:search` | `{wiki_id, query, top_k?}` | `Vec<RagHit>` |

**会议域** `commands/meeting.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `meeting:create` | `{title, participants?}` | `MeetingDto` |
| `meeting:list` | `{}` | `Vec<MeetingDto>` |
| `meeting:get` | `{id}` | `MeetingDto` |
| `meeting:delete` | `{id}` | `()` |
| `meeting:start-recording` | `{id, asr_config}` | `()` | 转写走事件 |
| `meeting:stop-recording` | `{id}` | `{transcript}` |
| `meeting:pause-recording` | `{id}` | `()` |
| `meeting:resume-recording` | `{id}` | `()` |
| `meeting:cancel-recording` | `{id}` | `()` |
| `meeting:retranscribe` | `{id, asr_config}` | `()` | 换 ASR 模型重新转写 |
| `meeting:clean` | `{id}` | `{cleaned}` |
| `meeting:summary` | `{id}` | `{summary}` |
| `meeting:qa` | `{id, question}` | `{answer}` |
| `meeting:push-to-agent` | `{meeting_id, agent_id, session_id?}` | `()` |
| `meeting:export` | `{id, format, options?}` | `{path}` | md/docx |

**ASR 域** `commands/asr.rs`（会议子域，§10.3.8，见 phase3-extend.md）

| 命令 | 参数 | 返回 |
|------|------|------|
| `asr:backends` | `{}` | `Vec<AsrBackendInfo>` |
| `asr:model-catalog` | `{}` | `Vec<AsrModelInfo>` |
| `asr:model-installed` | `{}` | `Vec<InstalledAsrModel>` |
| `asr:model-download` | `{model_id}` | `()` | 进度走事件 |
| `asr:model-remove` | `{model_id}` | `()` |
| `asr:test` | `{asr_config}` | `{ok, latency_ms, error?}` |

**文件域** `commands/file.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `file:pick` | `{filters?}` | `String` |
| `file:read-text` | `{path, max_bytes?}` | `String` |
| `file:write` | `{path, content}` | `()` |
| `file:list` | `{dir}` | `Vec<FileEntry>` |
| `file:parse` | `{path}` | `{text, mime}` | pdf/docx/html→文本 |

**翻译/OCR 域** `commands/translate.rs` / `commands/ocr.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `translate:translate` | `{text, source?, target, model_id?}` | `{translated, source, from_cache}` |
| `translate:batch` | `{texts, source?, target}` | `Vec<TranslateResult>` | 进度走 `translate:batch-progress` 事件 |
| `translate:file` | `{path, source?, target, out_path?}` | `{content}` | 整文件翻译 |
| `translate:history` | `{query?, limit?, offset?}` | `{items, total}` |
| `translate:detect` | `{text}` | `{lang, confidence}` |
| `glossary:list` | `{lang_pair?}` | `Vec<GlossaryTerm>` |
| `glossary:add` | `{term}` | `()` |
| `glossary:remove` | `{id}` | `()` |
| `glossary:import-csv` | `{path}` | `{imported, failed}` |
| `ocr:recognize` | `{image_path, lang?, provider?}` | `OcrResult` |
| `ocr:providers` | `{}` | `Vec<OcrProviderInfo>` |

**工作流域** `commands/workflow.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `workflow:list` | `{}` | `Vec<WorkflowDto>` |
| `workflow:run` | `{workflow_id, inputs}` | `{run_id}` | 进度走事件 |
| `workflow:stop` | `{run_id}` | `()` |
| `workflow:result` | `{run_id}` | `WorkflowResultDto` |

**任务域** `commands/task.rs`（Phase 2 任务设计区，§9.9.1，phase2-panel.md）

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `task:save-template` | `{definition}` | `WorkflowDto` | 保存自定义任务为模板（写入 workflows 表） |
| `task:run` | `{definition, inputs}` | `{run_id}` | 运行自定义任务（TaskDefinition→Workflow 映射） |
| `task:validate` | `{definition}` | `{ok, errors}` | 画布保存前校验（环检测/变量引用/工具存在性） |
| `task:rerun` | `{run_id, inputs?}` | `{run_id}` | 用相同定义重跑 |

**设置域** `commands/settings.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `settings:get` | `{key}` | `Value` |
| `settings:set` | `{key, value}` | `()` |
| `settings:providers` | `{}` | `Vec<ProviderDto>`（不含 Key） |
| `settings:save-provider-key` | `{provider_id, api_key}` | `()` | AES-GCM 加密 |

**仪表盘域** `commands/dashboard.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `dashboard:overview` | `{}` | `DashboardOverview` | 主页面板聚合数据（见 9.9） |
| `usage:stats` | `{range?}` | `UsageStats` | 用量统计：今日/本周/本月 token 与费用、按模型/Agent 分组 |
| `usage:trend` | `{days?}` | `Vec<UsagePoint>` | 用量趋势（按日聚合，供图表） |
| `mcp:status-all` | `{}` | `Vec<McpServerStatus>` | 全部 MCP 服务器状态（连接/错误/工具数） |

**工作区域** `commands/workspace.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `workspace:get` | `{agent_id?}` | `WorkspaceInfo` | 工作目录、最近目录、agent 上下文 |
| `workspace:set` | `{path}` | `WorkspaceInfo` | 切换工作目录（持久化到 preference） |
| `workspace:instructions` | `{path?}` | `Vec<InstructionFile>` | 读取工作目录指令文件（CLAUDE.md/AGENTS.md/README.md） |
| `workspace:write-instructions` | `{path, content}` | `()` | 写入/编辑指令文件 |
| `workspace:tree` | `{path?, depth?, max_entries?}` | `DirTree` | 目录树（单层/递归，忽略 .git/node_modules 等） |
| `workspace:read-file` | `{path}` | `{content, mime}` | 读取文件（文本截断保护） |
| `workspace:open-file` | `{path}` | `()` | 用默认程序打开 |
| `fs:watch` | `{workdir, enable}` | `()` | 开启/关闭工作目录变更监听（Phase 2，§9.10.7） |

**LSP 域** `commands/lsp.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `lsp:list` | `{workdir}` | `Vec<LspServerInfo>` | 启用的语言服务器状态 |
| `lsp:diagnostics` | `{path}` | `Vec<Diagnostic>` | 当前文件诊断（错误/警告） |
| `lsp:start` | `{server_id, workdir}` | `()` | 启动语言服务器 |
| `lsp:stop` | `{server_id}` | `()` | 停止语言服务器 |
| `lsp:detect` | `{workdir}` | `Vec<LspCandidate>` | 推断候选 LSP（Phase 2，§9.10.7，无进程启动） |

**Agent 上下文域** `commands/context.rs`（侧边栏聚合）

| 命令 | 参数 | 返回 |
|------|------|------|
| `context:agent` | `{agent_id, session_id?}` | `AgentContext` | 侧边栏聚合：用量/工作目录/指令/MCP/LSP/目录（见 9.10） |

**记忆域** `commands/memory.rs`（§10.7.4）

| 命令 | 参数 | 返回 |
|------|------|------|
| `memory:search` | `{query, scope?, type?, limit?}` | `Vec<MemoryHit>` |
| `memory:read` | `{path}` | `{body}` |
| `memory:write` | `{scope, key, body}` | `()` | 仅主 agent 规则类 |
| `memory:append-notes` | `{scope, entry}` | `()` | 追加 notes.md |
| `memory:reconcile` | `{}` | `{indexed, pruned}` |
| `memory:context-dump` | `{}` | `Vec<MemoryDump>` |

**系统域** `commands/system.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `system:info` | `{}` | `SystemInfo` | 平台/版本/资源信息 |
| `system:open-external` | `{url}` | `()` | 打开外部链接 |

### 8.3 事件清单（后端 → 前端）

| 事件 | 负载 | 触发时机 |
|------|------|----------|
| `chat:stream:start` | `{session_id, message_id, model}` | 流开始 |
| `chat:stream:delta` | `{session_id, message_id, delta}` | 每 token |
| `chat:stream:tool_call` | `{session_id, message_id, call}` | 工具调用 |
| `chat:stream:tool_result` | `{session_id, message_id, call_id, ok, output}` | 工具返回 |
| `chat:stream:done` | `{session_id, message_id, usage}` | 流结束 |
| `chat:stream:error` | `{session_id, message_id, message}` | 出错 |
| `chat:stream:aborted` | `{session_id, message_id}` | 中断 |
| `meeting:transcript` | `{meeting_id, index, text, is_final}` | ASR 增量 |
| `meeting:translation` | `{meeting_id, index, translated}` | 翻译增量 |
| `meeting:status` | `{meeting_id, status}` | 录音状态 |
| `workflow:stage` | `{run_id, stage_id, status, output?}` | 阶段完成 |
| `workflow:done` | `{run_id, result}` | 工作流完成 |
| `mcp:tools-changed` | `{server_id}` | 工具目录变更 |
| `mcp:status-changed` | `{server_id, status}` | MCP 服务器状态变化（§9.10.7，见 phase2-panel.md） |
| `model:list-changed` | `{}` | 模型配置变更 |
| `agent:changed` | `{event, agent}` | agent CRUD 事件 |
| `usage:updated` | `{stats}` | 用量更新（消息完成后推送，刷新面板） |
| `lsp:status-changed` | `{server_id, status}` | LSP 服务器状态变化 |
| `lsp:diagnostics` | `{path, diagnostics}` | 诊断更新 |
| `workspace:changed` | `{path}` | 工作目录切换 |
| `tool:approval-request` | `{call_id, tool_name, arguments, agent_id, risk_level}` | 工具审批请求（§10.10，见 phase2-panel.md） |
| `tool:approval-response` | `{call_id, approved, reason?}` | 工具审批响应（前端 → 后端） |
| `rag:progress` | `{document_id, status, progress?}` | RAG 摄取进度 |
| `translate:batch-progress` | `{done, total}` | 批量翻译进度 |
| `memory:changed` | `{path, scope, type}` | 记忆文件变更 |
| `asr:model-download-progress` | `{model_id, downloaded, total}` | ASR 模型下载进度 |
| `asr:backend-status` | `{backend_id, status}` | ASR 后端健康状态 |

#### 8.3.1 事件清单（前端 → 后端）

| 事件 | 负载 | 触发时机 |
|------|------|----------|
| `meeting:audio-chunk` | `{meeting_id, pcm: base64}` | 渲染进程音频块（§10.3.2，见 phase3-extend.md） |
| `tool:approval-response` | `{call_id, approved, reason?}` | 用户工具审批结果（§10.10，见 phase2-panel.md） |

> 注：其余前端 → 后端交互一律走 `invoke` 命令（§8.2），仅高频流（音频块）与异步响应（审批）走事件通道（§14.6 #34）。

---

## 9. Svelte 5 前端详细设计

### 9.1 设计令牌系统（两层架构）

**参考**：Cherry Studio 两层 token 架构 + iOS 18 精确色值 + Apple Design 色彩体系。

**Layer 1：原始令牌**（`tokens/primitives/`）— iOS 18 系统色 + oklch 感知均匀空间：

```ts
// tokens/primitives/colors.ts
export const primitives = {
    // iOS 18 系统语义色（精确 hex 值，对齐 HIG）
    blue:   { light: "#007AFF", dark: "#0A84FF" },
    green:  { light: "#34C759", dark: "#30D158" },
    indigo: { light: "#5856D6", dark: "#5E5CE6" },
    orange: { light: "#FF9500", dark: "#FF9F0A" },
    pink:   { light: "#FF2D55", dark: "#FF375F" },
    purple: { light: "#AF52DE", dark: "#BF5AF2" },
    red:    { light: "#FF3B30", dark: "#FF453A" },
    teal:   { light: "#5AC8FA", dark: "#64D2FF" },
    yellow: { light: "#FFCC00", dark: "#FFD60A" },

    // iOS 18 灰度（6 级）
    gray: {
        1:  { light: "#8E8E93", dark: "#8E8E93" },
        2:  { light: "#AEAEB2", dark: "#636366" },
        3:  { light: "#C7C7CC", dark: "#48484A" },
        4:  { light: "#D1D1D6", dark: "#3A3A3C" },
        5:  { light: "#E5E5EA", dark: "#2C2C2E" },
        6:  { light: "#F2F2F7", dark: "#1C1C1E" },
    },
} as const;
```

**Layer 2：语义别名**（`tokens/semantic.css`）— iOS 18 背景/填充系统：

```css
/* tokens/semantic.css — iOS 18 亮色主题 */
:root {
    /* 背景层级（iOS 18 三级系统背景） */
    --color-background: #FFFFFF;
    --color-secondary-background: #F2F2F7;
    --color-tertiary-background: #FFFFFF;

    /* 分组背景（设置页等分组列表） */
    --color-grouped-background: #F2F2F7;
    --color-secondary-grouped-background: #FFFFFF;
    --color-tertiary-grouped-background: #F2F2F7;

    /* 表面层级 */
    --color-card: #FFFFFF;              /* 卡片/面板 */
    --color-popover: #FFFFFF;           /* 弹窗/菜单 */

    /* 填充（iOS 18 四级系统填充，用于输入框/搜索框背景） */
    --color-fill: rgba(120, 120, 128, 0.2);
    --color-secondary-fill: rgba(120, 120, 128, 0.16);
    --color-tertiary-fill: rgba(120, 120, 128, 0.12);
    --color-quaternary-fill: rgba(120, 120, 128, 0.08);

    /* 标签层级 */
    --color-label: rgba(0, 0, 0, 0.85);
    --color-secondary-label: rgba(60, 60, 67, 0.6);
    --color-tertiary-label: rgba(60, 60, 67, 0.3);
    --color-quaternary-label: rgba(60, 60, 67, 0.18);

    /* 分隔线 */
    --color-separator: rgba(60, 60, 67, 0.29);
    --color-opaque-separator: #C6C6C8;

    /* 语义色 */
    --color-primary: #007AFF;
    --color-destructive: #FF3B30;
    --color-success: #34C759;
    --color-warning: #FF9500;
    --color-info: #5AC8FA;

    /* Apple 毛玻璃材质 */
    --color-glass: rgba(255, 255, 255, 0.72);
    --color-glass-border: rgba(255, 255, 255, 0.5);
    --glass-blur: blur(20px) saturate(180%);
}

/* iOS 18 暗色主题 — 真反转 */
.dark {
    --color-background: #000000;
    --color-secondary-background: #1C1C1E;
    --color-tertiary-background: #2C2C2E;

    --color-grouped-background: #000000;
    --color-secondary-grouped-background: #1C1C1E;
    --color-tertiary-grouped-background: #2C2C2E;

    /* 表面层级 */
    --color-card: #1C1C1E;              /* 卡片/面板 */
    --color-popover: #2C2C2E;           /* 弹窗/菜单 */

    --color-fill: rgba(120, 120, 128, 0.36);
    --color-secondary-fill: rgba(120, 120, 128, 0.32);
    --color-tertiary-fill: rgba(120, 120, 128, 0.24);
    --color-quaternary-fill: rgba(120, 120, 128, 0.18);

    --color-label: rgba(255, 255, 255, 0.85);
    --color-secondary-label: rgba(235, 235, 245, 0.6);
    --color-tertiary-label: rgba(235, 235, 245, 0.3);
    --color-quaternary-label: rgba(235, 235, 245, 0.18);

    --color-separator: rgba(84, 84, 88, 0.65);
    --color-opaque-separator: #38383A;

    --color-primary: #0A84FF;
    --color-destructive: #FF453A;
    --color-success: #30D158;
    --color-warning: #FF9F0A;
    --color-info: #64D2FF;

    --color-glass: rgba(28, 28, 30, 0.72);
    --color-glass-border: rgba(255, 255, 255, 0.08);
}
```

**状态色完整变体**（iOS 18 风格）：

```css
/* error — 以 red 为基础（引用语义色板已有 token） */
--color-error: var(--color-destructive);          /* #FF3B30 */
--color-error-text: var(--color-destructive);
--color-error-bg: rgba(255, 59, 48, 0.12);       /* iOS 系统 fill 风格 */
--color-error-border: rgba(255, 59, 48, 0.3);

/* success / warning / info 同理，使用对应系统色的 0.12 opacity 作为 bg */
```

**圆角系统**（iOS 18 标准）：

| Token | 值 | iOS 18 用途 |
|-------|-----|------------|
| `--radius-xs` | 4px | 微圆角（badge/tag） |
| `--radius-sm` | 8px | 按钮文字、小元素 |
| `--radius-md` | 10px | 输入框、搜索框（iOS 18 系统 fill） |
| `--radius-lg` | 12px | 按钮、卡片、列表组 |
| `--radius-xl` | 14px | 模态框、操作表（iOS 18 标准） |
| `--radius-full` | 9999px | 圆形头像/按钮 |

### 9.2 排版令牌（tokens/typography.ts）

**参考**：iOS 18 Dynamic Type 完整字号系统 + Apple Design 排版规则（tracking 随字号变化、leading 与字号反比）。

```css
:root {
    /* 字体链（iOS 18 标准） */
    --font-system: -apple-system, BlinkMacSystemFont, "SF Pro Display",
                   "SF Pro Text", "PingFang SC", "Helvetica Neue", sans-serif;
    --font-mono: "SF Mono", "Menlo", "Monaco", "Cascadia Code", "Courier New", monospace;

    /* iOS 18 Dynamic Type 完整字号系统 */
    /* Large Title — 页面大标题 */
    --text-large-title: 34px;
    --line-height-large-title: 41px;
    --weight-large-title: 700;
    --tracking-large-title: 0.37px;

    /* Title 1 — 一级标题 */
    --text-title1: 28px;
    --line-height-title1: 34px;
    --weight-title1: 700;
    --tracking-title1: 0.36px;

    /* Title 2 — 二级标题 */
    --text-title2: 22px;
    --line-height-title2: 28px;
    --weight-title2: 700;
    --tracking-title2: 0.35px;

    /* Title 3 — 三级标题 */
    --text-title3: 20px;
    --line-height-title3: 25px;
    --weight-title3: 600;
    --tracking-title3: 0.38px;

    /* Headline — 突出文本 */
    --text-headline: 17px;
    --line-height-headline: 22px;
    --weight-headline: 600;
    --tracking-headline: -0.41px;

    /* Body — 正文 */
    --text-body: 17px;
    --line-height-body: 22px;
    --weight-body: 400;
    --tracking-body: -0.41px;

    /* Callout — 辅助说明 */
    --text-callout: 16px;
    --line-height-callout: 21px;
    --weight-callout: 400;
    --tracking-callout: -0.32px;

    /* Subheadline — 次要标题 */
    --text-subheadline: 15px;
    --line-height-subheadline: 20px;
    --weight-subheadline: 400;
    --tracking-subheadline: -0.24px;

    /* Footnote — 脚注 */
    --text-footnote: 13px;
    --line-height-footnote: 18px;
    --weight-footnote: 400;
    --tracking-footnote: -0.08px;

    /* Caption 1 — 注释 */
    --text-caption1: 12px;
    --line-height-caption1: 16px;
    --weight-caption1: 400;
    --tracking-caption1: 0px;

    /* Caption 2 — 最小注释 */
    --text-caption2: 11px;
    --line-height-caption2: 13px;
    --weight-caption2: 600;
    --tracking-caption2: 0.07px;
}
```

**Apple Design 排版规则**（来自 WWDC *The Details of UI Typography*）：

| 规则 | 说明 |
|------|------|
| **tracking 随字号变化** | 大标题用负 tracking（-0.41px），正文接近 0，小注释用正 tracking（0.07px）。永远不要一个 `letter-spacing` 值用于所有字号 |
| **leading 与字号反比** | 大标题行高紧凑（41/34 ≈ 1.2），正文宽松（22/17 ≈ 1.29） |
| **用 weight+size+leading 建立层级** | 不要只靠字号，用字重增加存在感 |
| **尊重用户字体设置** | 用 `rem`/`em` 而非固定 px，支持 Dynamic Type |
| **默认用系统字体** | 它已内建光学尺寸、tracking 表和可读性调优 |

### 9.3 毛玻璃实现（glass.css）

```css
.glass {
    backdrop-filter: saturate(180%) blur(20px);
    background: var(--color-glass);
    border: 1px solid var(--color-glass-border);
}
```

### 9.4 动画令牌（tokens/motion.ts）

**参考**：Apple Design fluid interface spring 参数 + iOS 18 缓动函数 + `prefers-reduced-motion` 适配。

```ts
export const motion = {
    // === Apple Design Spring 参数（Designing Fluid Interfaces）===
    // 关键：spring 无固定时长，settling time 由参数涌现
    // damping: 1.0 = 临界阻尼（无回弹）；< 1.0 = 欠阻尼（有回弹）
    // response: 到达目标的速度（秒），非 duration

    // 默认 UI spring（临界阻尼，无回弹，优雅不干扰）
    springDefault: { damping: 1.0, response: 0.35 },
    // 动量交互 spring（有回弹，仅当手势本身有动量时）
    springMomentum: { damping: 0.8, response: 0.35 },
    // 面板/抽屉 spring（Cherry Studio 风格）
    springPanel: { damping: 30, stiffness: 350 },

    // iOS 18 缓动曲线
    ease: "cubic-bezier(0.25, 0.1, 0.25, 1)",
    easeIn: "cubic-bezier(0.4, 0, 1, 1)",
    easeOut: "cubic-bezier(0, 0, 0.2, 1)",
    easeInOut: "cubic-bezier(0.4, 0, 0.2, 1)",
    spring: "cubic-bezier(0.34, 1.56, 0.64, 1)",

    // iOS 18 时长规范
    durationFast: 150,      // 按钮点击
    durationBase: 200,      // 列表项出现、开关切换
    durationNormal: 250,    // 模态框弹出
    durationSlow: 300,      // 页面过渡
    durationSheet: 350,     // 操作表/抽屉
} as const;

// === Apple Design 交互规则 ===
// 1. 按钮反馈在 pointer-down，不在 click/touch-up
// 2. 拖拽全程 1:1 跟踪，不仅在释放时
// 3. 所有动画可中断（interruptibility）
// 4. 中断时从当前屏幕值（presentation value）开始，非目标值
// 5. 反转时混合速度（velocity blending），不硬切
// 6. 动量投射：用速度预测落点，非从释放点吸附
// 7. 橡皮筋：边界处渐进阻力，不停硬停
```

### 9.4.1 Fluid Interface 原则（Apple Design）

**来源**：Apple WWDC *Designing Fluid Interfaces* — 接口何时停止像计算机，开始像自身的延伸。

**核心理念**：当运动从当前屏幕值开始、继承用户速度、向前投射动量、且可在任意时刻抓取反转时，接口就「活」了。Spring 是使这一切自然的工具——它天然可中断且感知速度。

**七项原则**：

| # | 原则 | 说明 | Prism Agent R 应用 |
|---|------|------|-------------------|
| 1 | **响应** | 在 pointer-down 而非 release 时反馈；消除一切延迟 | 按钮 `:active` 立即 scale(0.97) |
| 2 | **直接操纵** | 1:1 跟踪，尊重抓取偏移（where they grabbed） | 拖拽 Splitter、Drawer 跟手 |
| 3 | **可中断性** | 从 presentation value 开始，不从 target value；反转时混合速度 | 所有 spring 动画可中断 |
| 4 | **行为优于动画** | 用 spring 替代固定时长动画；spring 响应新输入只需改 target | 弹窗/面板用 springDefault |
| 5 | **速度交接** | 手势结束时，动画以手指精确速度继续 | 抽屉释放速度 → spring initial velocity |
| 6 | **动量投射** | 用速度预测落点，非从释放点吸附 | `project(velocity) → nearestSnap` |
| 7 | **空间一致性** | 进出路径对称；锚定到触发源 | 右侧滑入 → 右侧滑出；菜单从按钮 origin 弹出 |

**Apple 精确参数**：

| 交互 | damping | response | 说明 |
|------|---------|----------|------|
| 移动/重定位（如 PiP） | 1.0 | 0.4s | 临界阻尼，无回弹 |
| 旋转 | 0.8 | 0.4s | 轻微回弹 |
| 抽屉/Sheet | 0.8 | 0.3s | 手势驱动，有回弹 |
| 默认 UI 元素 | 1.0 | 0.3~0.4s | 优雅不干扰 |

**橡皮筋公式**（边界软阻力）：

```ts
function rubberband(overshoot: number, dimension: number, constant = 0.55): number {
    return (overshoot * dimension * constant) / (dimension + constant * Math.abs(overshoot));
}
```

**动量投射公式**（Apple *Designing Fluid Interfaces* sample code）：

```ts
function project(initialVelocity: number, decelerationRate = 0.998): number {
    return (initialVelocity / 1000) * decelerationRate / (1 - decelerationRate);
}
// element at y=50, target y=150 (100px to go), finger moving 50px/s
// → projectedEndpoint = 50 + project(50) = 50 + 49.9 = ~100
// → nearestSnapPoint(100) → target
```

**手势设计清单**：

| 手势 | 规则 |
|------|------|
| Tap | pointer-down 即高亮（即时），touch-up 提交；~10px hysteresis；可拖离取消 |
| Drag/Swipe | ~10px 阈值后锁定方向，然后 1:1 跟踪 |
| 手势识别 | 从第一次 move 并行检测所有可能手势，确认后取消失败者 |
| 双击 | 会延迟单击，仅在双击确实存在时使用 |

### 9.4.2 材质与深度（Materials & Depth）

**来源**：Apple Design — 半透明材质作为浮动功能层，用层次建立结构而不抢焦点。

**层级系统**（表面颜色分层）：

| 层级 | 亮色 | 暗色 | 用途 |
|------|------|------|------|
| Ground | `--color-background` | `#000000` | 页面背景 |
| Surface | `--color-secondary-background` | `#1C1C1E` | 卡片/面板 |
| Raised | `--color-popover` | `#2C2C2E` | 弹窗/菜单 |
| Accent | `--color-fill` | `rgba(120,120,128,0.36)` | hover 背景 |

**毛玻璃实现**（iOS 18 navbar/toolbar 标准）：

```css
.toolbar {
    background: var(--color-glass);           /* rgba(255,255,255,0.72) 亮色 */
    backdrop-filter: var(--glass-blur);       /* blur(20px) saturate(180%) */
    -webkit-backdrop-filter: var(--glass-blur);
    border-bottom: 0.5px solid var(--color-separator);
}

.dark .toolbar {
    background: var(--color-glass);           /* rgba(28,28,30,0.72) 暗色 */
    border-bottom-color: rgba(255, 255, 255, 0.1);
}
```

**材质规则**（Apple Design）：

| 规则 | 说明 |
|------|------|
| 内容在材质下滚动 | toolbar/sheet 是半透明层，内容从下方滚过，不是固定条 |
| 材质重量编码层级 | 更暗/更重的材质分隔结构区域（侧边栏）；更轻的材质吸引交互元素 |
| 禁止叠放轻材质 | 亮色半透明叠在另一个上 → 可读性崩溃 |
| 大表面 = 更厚材质 | 更强 blur + 更深 shadow |
| 减暗聚焦，分离保持流 | 模态任务用 scrim + 推回背景；非阻塞面板用半透明 + 偏移，无 scrim |
| 滚动边缘效果 | 内容与浮动 chrome 交界处用渐变 mask，不用 1px border |

**阴影系统**（7 级，扁平优先）：

```css
:root {
    --shadow-2xs: 0 1px 2px rgba(0, 0, 0, 0.05);
    --shadow-xs: 0 1px 3px rgba(0, 0, 0, 0.1);
    --shadow-sm: 0 2px 4px rgba(0, 0, 0, 0.1);
    --shadow-md: 0 4px 8px rgba(0, 0, 0, 0.1);
    --shadow-lg: 0 8px 16px rgba(0, 0, 0, 0.15);
    --shadow-xl: 0 16px 32px rgba(0, 0, 0, 0.2);
    --shadow-2xl: 0 32px 64px rgba(0, 0, 0, 0.25);
}
/* 静止时扁平（flat-at-rest），仅 hover/浮动时使用 shadow */
```

### 9.4.3 组件架构（Primitives + Composites）

**参考**：Cherry Studio 50+ 原子组件 + 25+ 复合组件的分层模式。

> **目录说明**：本节的 `primitives/` + `composites/` 两层结构是组件目录的**权威定义**；
> §4.2 组件树中的 `components/base/`（早期命名）与 `components/chat/`、`components/agent/` 等业务目录
> 在实现时并入本结构——base/ 归入 primitives/，业务组件归入 composites/ 或各自业务子目录，不重复实现。

**分层原则**：
- **原子组件（Primitives）**：无业务逻辑，纯 UI 原语，基于 Radix UI（Svelte 版用 bits-ui）
- **复合组件（Composites）**：组合原子组件，包含布局逻辑，仍无业务逻辑
- **页面组件**：业务逻辑 + 复合组件 + 状态管理

**Prism Agent R 组件清单**：

```
src/lib/components/
├── primitives/                    # 原子组件（50+，bits-ui 基础）
│   ├── Button.svelte              # 5 变体 × 多尺寸（primary/secondary/text/gray/destructive，见 §9.7）
│   ├── Input.svelte               # 文本输入
│   ├── Textarea.svelte            # 多行输入
│   ├── Select.svelte              # 下拉选择
│   ├── Switch.svelte              # 开关（4 尺寸 xs/sm/md/lg + loading）
│   ├── Checkbox.svelte            # 复选框
│   ├── Dialog.svelte              # 弹窗（4 尺寸 sm/default/lg/xl）
│   ├── Drawer.svelte              # 抽屉（Vaul 风格）
│   ├── Popover.svelte             # 浮层
│   ├── Tooltip.svelte             # 提示
│   ├── Tabs.svelte                # 标签页
│   ├── Accordion.svelte           # 折叠面板
│   ├── Badge.svelte               # 徽标
│   ├── Avatar.svelte              # 头像
│   ├── Skeleton.svelte            # 骨架屏
│   ├── Spinner.svelte             # 加载器
│   ├── ScrollArea.svelte          # 自定义滚动条（6px 细滚动条）
│   ├── Separator.svelte           # 分隔线
│   ├── Kbd.svelte                 # 键盘快捷键展示
│   ├── ContextMenu.svelte         # 右键菜单
│   ├── DropdownMenu.svelte        # 下拉菜单
│   ├── Command.svelte             # 命令面板（cmdk）
│   ├── Calendar.svelte            # 日历
│   ├── Slider.svelte              # 滑块
│   └── ...                        # 更多原子组件
├── composites/                    # 复合组件（25+）
│   ├── PageHeader.svelte          # 页面标题（通用）
│   ├── PageSidePanel.svelte       # 浮动侧边面板（spring 动画）
│   ├── ConfirmDialog.svelte       # 确认弹窗
│   ├── SearchInput.svelte         # 搜索输入
│   ├── DataTable.svelte           # 数据表格（TanStack Table）
│   ├── MarkdownViewer.svelte      # Markdown 渲染（流式支持）
│   ├── CodeBlock.svelte           # 代码块（shiki 语法高亮）
│   ├── EmptyState.svelte          # 空态占位
│   ├── SortableList.svelte        # 拖拽排序
│   ├── TreeView.svelte            # 树形视图
│   ├── Flex.svelte                # 布局辅助
│   ├── Ellipsis.svelte            # 文本截断
│   └── ...                        # 更多复合组件
└── layout/                        # 布局组件
    ├── AppShell.svelte            # 三栏主框架
    ├── SideNav.svelte             # 左侧导航
    ├── ContentArea.svelte         # 中央内容区
    ├── RightPanel.svelte          # 右侧工具面板
    └── StatusBar.svelte           # 底部状态栏
```

**组件设计规范**：

| 规范 | 值 | 来源 |
|------|-----|------|
| Button 尺寸 | `min-height: 50px; padding: 16px 24px; font-size: 17px` | iOS 18 标准按钮 |
| Button 圆角 | `12px`（iOS 18 标准） | iOS 18 |
| Button 反馈 | `:active { transform: scale(0.98) }`（pointer-down 即触发） | Apple Design fluid interface |
| Dialog 圆角 | `14px`（iOS 18 模态框标准） | iOS 18 |
| Dialog 背景 | `rgba(255,255,255,0.72)` + `backdrop-filter: blur(20px) saturate(180%)` | iOS 18 毛玻璃 |
| Modal 动画 | `scale(0.95) → scale(1)` + `opacity 0→1`，200ms ease | iOS 18 |
| 开关尺寸 | `51×31px`，thumb `27px` 圆形 | iOS 18 Toggle |
| 列表项 | `min-height: 44px`，`0.5px` 分隔线 | iOS 18 |
| 输入框 | `background: rgba(120,120,128,0.12)`，`border-radius: 10px` | iOS 18 系统 fill |
| 触摸目标 | `≥44×44pt` | iOS 18 无障碍 |
| 对比度 | 正文 `4.5:1`，大文本/图标 `3:1` | iOS 18 无障碍 |
| 阴影策略 | 静止时扁平，hover/浮动时 `shadow-md` | Apple Design flat-at-rest |
| 滚动条 | 6px 细滚动条，圆角 thumb | Cherry Studio |
| 焦点环 | `ring-2 ring-primary/50` | 无障碍标准 |
| 响应式断点 | 640/1024/1280/1536px（mobile/tablet/desktop/wide/ultra-wide） | Cherry Studio |

### 9.5 Codex 风格三栏布局

```
┌─────────┬─────────────────────────────┬──────────────┐
│ SideNav │      ContentArea            │ RightPanel   │
│ (240px) │  ┌───────────────────────┐  │ (320px 可折叠)│
│ Logo    │  │ 会话标题 + 模型选择    │  │ Agent 信息    │
│ 新建对话 │  ├───────────────────────┤  ├────────────┤
│ 历史会话 │  │                       │  │ 工具(MCP)   │
│ (搜索)   │  │   MessageList         │  ├────────────┤
│ ─────── │  │   (流式/工具卡片)      │  │ 技能已启用   │
│ Agent   │  │                       │  └────────────┘
│ 知识库   │  ├───────────────────────┤
│ 会议    │  │  Composer              │
│ 技能市场 │  └───────────────────────┘
│ 设置    │  │ StatusBar: 模型/状态   │
└─────────┴─────────────────────────────┴──────────────┘
```

- 三栏宽度可拖拽（Splitter）
- RightPanel 可折叠（⌘\）
- SideNav 可折叠为图标模式（44px）
- 会话列表支持搜索/固定/重命名/删除

### 9.6 状态管理（Svelte 5 Runes）

```ts
// lib/stores/chat.svelte.ts
export class ChatStore {
    current = $state<Session | null>(null);
    messages = $state<Message[]>([]);
    streaming = $state(false);
    streamingText = $state("");
    activeToolCalls = $state<Map<string, ToolCallView>>(new Map());

    async send(content: string) {
        this.streaming = true;
        const msg = await api.chat.send(this.current!.id, content);
        this.messages = [...this.messages, msg];
        onStreamEvents(this.current!.id, {
            onDelta: (d) => { this.streamingText += d; },
            onToolCall: (c) => this.activeToolCalls.set(c.id, { name: c.name, status: "running" }),
            onDone: () => { this.streaming = false; this.refreshHistory(); },
            onError: (m) => { this.streaming = false; toast.error(m); },
        });
    }
}
export const chatStore = new ChatStore();
```

### 9.7 基础组件示例（Button.svelte）

**iOS 18 按钮规范**（来自 ios18-design-system skill）：

```svelte
<script lang="ts">
    type Variant = "primary" | "secondary" | "text" | "gray" | "destructive";
    let { variant = "primary", disabled = false, onclick, children }: {
        variant?: Variant; disabled?: boolean; onclick?: () => void; children: Snippet;
    } = $props();
</script>

<button class={`btn btn-${variant}`} {disabled} {onclick}>
    {@render children()}
</button>

<style>
    .btn {
        display: inline-flex; align-items: center; justify-content: center;
        gap: 8px; padding: 16px 24px; min-height: 50px;
        font-size: 17px; font-weight: 600; font-family: var(--font-system);
        border: none; border-radius: 12px; cursor: pointer;
        transition: all 0.2s var(--ease);
    }
    /* Apple Design: 反馈在 pointer-down，不在 click */
    .btn:active { transform: scale(0.98); }
    .btn:disabled { opacity: 0.4; cursor: not-allowed; }

    /* Primary — iOS 18 蓝色填充 */
    .btn-primary { background: var(--color-primary); color: #FFFFFF; }
    .btn-primary:hover { background: #0066D6; }

    /* Secondary — iOS 18 系统填充背景 */
    .btn-secondary {
        color: var(--color-primary);
        background: rgba(0, 122, 255, 0.12);
    }
    .btn-secondary:hover { background: rgba(0, 122, 255, 0.18); }

    /* Text — 无背景 */
    .btn-text {
        padding: 8px 12px; min-height: auto;
        color: var(--color-primary); background: transparent;
    }
    .btn-text:hover { background: rgba(0, 122, 255, 0.08); }

    /* Gray — iOS 18 灰色填充 */
    .btn-gray {
        color: var(--color-primary);
        background: rgba(120, 120, 128, 0.12);
    }

    /* Destructive — iOS 18 红色 */
    .btn-destructive {
        color: var(--color-destructive);
        background: rgba(255, 59, 48, 0.12);
    }
</style>
```

**iOS 18 组件 CSS 参考**（关键组件的精确实现值）：

```css
/* 模态框（iOS 18 标准：270px 宽，毛玻璃背景，14px 圆角） */
.modal-content {
    width: 270px;
    background: var(--color-glass);
    backdrop-filter: var(--glass-blur);
    border-radius: 14px;
    animation: modalEnter 0.2s var(--ease);
}
@keyframes modalEnter {
    from { opacity: 0; transform: scale(0.95); }
    to { opacity: 1; transform: scale(1); }
}

/* 开关（iOS 18 标准：51×31px，27px 圆形 thumb） */
.toggle {
    width: 51px; height: 31px;
    background: rgba(120, 120, 128, 0.16);
    border-radius: 16px; cursor: pointer;
    transition: background 0.2s var(--ease);
}
.toggle::after {
    content: ''; position: absolute;
    top: 2px; left: 2px; width: 27px; height: 27px;
    background: #FFFFFF; border-radius: 50%;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
    transition: transform 0.2s var(--ease);
}
.toggle.active { background: #34C759; }
.toggle.active::after { transform: translateX(20px); }

/* 列表项（iOS 18 标准：44px 最小高度，0.5px 分隔线） */
.list-item {
    display: flex; align-items: center; justify-content: space-between;
    padding: 12px 16px; min-height: 44px;
    border-bottom: 0.5px solid var(--color-separator);
    transition: background 0.15s var(--ease);
}
.list-item:active { background: rgba(0, 0, 0, 0.06); }

/* 输入框（iOS 18 标准：系统 fill 背景，10px 圆角） */
.input {
    width: 100%; padding: 12px 16px;
    font-size: 17px; color: var(--color-label);
    background: var(--color-fill);
    border: none; border-radius: 10px; outline: none;
    transition: background 0.2s var(--ease);
}
.input:focus { background: rgba(120, 120, 128, 0.18); }
```

### 9.8 快捷键体系

| 快捷键 | 动作 |
|--------|------|
| `⌘K` | 命令面板 |
| `⌘N` | 新建会话 |
| `⌘\` | 切换右侧面板 |
| `⌘1-5` | 切换导航页 |
| `⌘,` | 打开设置 |
| `Shift+Enter` | 发送消息 |
| `Esc` | 中断生成/关闭弹窗 |

## 10. 特色功能详细设计（Phase 1 部分）

> 注：§10 章节分散在三个文件——本文件为 §10.4（Skill）/§10.6（工作流引擎+模板）/§10.7（记忆）/§10.8（文件）；
> §10.1-10.3/10.5/10.9/10.11-10.13 见 `phase3-extend.md`；§10.10 见 `phase2-panel.md`。

### 10.4 Skill 技能系统详细设计

**目录布局**：

```
{app_data}/skills/{folderName}/
├── SKILL.md               # 技能定义（frontmatter: name/description/tags/author）
├── README.md              # 可选
├── scripts/               # 可选脚本
└── assets/                # 可选资源
```

**安装流程**（`skill:install`）：

```
source 解析:
  skills.sh:{id}       → GET https://skills.sh/api/skills/{id} → 下载 zip → 解压
  github:owner/repo    → clone → 定位技能目录 → 复制
  zip                  → 本地 zip 解压
  local:{path}         → 本地目录加载（不入库）

1. 解压/复制到临时目录 → 校验 SKILL.md 存在
2. 解析 frontmatter
3. 复制到 {app_data}/skills/{folderName}/（重名 .bak 备份覆盖）
4. 计算 content_hash（SKILL.md sha256）
5. 写 skills 表；source=builtin 时对所有 agent 默认启用
6. 清理临时目录
```

**加载与注入**（会话构建时）：

```rust
// core/adk/prompt.rs
pub struct PromptBuilder { ... }

impl PromptBuilder {
    /// 组装系统提示：base system + 启用技能 + 记忆 + Wiki 上下文
    pub async fn build(&self, agent: &Agent, session: &Session, memory: &dyn MemoryStore) -> Result<String, AppError> {
        let mut parts = vec![];
        if let Some(sp) = &agent.system_prompt { parts.push(sp.clone()); }
        for skill in self.enabled_skills(&agent.id).await? {
            parts.push(format!("\n---\n# Skill: {}\n{}\n", skill.name, skill.content()));
        }
        let mem = memory.build_context(&session.id, &agent.id).await?;
        if !mem.summary.is_empty() {
            parts.push(format!("\n---\n# 历史会话摘要\n{}", mem.summary));
        }
        Ok(parts.join("\n"))
    }
}
```

**市场搜索**（三源聚合）→ **完整设计见 phase2-panel.md §10.4.1-10.4.4**（Phase 2，T9 补充）。

Phase 1 仅实现技能安装/卸载/启停/注入（本节约 10.4 主体）；市场三源搜索、去重排序、版本检测在 Phase 2 落地。


### 10.6 多 Agent 工作流详细设计

**预置工作流**（首次启动写入）：

| 工作流 | 阶段 | 说明 |
|--------|------|------|
| 深度研究 | researcher → analyst → writer | 搜索 → 分析 → 成文 |
| 代码审查 | reader → reviewer → fixer | 读代码 → 审查 → 修复建议 |
| 头脑风暴 | diverge → converge → critic | 发散 → 收敛 → 批判 |
| 翻译校对 | translator → proofreader | 翻译 → 校对 |

**执行引擎**：

```rust
pub struct WorkflowEngine { coordinator: Arc<Coordinator>, events: EventEmitter }

impl WorkflowEngine {
    pub async fn run(&self, wf: Workflow, inputs: Value, run_id: &str) -> Result<WorkflowResult, AppError> {
        let mut outputs = HashMap::new();
        for stage in topological_sort(&wf.stages)? {     // 按 depends_on 排序
            let ctx = render_template(&stage.prompt_template, &inputs, &outputs);
            let reply = self.coordinator.dispatch(&stage.role, AgentTask {
                prompt: ctx, tools: stage.tools.clone(),
            }).await?;
            outputs.insert(stage.id.clone(), reply.output);
            self.events.stage_done(run_id, &stage.id, &reply.output);
        }
        Ok(WorkflowResult { run_id: run_id.into(), outputs })
    }
}
```

**任务调度**：tokio 任务池（默认 4 worker）+ `Semaphore` 限制并发 LLM 调用数。

#### 10.6.1 阶段模板系统（详细设计）🟧 Phase 2

**模板格式**（JSON 存储在 `workflows` 表 definition 字段；预置模板编译期内嵌为 Rust 常量，首次启动写入）：

```rust
/// 阶段模板 = 可复用阶段定义（预置 + 用户保存）
/// 与 TaskStageDef（§9.9.1，见 phase2-panel.md）/ WorkflowStage（§3.4）的关系：
///   - StageTemplate = 可复用的「阶段单元」（预置/用户保存，落库 stage_templates 表）
///   - TaskStageDef  = 画布上的「任务阶段」= StageTemplate 字段 + agent_id + depends_on + reflection
///   - WorkflowStage = 运行时「执行阶段」= TaskStageDef 的依赖部分（role/prompt_template/tools/depends_on）
///   三者字段已对齐（§9.9.1 TaskStageDef 为超集，见 phase2-panel.md），task:run 时 TaskStageDef → WorkflowStage 直接映射。
#[derive(Serialize, Deserialize, Clone)]
pub struct StageTemplate {
    pub id: String,                 // "research"
    pub name: String,               // "资料搜集"
    pub role: String,               // 角色标识（供 Coordinator 匹配 AgentActor）
    pub description: String,
    pub prompt_template: String,    // 提示模板（含 {{input}} / {{stage.x.output}} 占位符）
    pub tools: Vec<String>,         // 工具白名单
    pub max_iterations: u32,        // 工具循环上限
    pub model_hint: Option<String>, // 模型建议（如 "plan" = 规划模型；空 = Agent 默认）
    pub output_spec: Option<String>,// 输出格式约定（如 "markdown" / "json"），注入提示
    pub reflection: Option<ReflectionConfig>, // 反思配置（§10.9，见 phase3-extend.md，可空）
}

/// 预置工作流定义（= 阶段模板的有序组合 + 输入声明）
pub struct BuiltinWorkflow {
    pub id: String,                 // "deep-research"
    pub name: String,
    pub inputs: Vec<TaskInput>,     // 复用 §9.9.1 TaskInput 定义（见 phase2-panel.md）
    pub stages: Vec<StageTemplate>,
}
```

#### 10.6.2 预置工作流完整模板

**① 深度研究（deep-research）**

```json
{
  "id": "deep-research",
  "name": "深度研究",
  "inputs": [
    { "key": "topic", "label": "研究主题", "kind": "Text", "required": true },
    { "key": "depth", "label": "深度", "kind": "Select",
      "options": ["快速概览", "标准", "深度"], "default": "标准" }
  ],
  "stages": [
    {
      "id": "stage1", "name": "资料搜集", "role": "researcher",
      "prompt_template": "研究主题：{{topic}}\n深度：{{depth}}\n\n请使用网络搜索工具全面搜集与该主题相关的资料，覆盖：背景、关键概念、主要参与方、最新进展。输出带来源链接的资料汇编（Markdown）。",
      "tools": ["web_search", "knowledge_lookup", "read_file"],
      "max_iterations": 15,
      "output_spec": "markdown（含来源链接列表）"
    },
    {
      "id": "stage2", "name": "分析综合", "role": "analyst",
      "prompt_template": "基于以下资料进行深度分析：\n\n{{stage1.output}}\n\n请从多角度交叉验证，指出观点分歧、数据矛盾，给出综合结论与关键洞察。输出结构化分析（Markdown，含「关键结论」「争议点」「证据强度」小节）。",
      "tools": ["knowledge_lookup", "read_file"],
      "max_iterations": 10,
      "depends_on": ["stage1"],
      "output_spec": "markdown"
    },
    {
      "id": "stage3", "name": "成文", "role": "writer",
      "prompt_template": "基于分析撰写最终研究报告：\n\n{{stage2.output}}\n\n要求：结构清晰（摘要/正文/结论/参考）、语言精炼、保留关键来源。输出完整 Markdown 报告。",
      "tools": [],
      "max_iterations": 5,
      "depends_on": ["stage2"],
      "output_spec": "完整 markdown 报告"
    }
  ]
}
```

**② 代码审查（code-review）**

```json
{
  "id": "code-review", "name": "代码审查",
  "inputs": [
    { "key": "workdir", "label": "目标目录", "kind": "Text", "required": true },
    { "key": "focus", "label": "关注点", "kind": "Text", "default": "正确性、安全、性能" }
  ],
  "stages": [
    { "id": "stage1", "name": "代码通读", "role": "reader",
      "prompt_template": "通读工作目录 {{workdir}} 的代码（重点：入口、核心模块、变更文件）。输出：项目结构概览 + 关键文件清单（含行数与职责）。",
      "tools": ["read_file", "list_dir", "grep_search"], "max_iterations": 20 },
    { "id": "stage2", "name": "问题审查", "role": "reviewer",
      "prompt_template": "针对关注点「{{focus}}」审查以下代码：\n\n{{stage1.output}}\n\n逐文件列出：严重度（critical/major/minor）、问题描述、位置（file:line）、修复建议。输出 Markdown 审查报告。",
      "tools": ["read_file", "grep_search"], "max_iterations": 20, "depends_on": ["stage1"] },
    { "id": "stage3", "name": "修复建议", "role": "fixer",
      "prompt_template": "基于审查报告给出可执行修复方案：\n\n{{stage2.output}}\n\n对每个 major+ 问题给出具体代码片段（diff 形式优先）。输出「修复补丁」Markdown。",
      "tools": ["read_file"], "max_iterations": 10, "depends_on": ["stage2"] }
  ]
}
```

**③ 头脑风暴（brainstorm）**

```json
{
  "id": "brainstorm", "name": "头脑风暴",
  "inputs": [
    { "key": "topic", "label": "主题", "kind": "Text", "required": true },
    { "key": "count", "label": "候选数", "kind": "Number", "default": 10 }
  ],
  "stages": [
    { "id": "stage1", "name": "发散", "role": "diverge",
      "prompt_template": "针对「{{topic}}」发散出至少 {{count}} 个候选方案/创意，覆盖不同角度（技术、商业、用户体验）。不评判优劣，只列点子。",
      "tools": ["web_search"], "max_iterations": 8 },
    { "id": "stage2", "name": "收敛", "role": "converge",
      "prompt_template": "对以下候选进行收敛归类：\n\n{{stage1.output}}\n\n合并相似项，按可行性/价值/风险三维度初筛，保留 Top 5 并说明理由。",
      "tools": [], "max_iterations": 5, "depends_on": ["stage1"] },
    { "id": "stage3", "name": "批判", "role": "critic",
      "prompt_template": "对 Top 5 候选逐一提出批判性意见（漏洞、成本、反方观点）：\n\n{{stage2.output}}\n\n最终输出：每个候选的「优点/缺点/风险/改进建议」表。",
      "tools": [], "max_iterations": 5, "depends_on": ["stage2"] }
  ]
}
```

**④ 翻译校对（translate-proofread）**

```json
{
  "id": "translate-proofread", "name": "翻译校对",
  "inputs": [
    { "key": "text", "label": "待翻译文本", "kind": "Textarea", "required": true },
    { "key": "source", "label": "源语言", "kind": "Text", "default": "auto" },
    { "key": "target", "label": "目标语言", "kind": "Text", "required": true }
  ],
  "stages": [
    { "id": "stage1", "name": "初译", "role": "translator",
      "prompt_template": "将以下文本从 {{source}} 翻译为 {{target}}，保留代码与专有名词，只输出译文：\n\n{{text}}",
      "tools": [], "max_iterations": 3 },
    { "id": "stage2", "name": "校对", "role": "proofreader",
      "prompt_template": "校对以下译文，修正术语不一致、漏译、生硬表达，输出终稿：\n\n{{stage1.output}}",
      "tools": [], "max_iterations": 3, "depends_on": ["stage1"] }
  ]
}
```

#### 10.6.3 模板变量解析（render_template）

```rust
pub fn render_template(template: &str, inputs: &Value, outputs: &HashMap<String, Value>) -> Result<String, AppError> {
    // 支持语法:
    //   {{topic}}              → inputs.topic
    //   {{stage1.output}}      → outputs["stage1"]（依赖阶段结果）
    //   {{stage1.output|truncate:2000}}  → 截断（防上下文溢出）
    //   {{stage1.output|lines:50}}       → 只取前 50 行
    let mut out = template.to_string();
    for (k, v) in inputs.as_object().unwrap_or(&Map::new()) {
        out = out.replace(&format!("{{{{{k}}}}}"), &value_to_str(v));
    }
    // 依赖输出替换（先解析全部依赖已存在的，缺失 → 校验错误）
    for (id, val) in outputs {
        out = out.replace(&format!("{{{{{id}.output}}}}"), &value_to_str(val));
    }
    // 管道过滤器处理（split on |）
    Ok(out)
}
```

**校验规则**（`validate_definition`，§9.9.1 `task:validate` 复用，见 phase2-panel.md）：

- 模板中引用的 `{{stage.x.output}}` 必须存在 `depends_on` 依赖（或为前序阶段）
- 变量引用缺失 → 构建期报错（带缺哪个变量）
- 阶段图环检测（拓扑排序失败 → 拒绝）
- 每阶段输出注入下一阶段前做 `truncate:8000` 上限保护

#### 10.6.4 模板管理与用户自定义 🟧 Phase 2

| 操作 | 说明 |
|------|------|
| 预置模板 | 内嵌常量，只读；首次启动写入 workflows 表 `source=builtin` |
| 用户模板 | `task:save-template`（§9.9.1，见 phase2-panel.md）保存为 `source=user`，可编辑/删除 |
| 阶段模板复用 | 用户可保存单个 StageTemplate 到 `stage_templates` 表，编排时拖入 |
| 模板继承 | 用户模板可基于预置修改（复制 → 改 inputs/stages） |

```sql
-- 迁移 007_workflow_templates.sql
CREATE TABLE stage_templates (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    role          TEXT NOT NULL,
    description   TEXT,
    prompt_template TEXT NOT NULL,
    tools         TEXT NOT NULL DEFAULT '[]',
    max_iterations INTEGER DEFAULT 10,
    model_hint    TEXT,                          -- 模型建议（§10.6.1）
    output_spec   TEXT,                          -- 输出格式约定（§10.6.1）
    reflection    TEXT,                          -- 反思配置 JSON（§10.9，见 phase3-extend.md，可空）
    source        TEXT NOT NULL DEFAULT 'user',   -- builtin | user
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
```

### 10.7 记忆系统

**设计参考**：MiMo-Code 记忆架构（`src/memory/paths.ts` 4 scope + 9 type、checkpoint-writer 子 agent 唯一策展、SQLite FTS5 索引 + BM25 搜索、主动召回协议、写入沙箱、校验重试）。本设计移植为 Rust + Tauri 实现。

#### 10.7.1 分层与存储路径

```
{app_data}/memory/
├── global/MEMORY.md                 # 全局记忆：跨项目用户偏好/规则
├── projects/{pid}/MEMORY.md         # 项目记忆：pid = 仓库绝对路径 sha256 前 12 位
│   └── MEMORY-{topic}.md           # 溢出文件（某节超预算时）
└── sessions/{sid}/
    ├── checkpoint.md                # 会话检查点（11 节结构，writer 专属）
    ├── checkpoint-{topic}.md        # 溢出文件
    ├── notes.md                     # 会话草稿本（合法 scratchpad）
    └── tasks/{task_id}/
        └── progress.md              # 任务进度（子 agent 汇报）
```

**Scope 定义**（对齐 MiMo-Code `memory/paths.ts`）：

| Scope | 路径模式 | 内容 | 写入者 | 注入时机 |
|-------|---------|------|--------|----------|
| `global` | `global/MEMORY.md` | 跨项目偏好/规则 | 主 agent 可编辑 | 会话构建时 |
| `projects` | `projects/{pid}/MEMORY.md` | 项目规则/架构决策/发现 | 主 agent 可编辑 | 会话构建时（pid 匹配） |
| `sessions` | `sessions/{sid}/checkpoint.md` | 会话状态（11 节） | **checkpoint-writer 专属** | 上下文重建时 |
| `sessions` | `sessions/{sid}/notes.md` | 会话草稿 | 主 agent | 上下文重建时 |
| `sessions` | `sessions/{sid}/tasks/{tid}/progress.md` | 子任务进度 | 子 agent 汇报 | 任务引用时 |

**Type 自动检测**（从文件路径模式推断）：

```rust
fn detect_type(path: &str) -> MemoryType {
    if path.ends_with("/checkpoint.md") || path.starts_with("checkpoint-") {
        MemoryType::Checkpoint
    } else if path.contains("/tasks/") && path.ends_with("/progress.md") {
        MemoryType::Progress
    } else if path.ends_with("/notes.md") {
        MemoryType::Notes
    } else if path.ends_with("/MEMORY.md") || path.starts_with("memory-") {
        MemoryType::Memory
    } else {
        MemoryType::Free
    }
}
```

**Project ID 生成**（对齐 MiMo-Code `resolveProjectId`）：

```rust
fn resolve_project_id(repo_path: &str) -> String {
    let hash = sha256(repo_path.as_bytes());
    hex::encode(&hash[..6])  // 前 12 位十六进制
}
```

**记忆层级**（对齐 MiMo-Code 的 scope + type 模型）：

| scope | type | 内容 | 写入者 | 注入时机 |
|-------|------|------|--------|----------|
| `global` | memory | 跨项目偏好/规则 | 主 agent 可编辑 | 会话构建时 |
| `projects` | memory | 项目规则/架构决策/发现 | 主 agent 可编辑 | 会话构建时（工作目录匹配 pid） |
| `projects` | notes | 项目草稿 | 主 agent | 按需 |
| `sessions` | checkpoint | 会话状态（11 节） | **checkpoint-writer 专属** | 上下文重建时 |
| `sessions` | notes | 会话草稿（合法 scratchpad） | 主 agent | 上下文重建时 |
| `sessions` | task-progress | 子任务进度 | 子 agent 汇报 | 任务引用时 |
| `cc` | - | Claude Code 记忆（可选索引） | 外部 | 可关闭 |

#### 10.7.2 存储实现（SQLite FTS5 索引 + Markdown 文件）🟧 Phase 2

```sql
-- 迁移 006_memory.sql — 记忆 FTS5 虚拟表（可执行 DDL）
CREATE VIRTUAL TABLE memory_fts USING fts5(
    body,                          -- 记忆文件正文
    fingerprint,                   -- 指纹（path+mtime+size）
    scope UNINDEXED,               -- global|projects|sessions|cc
    type UNINDEXED,                -- memory|notes|checkpoint|progress|free
    path UNINDEXED,                -- 文件绝对路径
    tokenize='unicode61'           -- Unicode 分词（CJK 安全）
);
-- 索引回填（reconcile 时执行）：INSERT INTO memory_fts(body, fingerprint, scope, type, path) SELECT ... FROM 磁盘文件
```

```rust
// data/services/memory/store.rs
pub struct MemoryStoreImpl {
    db: Database,
    base_dir: PathBuf,                     // {app_data}/memory
    fts: RwLock<FtsIndex>,                 // 内存中 FTS5 句柄
}

impl MemoryStoreImpl {
    /// 写入/更新一个记忆文件（Markdown），并增量索引 FTS
    pub async fn upsert(&self, scope: MemoryScope, key: &str, body: &str) -> Result<(), AppError> {
        let path = self.resolve_path(scope, key)?;       // 路径校验（防穿越）
        tokio::fs::write(&path, body).await?;
        self.reconcile_file(scope, &path).await?;        // 增量索引
        Ok(())
    }

    /// 扫描磁盘 → 与 FTS 表 diff → 增删失效行（对齐 MiMo-Code reconcile.ts）
    pub async fn reconcile(&self) -> Result<(), AppError> {
        // 1. 遍历 {app_data}/memory/** 与 {workdir}/.prism/*.md
        // 2. 计算每文件 fingerprint（path + mtime + size）
        // 3. INSERT OR REPLACE 新增/变更文件；删除磁盘上已不存在的行
        Ok(())
    }

    /// BM25 搜索（对齐 MiMo-Code memory tool 语义）
    pub async fn search(&self, query: &str, opts: SearchOpts) -> Result<Vec<MemoryHit>, AppError> {
        let tokens: Vec<String> = tokenize(query);        // 去除标点，取 alnum 片段
        let mut sql = String::from("SELECT body, scope, path, bm25(memory_fts) AS score FROM memory_fts WHERE memory_fts MATCH ?");
        let match_expr = tokens.join(" OR ");
        let rows = sqlx::query(&sql).bind(match_expr).fetch_all(&self.db.pool).await?;
        // 分数归一化 + 相对下限过滤 + scope/type 过滤 → 排序返回
        Ok(rows)
    }
}
```

**搜索细节**（对齐 MiMo-Code `memory/service.ts` + `memory/fts-query.ts`）：

- **token 化**：`query` 按 `[\p{L}\p{N}_]+`（Unicode，CJK 安全）切分 → 每个 token 用短语引号包裹 → **OR 连接**（最大化召回）
- **过度获取**：fetch `limit * 3`（上限 50）行，再过滤
- **相对分数下限**：`score >= max_score * 0.15` 才保留（滤掉低相关噪音）
- **BM25 方向**：BM25 返回 lower = better，取反为 higher = better
- **scope/type 过滤**：默认全 scope；支持 `scope=projects`、`type=checkpoint` 等精确过滤
- **命中即权威**：返回的路径可直接 Read 全文（snippet 只展示前 ~200 字符）

#### 10.7.3 checkpoint-writer 策展机制（核心，移植 MiMo-Code）🟧 Phase 2

**角色**：checkpoint-writer 是一个独立子 agent（Rust 内通过 AutoAgents Actor 实现），是会话 checkpoints 的**唯一策展人**。

```
触发条件（上下文使用率阈值，默认 20% / 40% / 60% / 80%）：
  ├─ 达到阈值 → 唤醒 writer
  ├─ 用户显式触发（/checkpoint 命令）
  └─ 会话结束（summary 写入）

writer 执行：
  1. 读取本会话对话原文（messages 表，role 过滤）
  2. 生成/更新 checkpoint.md（11 节固定结构，见 10.7.3 节预算）
  3. 提炼新知识 → 追加/更新 MEMORY.md（Rules / Architecture decisions / Discovered durable knowledge）
  4. 清理过期任务（done/abandoned 归档）

约束：
  - 主 agent 不得直接写 checkpoint.md（仅可编辑 MEMORY.md 规则类 + notes.md）
  - writer 每次运行有 token 预算（如 8K），超限拆分
```

**checkpoint.md 11 节结构**（对齐 MiMo-Code `checkpoint-templates.ts`）：

```markdown
# Session Checkpoint
## 1. Active intent          — 当前会话目标（≤500 tokens）
## 2. Next action           — 下一步（≤1000 tokens）
## 3. Directives            — 用户指令/优先级（≤800 tokens）
## 4. Task tree             — 任务树（含状态）（≤1000 tokens）
## 5. Current work          — 正在进行的任务详情（≤2000 tokens）
## 6. Files                 — 涉及文件（≤1500 tokens）
## 7. Learnings             — 学到的知识（≤2000 tokens）
## 8. Errors                — 遇到的错误/教训（≤1500 tokens）
## 9. Live resources        — 运行中的资源（≤1000 tokens）
## 10. Design decisions     — 设计决策记录（≤3000 tokens）
## 11. Open notes           — 未决问题（≤800 tokens）
```

**节预算与溢出机制**（对齐 MiMo-Code spillover，统一配置见 §13.1 `TokenBudget`）：

| 节 | 预算 | 溢出目标 |
|----|------|---------|
| §1 Active intent | 500 tokens | 不溢出（截断） |
| §2 Next action | 1000 tokens | 不溢出（截断） |
| §3 Directives | 800 tokens | MEMORY.md Rules |
| §4 Task tree | 1000 tokens | 不溢出（截断） |
| §5 Current work | 2000 tokens | 不溢出（截断） |
| §6 Files | 1500 tokens | 不溢出（截断） |
| §7 Learnings | 2000 tokens | MEMORY.md Discovered |
| §8 Errors | 1500 tokens | 不溢出（截断） |
| §9 Live resources | 1000 tokens | 不溢出（截断） |
| §10 Design decisions | 3000 tokens | `checkpoint-{topic}.md` |
| §11 Open notes | 800 tokens | `checkpoint-{topic}.md` |

溢出格式：在原节写 `- See checkpoint-{topic}.md (N entries)` + 在溢出文件写完整内容。

**MEMORY.md 4 节结构**（对齐 MiMo-Code `MEMORY_TEMPLATE`）：

```markdown
# Project memory
_Durable project-level knowledge. Persists across all sessions in this project._

## Project context            — 项目是什么（≤1000 tokens）
## Rules                      — 硬约束（≤2000 tokens）
## Architecture decisions     — 设计选择 + 理由（≤3000 tokens）
## Discovered durable knowledge — 跨会话持久事实（≤4000 tokens）
```

**校验与重试机制**（对齐 MiMo-Code `checkpoint-validator.ts`）：

```rust
pub enum CheckpointViolation {
    TopicMissing,                    // 缺少 "Topic:" 行
    TopicTooLong,                    // > 80 字符
    SubsectionMissing(String),       // 必要子节缺失
    SubsectionOutOfOrder,            // 节顺序错误
    DiscoveredDuplicateTitle(String), // §7 标题重复
    DiscoveredMissingWhy,            // §7 缺少 "Why:" 行
    DiscoveredMissingHowToApply,     // §7 缺少 "How to apply:" 行
    NextFiller,                      // §2 仅为 "continue"/"resume" 等
    BudgetExceeded,                  // 总 token 超预算
    SectionBudgetExceeded(String),   // 单节 token 超预算
}

pub fn validate_checkpoint(content: &str) -> Vec<CheckpointViolation> { ... }

/// 校验失败时：重命名 checkpoint.md → checkpoint.invalid.md，通知 writer 重试
pub fn quarantine_checkpoint(sid: &str) -> Result<(), AppError> { ... }
```

**notes.md 草稿本**：主 agent 的合法 scratchpad（引用/未决问题/跨项目观察），writer 在 checkpoint 时整理归纳进对应节。

#### 10.7.4 注入与召回（Active Recall）🟧 Phase 2

**上下文重建注入**（对齐 MiMo-Code 的注入分段，token 预算可配置，与 §13.1 TokenBudget 对齐）：

| 段 | 内容 | 预算（默认） |
|----|------|--------------|
| checkpoint.md | 会话检查点（全量或预算截断） | 11K tokens |
| MEMORY.md（project） | 项目记忆 | 10K tokens |
| MEMORY.md（global） | 全局记忆 | 6K tokens |
| notes.md | 会话草稿 | 6K tokens |
| tasks/*/progress.md | 进行中任务的进度 | 2K tokens |
| recent_user | 最近用户输入（FIFO，单条 ≤2K） | 16K tokens |

**注入标记**：截断时标注 `⚠️ Truncated at ~N tokens. Read(<path>, offset=L) for the rest.`，主 agent 按需 Read 尾部。

**主动召回协议**（系统提示中的指令）：

```
- 上下文重建后，checkpoint/MEMORY/notes 已在上下文中 → 勿重复 Read
- 记忆条目中的路径/函数名是写入时的快照，使用前先验证
- 未记录的信息不要臆断：先 memory:search，再决定是否问用户
```

**记忆工具命令**：

| 命令 | 参数 | 返回 |
|------|------|------|
| `memory:search` | `{query, scope?, type?, limit?}` | `Vec<MemoryHit>` |
| `memory:read` | `{path}` | `{body}` |
| `memory:write` | `{scope, key, body}` | `()` | 仅主 agent 规则类 |
| `memory:append-notes` | `{scope, entry}` | `()` | 追加 notes.md |
| `memory:reconcile` | `{}` | `{indexed, pruned}` | 手动全量重建索引 |
| `memory:context-dump` | `{}` | `Vec<MemoryDump>` | 当前注入记忆摘要（调试用） |

#### 10.7.5 记忆前端（设置页 → 记忆管理）🟩 Phase 3

```
┌─ 记忆管理 ───────────────────────────────┐
│ [全局] [项目] [会话] [搜索]               │  ← 4 Tab
│ 全局记忆 MEMORY.md：                      │
│ ┌────────────────────────────────────┐  │
│ │ # 全局记忆                          │  │
│ │ 可编辑 Markdown（语法高亮）          │  │
│ │ [保存]                             │  │
│ └────────────────────────────────────┘  │
│ 索引状态: 23 文件 · 上次 reconcile: 2min │
│ [↻ 重建索引]                            │
└──────────────────────────────────────────┘
```

- 可编辑全局/项目 MEMORY.md（主 agent 权限一致）
- 会话 checkpoint 只读展示（writer 生成）
- 搜索 Tab：`memory:search` 结果列表 → 点击 Read 全文
#### 10.7.6 写入沙箱（Write Security）🟧 Phase 2

**来源**：MiMo-Code `tool/memory-path-guard.ts` — 不同 agent 有不同的记忆写入权限。

```rust
pub enum WriteSandbox {
    /// checkpoint-writer：只能写入 memory 树下的特定文件
    CheckpointWriter,
    /// dream/distill：可写入 memory 树 + 工作目录 .prism/
    DreamDistill,
    /// 主 agent：完整记忆写入权限（但不能写 tasks/*）
    MainAgent,
    /// 子 agent：只能写 tasks/{TID}/*.md（且 TID 必须匹配）
    SubAgent { task_id: String },
}

/// 写入校验：检查路径是否在当前 agent 的沙箱内
pub fn assert_memory_write_allowed(
    agent: &WriteSandbox, path: &Path, worktree: &Path,
) -> Result<(), AppError> {
    match agent {
        WriteSandbox::CheckpointWriter => {
            // 只允许写入：
            // - projects/{pid}/MEMORY.md (或 memory-{topic}.md)
            // - sessions/{sid}/checkpoint.md (或 checkpoint-{topic}.md)
            // - sessions/{sid}/notes.md
            // - sessions/{sid}/tasks/{tid}/*.md
            if !is_checkpoint_writer_path(path) {
                return Err(AppError::Forbidden("checkpoint-writer 只能写入记忆树".into()));
            }
        }
        WriteSandbox::MainAgent => {
            // 允许写入 memory 树，但不能写 tasks/*（那是 writer 的领域）
            if path.contains("/tasks/") {
                return Err(AppError::Forbidden("主 agent 不能写入 tasks/".into()));
            }
        }
        WriteSandbox::SubAgent { task_id } => {
            // 只能写 tasks/{task_id}/*.md
            if !path.ends_with(&format!("/tasks/{}/progress.md", task_id)) {
                return Err(AppError::Forbidden("子 agent 只能写入自己的任务进度".into()));
            }
        }
        WriteSandbox::DreamDistill => {
            // memory 树 + .prism/ 目录
        }
    }
    Ok(())
}
```

#### 10.7.7 主动召回注入（Active Recall）🟧 Phase 2

**来源**：MiMo-Code 在每条用户消息后注入记忆召回提示。

```rust
/// 在上下文重建时，向最后一条用户消息追加召回提示
pub fn inject_active_recall(session_dir: &Path) -> String {
    format!(
        "This session has memory at {session_dir}/. Recall content\n\
         not in your context with:\n\
         - memory({{ operation: \"search\", query: \"<keyword>\" }})\n\
         - Read(file_path=\"{session_dir}/checkpoint.md\")\n\
         - task({{ operation: \"list\" }})\n\
         Don't ask the user about something memory may already record."
    )
}
```

**注入规则**：
- 仅当记忆文件存在时注入（`has_memory_or_tasks` 检查）
- 追加到**最后一条用户消息**的尾部（作为合成文本）
- 每轮对话都注入（确保 agent 始终知道记忆可用）

**事件**：`memory:changed`（文件被 writer/agent 更新后广播，前端记忆面板刷新）。


### 10.8 文件与附件

- `file:pick` 使用 Tauri dialog 插件
- `file:parse` 支持 txt/md/pdf/doc/docx/html/json/csv/xml → 文本（`pdf-extract`、`docx-rs`、`scraper`、`html2md`）
- 对话附件：解析后作为 user 消息 attachments 元数据，注入 prompt 或走 RAG

## 11. 错误处理与日志

### 11.1 统一错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("MCP server error: {0}")]
    McpServer(String),
    #[error("LLM provider error: {0}")]
    LlmProvider(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
        s.serialize_str(&self.to_string())
    }
}
```

### 11.2 日志

- `tracing` + `tracing-subscriber`：文件（`{app_data}/logs/prism.log`，按天轮转）+ 控制台
- 关键操作（agent CRUD、MCP 连接、技能安装）打 INFO；工具调用打 DEBUG（含参数脱敏）
- Tauri 命令层统一 `#[tracing::instrument]` 记录耗时

---

## 12. 安全设计

| 项目 | 方案 |
|------|------|
| API Key 存储 | AES-256-GCM 加密（密钥派生自用户级 `keyring` 或机器指纹），`api_key_enc` 列 |
| IPC 权限 | Tauri capabilities：按命令域配置 `allow`，前端 window 仅授权所需命令 |
| 前端 XSS | 内容一律走 Svelte 转义；Markdown 渲染前 `sanitize`（`ammonia`） |
| MCP 隔离 | 本地 MCP 进程低权限启动；工具参数大小限制（默认 1MB） |
| 路径安全 | 所有文件读写强制 `canonicalize` 后前缀校验，防目录穿越 |
| 日志脱敏 | 日志过滤 API Key / Token 模式（`sk-` 等） |
| 远程 MCP | OAuth 回调本地监听随机端口 + PKCE |
| 安全护栏 | 四层防御：输入过滤（注入检测/敏感词）→ Agent 约束（系统提示/工具权限）→ 输出过滤（毒性检测）→ 人工监督（审批/升级），详见 §10.12（见 phase3-extend.md） |
| 人机协同 | 工具审批分级（Low/Medium/High/Critical）+ 升级机制 + ToolApprovalDialog，详见 §10.10（见 phase2-panel.md） |
| SSRF 防护 | web 工具过滤 private/loopback IP 段 |

---

## 13. 性能设计

| 目标 | 指标 |
|------|------|
| 冷启动 | < 1s（Rust 原生，无 Electron 运行时） |
| 常驻内存 | < 120MB（对比 Electron ~300MB+） |
| 首 Token | < 100ms（流式管道零拷贝转发） |
| 包体 | < 20MB 安装包（对比 Electron ~150MB） |
| 并发 | 4 个并发 Agent 流不卡顿（tokio 多核） |

优化手段：
- 向量存储 BLOB 二进制（f32 小端）而非 JSON
- 嵌入结果 LRU 缓存（`moka` crate）
- 消息历史分页加载（每页 50 条）
- WebView 侧 `content-visibility: auto` 懒渲染长会话
- 代码高亮按需加载（shiki 动态 import）

### 13.1 上下文压缩（Context Compaction）

> **§13.1 完整设计见 phase3-extend.md**（Phase 3，T24）。本节在 Phase 1 仅定义性能基线，压缩机制在 Phase 3 落地。

> 涉及：ContextWindow / 压力等级 / 工具输出裁剪 / Head-Tail 选择 / 溢出恢复 / 微压缩 / TokenBudget 统一配置。

## 14. 旧版 prism-agent 经验与规避

**来源**：prism-agent 项目（Electron 版）长期开发的沉淀记忆——包括**已修复的缺陷**（直接规避）与**有效模式**（移植借鉴）。Rust 重写时逐条对照。

### 14.1 模型/Provider 配置类（必须规避）

| # | 旧版缺陷 | 后果 | 本版规避方案 |
|---|----------|------|--------------|
| 1 | **Provider ID 双轨漂移**：DB seeder 用 `'xiaomi'`，系统注册表用 `'mimo'` | 模型列表/端点解析失败（`PROVIDER_URLS['xiaomi']` → undefined），OAuth 失效 | Provider ID 单一来源：DB 主键 + 配置项统一一个常量表（Rust `enum ProviderId`，`FromStr` 校验），**禁止字符串散落** |
| 2 | **模型 ID 格式不一致**：models.dev 用点号（`mimo-v2.5`），应用侧用连字符（`mimo-v2-5`） | API 400 错误 | 模型 ID 直接存供应商返回的原值，不做格式转换；接入时 `model:test` 实测验证 |
| 3 | **模型列表靠静态注册表** | 供应商新增模型不生效，维护成本高 | `model:list` 从 `GET /v1/models` 动态拉取 + 本地缓存（5min TTL），注册表仅作元数据覆盖（价格/上下文窗口） |
| 4 | **Key 旋转逻辑的 JSON 类型陷阱**：Drizzle SQLite JSON mode 读出字符串 | `(row.apiKeys ?? []).filter` 崩溃 | Rust 侧 sqlx 显式类型：`api_keys TEXT` + serde 反序列化 + `Array::is_empty()` 防御；JSON 字段一律显式 parse 后使用 |
| 20 | **ID 归一化静默转换**：`canonOf` 两遍归一化把 `mimo-v2.5-pro` 静默变连字符 | 用户配置被悄悄改写、API 400 | 禁止对模型 ID 做任何自动归一化；只做 trim + 原值存储，校验交给 `model:test` |
| 21 | **模型列表获取失败无降级**：`NoOutputGeneratedError` 是 model 400 的下游症状 | 报错指向不明 | `model:list` 拉取失败时返回缓存 + `stale: true` 标记；错误链保留原始 provider 错误上下文 |
| 22 | **工具 applies 门控静默不激活**：ToolApplyScope 构造错误导致工具不生效且无提示 | 工具"消失"难排查 | 工具注册后立即 self-check：注入的 tool 数 = 期望数，不符则日志告警 |

### 14.2 会话/消息/异步流类（直接规避）

| # | 旧版缺陷 | 后果 | 本版规避方案 |
|---|----------|------|--------------|
| 5 | **upsert 不传 ID 生成重复消息**：`saveMessages()` 未传 `id: anchorMessageId`，placeholder 永驻 pending | 界面卡"正在准备回复" | Rust 消息写入强制显式 ID（`INSERT OR REPLACE` with id），无默认生成路径 |
| 6 | **AudioStreamManager 时序丢块**：renderer 先发 chunk，主进程 stream 后建 | 录音开头 1-2s 音频丢失 | §10.3.2（见 phase3-extend.md）已规避：先建 stream + `pending` 缓冲 flush |
| 7 | **异步迭代器空转**：无消费者时 `next()` 忙等 | CPU 占用 | Rust mpsc channel 天然阻塞，无忙等（Rust 语言优势，无需处理） |
| 8 | **流式响应断连无恢复** | WebView 刷新后流丢失 | §7 已设计：事件带 message_id + `chat:stream:aborted`，前端可重连恢复 |
| 23 | **流开始标记缺失**：`onChunk` 是类型过滤子集，漏掉 start 标记 | 前端不知道流开始了，状态机错乱 | 事件序列显式包含 `chat:stream:start`（§7.2 已设计），前端以 start/done/aborted 三事件为准 |
| 24 | **审批续跑缺失**：现有 IPC 只 patch 不续跑，审批后模型不再继续 | 工具审批后对话停住 | 工具审批流程状态机显式：tool_call → 等待审批 → 批准后回填 tool 消息 → 继续生成（§7.3 agentic loop 已含） |
| 25 | **流状态缓存写错侧**：stream 状态缓存必须由主进程 setShared，渲染端写无效 | 刷新后流状态丢失/空白 | 流状态一律后端管理（ChatService.active_streams），前端只读 |
| 26 | **编辑重发卡"正在回复"**：stop 流程未先清 state | 界面永久 loading | stop/abort 处理顺序：先清 active_streams 状态，再 emit aborted，最后前端清 UI 状态 |
| 27 | **fs.watch recursive 构造抛错（Linux）**：Windows 支持但 Linux 部分版本抛异常 | 文件监听崩溃 | 目录监听失败时降级为轮询（mtime 间隔 2s），不阻断功能（§9.10.7 fs:watch） |

### 14.3 存储与持久化类

| # | 旧版缺陷 | 后果 | 本版规避方案 |
|---|----------|------|--------------|
| 9 | **SQLite PK 迁移被 FK 阻塞**：直接 `UPDATE id = newId` 失败 | 迁移崩溃、app 起不来 | sqlx 迁移用 "insert → remap refs → delete" 模式；单条迁移 try/catch 隔离，失败不阻断启动 |
| 10 | **增量保存粒度不一**：会议 10 段落库、聊天全量 | 内存压力 | 统一策略：**每 N 事件或每 30s 落库一次**（可配置），长任务防丢失 |
| 11 | **混合存储原则缺失** | 大文件塞 DB 或小数据碎片化 | 小数据（元数据/文本）→ SQLite（事务安全）；大文件（音频/附件）→ 文件系统（流式读写）；沿用 §5 设计 |
| 28 | **迁移按假定唯一 ID 删除**：deepseek 迁移按 ID 删 UUID 旧行失败、复制旧 name | 迁移后脏数据残留 | 迁移匹配一律按业务复合键（provider_id + model_id），delete-first + insert；迁移版本号必须 bump，否则静默不生效 |
| 29 | **模型行 ID 双形态（UUID vs 标准 id）**：agent FK 引用错形态 → "Agent not found" | 关联查询失败 | Agent.model_id 统一存标准 id（非 UUID 行主键），查询时 left join 匹配 |
| 30 | **seeder 崩溃阻塞启动** | 应用起不来 | 启动 seed 逻辑 try/catch 隔离 + 每 seed 独立事务；失败打日志但继续启动 |
| 31 | **默认值回填缺失**：PreferenceSeeder 不回填已存值，改默认值不生效 | 老用户永远用旧默认 | 默认值变更用独立"一次性迁移"（版本号 bump），不依赖 seeder 回填 |
| 32 | **SQLite 保留字与查询陷阱**：`group` 等列名须引号；WAL 模式查库须含 `-wal` 文件 | 查询报错/查不到新数据 | Rust 侧列名避开保留字（snake_case 命名时注意）；测试用连接池统一入口，不用裸文件读取 |

### 14.4 有效模式（移植借鉴）

| # | 旧版模式 | 本版对应 |
|---|----------|----------|
| 12 | **每 Agent 可配置工作区**（per-session workspace：session → workspaceId → path join） | Agent 侧边栏"工作目录"Tab + PromptBuilder 注入路径 |
| 13 | **工具审批分级**：read/ls/glob/grep 自动放行，write/edit/delete 需审批（permission_mode） | §12 安全设计已含；MCP 工具权限校验沿用此分级 |
| 14 | **Agent 消息编辑 = 删除重建**（delete-and-replace） | 对话前端 regenerate/编辑采用"删旧消息 + 重新生成"模式 |
| 15 | **provider 模型 ID 种子校验**：`pnpm generate` 全量重生成 + CI 双向检查 | Rust 侧 `model:list` 动态拉取 + 测试断言，替代静态生成 |
| 16 | **侧边栏顺序偏好持久化**（`ui.sidebar.favorites` JSON 数组） | 面板/侧边栏布局顺序存入 `preferences`，可拖拽调整 |

### 14.5 开发环境与平台差异注意（三平台）

| # | 旧版教训 | 本版对应 |
|---|----------|----------|
| 17 | rolldown-vite 缓存导致旧 build 生效（改 main 进程代码必须清缓存） | Tauri 无此问题，但注意：改 Rust 后 `cargo build` 增量编译依赖特征；前端 Vite 缓存同类清理 |
| 18 | worktree 内 `pnpm install` 原生构建失败（Windows 缺 node-gyp 条件） | Rust/Tauri 用 `cargo` 无此问题；前端只用 `npm ci` |
| 19 | 日志分层：error 与 info 分文件（winston） | tracing 同构：`app-error.log` / `app.log` 双目标 + 按天轮转 |

**平台差异对照表**（开发与实现时必须逐项核对）：

| 关注点 | Windows | macOS | Linux | 处理 |
|--------|---------|-------|-------|------|
| WebView | WebView2 (Edge) | WKWebView | WebKitGTK | Tauri 2.x 自动选择，无需代码分支 |
| 应用数据目录 | `%APPDATA%\prism-agent\` | `~/Library/Application Support/prism-agent/` | `~/.local/share/prism-agent/` | `dirs` crate + `app_data_dir()`，禁止硬编码 |
| 本地 ASR 二进制（sherpa-onnx） | `sherpa-onnx.exe` | `sherpa-onnx` | `sherpa-onnx` | 按平台打包对应二进制（§10.3.1，见 phase3-extend.md）；未找到时降级提示 |
| LSP 可执行文件查找 | `where` 命令 | `which` | `which` | `std::process` 按 `cfg!(windows)` 分支选 `where`/`which`（§9.10.5） |
| 路径分隔符 | `\` | `/` | `/` | 一律 `std::path::PathBuf`，禁止字符串拼接路径 |
| 命令行工具调用 | `cmd /c` | `sh -c` | `sh -c` | 统一封装 `run_command(cmd, args)` 抽象（§10.3/§10.5 复用，见 phase3-extend.md） |
| 打包格式 | NSIS / MSI | .dmg | .deb / .rpm / AppImage | `tauri build` 三平台产物；CI 三平台矩阵（T1/T18） |
| 托盘/窗口行为 | 无差异 | 标题栏材质差异 | 无差异 | 前端不做平台分支；系统 chrome 由 Tauri 管理 |
| 字体链 | Segoe UI + 雅黑 | SF Pro / PingFang | Noto / 系统 | §9.2 字体链已按三平台降级 |
| 文件系统权限 | 无 | TCC 权限提示（麦克风/文件） | 依赖发行版 | 首次使用麦克风/目录时提示；macOS 需在 Info.plist 声明用途 |

### 14.6 IPC / 命令层 / 前端渲染类（必须规避）

| # | 旧版缺陷 | 后果 | 本版规避方案 |
|---|----------|------|--------------|
| 33 | **新增 IPC 通道漏同步**：通道名/preload/handler/服务/前端 5 处必须一致 | "is not a function" | Rust 侧 `#[tauri::command]` 单点定义 + 前端 `invoke()` 泛型封装；命令名以 `域:动作` 常量集中（§8.2 清单），禁止散落字符串 |
| 34 | **高频流数据用错通道**：音频块走 invoke（promise 开销/背压） | 音频丢帧、卡顿 | 高频流（音频块/流式增量）走 `listen/emit` 事件（§7.2、§10.3.2），invoke 仅用于请求-响应 |
| 35 | **事件订阅无清理**：Electron ipcRenderer 无 `off`，泄漏监听 | 内存泄漏、重复触发 | Tauri `listen()` 返回 unlisten 函数，组件卸载时清理（§7.5 前端封装已含） |
| 36 | **配置合并用 `partial \|\| default`**：partial 为 truthy 时丢弃默认字段 | 配置静默丢失 | 配置合并一律 `{...default, ...partial}`（Rust 侧用 serde 默认值 + 显式 merge） |
| 37 | **虚拟 ID 误查实体表**：agent-session 是虚拟 topicId，误查 topic 表 → NOT_FOUND | 错误指向不明 | 分层查询先辨类型（session 虚拟 id vs 实体 id），错误信息带上下文 |
| 38 | **幽灵页状态跨库共享**：selectedPagePath 切换库未重置 | 打开别的库显示旧选中 | 路由/面板切换时在源头重置选中态（§9.9/9.10 前端状态管理注意） |
| 39 | **autosave 切页丢数据**：flush 不在 effect cleanup | 编辑内容丢失 | 自动保存 flush 必须放组件卸载/切换的 cleanup（wiki 编辑器、指令编辑器） |
| 40 | **SWR 父子 hook 共享缓存键**：同 key 不同 enabled 互相污染 | 数据错乱 | Rust 侧无 SWR；前端 store 用 `$derived` 显式数据流，缓存键带完整参数 |
| 41 | **组件库下拉不渲染**：@cherrystudio/ui DropdownMenu 实测内容不渲染 | 菜单点了没反应 | 自建设计系统（§9.7）的 Popover/Dropdown 用原生实现 + 定位，避免第三方陷阱 |
| 42 | **不可见 provider 数据不完整**：设置页渲染全开 provider 崩溃 | 设置页崩 | 前端渲染前校验数据结构完整性；Provider 状态拉取失败显示占位而非崩溃 |

### 14.7 安全 / 文件系统 / 构建类（直接规避）

| # | 旧版缺陷 | 后果 | 本版规避方案 |
|---|----------|------|--------------|
| 43 | **目录穿越入口**：addSource 裸 join 拼接路径 | 任意文件读写 | 所有文件操作 `canonicalize` + 前缀校验（§12 已含）；工具层统一 `validate_path` wrapper |
| 44 | **SSRF 防护缺失**：web 工具可访问内网地址 | 内网探测 | `remote_url_safety`：过滤 private/loopback IP 段（§12 安全设计补充） |
| 45 | **删目录前未停 watcher**：fs watcher 事件复活已删目录 | 目录删不掉 | 删除前先 dispose watcher（§9.10.7 fs:watch 生命周期，见 phase2-panel.md） |
| 46 | **Windows 相对路径反斜杠**：断言/比较用字符串字面 | 测试失败、路径不匹配 | 一律 `std::path::Path` 操作；测试断言用 `path.join()` 构造预期（§14.5 已提） |
| 47 | **覆盖文件前未查 git 历史**：误删已提交的回归测试 | 静默丢代码 | 覆写/删除前 `git log -- <path>` 核对；Rust 项目同样适用（删除源文件前确认） |
| 48 | **缓存致旧代码生效**：rolldown-vite 不清缓存 | 改代码无效 | Tauri dev：改 Rust 后等 cargo 增量编译；前端 Vite 遇怪问题先清 `node_modules/.vite` |
| 49 | **CI 交叉编译/发布陷阱**：macOS 无法从 Windows 交叉编译；electron-builder 隐式 publish | 构建失败/误发布 | CI 三平台各自原生 runner；`tauri build` 显式 `--no-bundle` 或控制发布动作（§14.5） |
| 50 | **主进程测试需 mock 全局单例**：模块加载期执行 app.getPath | 测试崩 | Rust 侧依赖注入（AppState 注入 paths），测试用临时目录，无全局状态 |
| 51 | **批量写 JSON 覆写现有值**：按路径 setPath 把现有值置空 | 配置丢失 | 配置文件更新用读-改-写原子流程（temp + rename），禁止局部覆写 |

---

