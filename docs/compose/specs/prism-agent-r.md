---
feature: prism-agent-r
status: designed
updated: 2026-08-04
branch: main
commits: # filled at delivery
platform: windows | macos | linux
---

# Prism Agent R — Rust 重构版

> **平台定调：本项目为跨平台桌面应用，正式支持 Windows、macOS、Linux 三大桌面操作系统。**
> 所有功能（Agent 核心、面板、MCP、会议 ASR、Wiki/RAG、翻译/OCR、记忆系统）在三平台行为一致；
> 前端使用 WebView（Tauri 2.x 内置），后端 Rust 编译原生二进制（无 Node.js 运行时依赖）。
> 涉及平台差异的板块（路径处理、LSP 检测、本地 ASR 二进制、CI 构建矩阵、打包分发）已在本文档中明确标注，见 §14.5 与各相关章节。

## Report

## [S1] Problem

原 Prism Agent 基于 Electron + Node.js + React 构建，存在以下问题：

- **包体过大**：Electron 运行时 + Chromium ~150MB+，安装包臃肿
- **内存占用高**：Node.js + Chromium 常驻内存 ~300MB+，低配置机器卡顿
- **性能瓶颈**：Node.js 事件循环模型难以高效处理大规模并发 Agent 任务
- **类型安全缺失**：IPC 通信缺乏端到端类型检查，运行时错误难以排查
- **Agent 能力有限**：缺乏多 Agent 协作、工作流编排能力
- **生态依赖重**：Vercel AI SDK 等 JS 生态依赖多，锁版本困难

目标：用 **Rust 重写全部后端**（Tauri 2.x 壳），**Svelte 5 重写前端**，构建一个高性能、轻量级（包体 <15MB、内存 <100MB）、类型安全的 **跨平台（Windows / macOS / Linux）** AI Agent 平台，保留原项目的全部核心功能（Agent 系统、技能系统、MCP、LLM Wiki、RAG、会议纪要、翻译、OCR），并新增 **主页面板（多 Agent 总控制台 + 任务设计区）** 与 **Agent 运行时侧边栏**。

## [S2] Design

---

## 1. 架构总览

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Tauri 2.x Shell (WebView)                      │
├──────────────────────────────┬──────────────────────────────────────┤
│      Svelte 5 前端 (渲染层)   │        Rust 后端 (主进程核心)          │
│  ┌────────────────────────┐  │  ┌────────────────────────────────┐  │
│  │  Apple Design 系统      │  │  │  Agent 编排层 (AutoAgents)      │  │
│  │  ├ 设计令牌 (Design    │  │  │  ├ Actor 运行时                  │  │
│  │  │  Tokens)            │  │  │  ├ 工作流引擎 (Workflow)         │  │
│  │  ├ 基础组件 (Base)     │  │  │  └ 任务调度器 (Scheduler)        │  │
│  │  ├ 复合组件 (Composite)│  │  ├────────────────────────────────┤  │
│  │  └ 布局系统 (Layout)   │  │  │  Agent 核心层 (Rig)              │  │
│  ├────────────────────────┤  │  │  ├ Provider 适配器 (多厂商)      │  │
│  │  Codex 风格面板布局     │  │  │  ├ Agent 生命周期管理            │  │
│  │  ├ 左侧导航栏           │  │  │  ├ 流式 Token 管道              │  │
│  │  ├ 中央对话区           │  │  │  └ Tool 执行器                  │  │
│  │  ├ 右侧工具面板         │  │  ├────────────────────────────────┤  │
│  │  └ 底部状态栏           │  │  │  Agent 组件层 (ADK-Rust)        │  │
│  ├────────────────────────┤  │  │  ├ 模型抽象 (Model Trait)        │  │
│  │  Svelte 5 运行时        │  │  │  ├ 工具注册表 (Tool Registry)    │  │
│  │  ├ 状态 Store (Runes)  │  │  │  ├ 记忆系统 (Memory)             │  │
│  │  ├ 路由 (SvelteKit)    │  │  │  └ 提示工程 (Prompt Builder)     │  │
│  │  └ IPC 客户端封装       │  │  ├────────────────────────────────┤  │
│  └────────────────────────┘  │  │  MCP 协议层 (mcp-rust-sdk)       │  │
│                              │  │  ├ 运行时管理 (Runtime)          │  │
│                              │  │  ├ 传输层 (stdio/SSE/HTTP)       │  │
│                              │  │  └ 工具目录 (Catalog)            │  │
│                              │  ├────────────────────────────────┤  │
│                              │  │  数据层                          │  │
│                              │  │  ├ SQLite (sqlx + 异步)          │  │
│                              │  │  ├ RAG 引擎 (分块/嵌入/检索)      │  │
│                              │  │  └ 缓存服务 (内存 LRU)            │  │
│                              │  ├────────────────────────────────┤  │
│                              │  │  业务服务层                      │  │
│                              │  │  ├ Agent/Session/Message        │  │
│                              │  │  ├ Skill/Wiki/Meeting/File      │  │
│                              │  │  └ Translate/Ocr/Settings       │  │
│                              │  └────────────────────────────────┘  │
├──────────────────────────────┴──────────────────────────────────────┤
│              Tauri IPC Bridge (Commands / Events)                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 数据流（一次对话）

```
用户输入 → Svelte Composer → invoke('chat:send') → Rust ChatService
  → ADK PromptBuilder 组装系统提示（注入技能/记忆/Wiki上下文）
  → Rig Agent 流式调用 LLM
  → 循环：
    ├─ 收到 text delta → emit('chat:chunk', delta) → 前端渲染
    ├─ 收到 tool_call → ToolExecutor 执行（内置/MCP）
    │   └─ 结果回填 → Rig 继续生成
    └─ 收到 finish → emit('chat:done') → 前端收尾
  → 消息持久化 SQLite → 记忆系统更新
```

---

## 2. 技术选型

| 层级 | 技术 | 版本 | 用途 |
|------|------|------|------|
| 桌面框架 | Tauri | 2.x | 跨平台壳，Rust 原生后端 |
| 前端框架 | Svelte | 5.x (runes) | 轻量高性能 UI |
| 前端路由 | SvelteKit | 2.x (适配 static) | 路由与页面结构 |
| 构建工具 | Vite | 6.x | 前端打包 |
| LLM 框架 | rig | 0.9+ | 核心 LLM 集成 |
| Agent 编排 | autoagents | 0.x | Actor 模型多 Agent |
| Agent 组件 | adk-rust | 0.x | 模型/工具/记忆抽象 |
| MCP | mcp-rust-sdk | 0.x | MCP 客户端协议 |
| 数据库 | sqlx | 0.8+ | 异步 SQLite + 编译期 SQL 校验 |
| 异步运行时 | tokio | 1.x | 异步运行时 |
| HTTP | reqwest | 0.12 | LLM/外部 API 调用 |
| 序列化 | serde / serde_json | 1.x | 数据序列化 |
| 错误处理 | thiserror | 1.x | 类型化错误 |
| 日志 | tracing + tracing-subscriber | 0.1 | 结构化日志 |
| ID 生成 | uuid | 1.x | UUID v4 |
| 加密 | aes-gcm / argon2 | - | API Key 加密存储 |
| WebSocket | tokio-tungstenite | 0.2x | ASR/实时传输 |
| 前端 CSS | CSS 变量 + @layer | - | 设计令牌系统 |
| 前端动画 | 原生 CSS + Web Animations API | - | 毛玻璃/过渡 |

> 注：`mcp-rust-sdk` 若不够成熟，回退方案为基于 `rmcp` crate 或自研轻量 MCP 客户端（见 6.4）。

**跨平台支撑说明**：

- **Tauri 2.x** 官方支持 Windows / macOS / Linux 三平台，同一套 Rust 后端 + WebView 前端代码编译出三平台原生应用（无 Node.js 运行时依赖）
- **前端**：Svelte 5 编译为静态资源，由各平台 WebView 渲染（Windows = WebView2 / macOS = WKWebView / Linux = WebKitGTK）；CSS 使用 -apple-system 降级字体链（§9.2）保证三平台显示一致
- **路径处理**：统一使用 `std::path::PathBuf` + `dirs` crate 解析各平台应用数据目录（Windows `%APPDATA%` / macOS `~/Library/Application Support` / Linux `~/.local/share`）；代码中禁止硬编码 `/` 或 `\` 分隔符
- **平台差异点**（详见 §14.5）：本地 ASR 二进制（sherpa-onnx）、LSP 可执行文件查找（`which`/`where`）、Tauri 打包分发格式（NSIS/dmg/deb-rpm）

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
    │   ├── agent.rs                   # agent:list/create/update/delete/get
    │   ├── session.rs                 # session:list/create/delete
    │   ├── chat.rs                    # chat:send/abort/history
    │   ├── model.rs                   # model:list/get-config/set-default
    │   ├── mcp.rs                     # mcp:list/add/remove/call-tool/test
    │   ├── skill.rs                   # skill:list/install/uninstall/toggle/search-market
    │   ├── wiki.rs                    # wiki:create/list/delete/read-page/write-page
    │   ├── rag.rs                     # rag:ingest/list-documents/delete-document/search
    │   ├── meeting.rs                 # meeting:create/start-record/stop-record/summary/qa/export
    │   ├── file.rs                    # file:pick/read/write/list/preview
    │   ├── translate.rs               # translate:translate/history
    │   ├── ocr.rs                    # ocr:recognize
    │   ├── workflow.rs                # workflow:run/list/get/stop
    │   ├── settings.rs                # settings:get/set/provider-key
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
    │   ├── migrations/                # 001_init.sql / 002_rag.sql / 003_meeting.sql ...
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
│   │   │   ├── tokens.css            # CSS 变量（.light / .dark）
│   │   │   ├── glass.css             # 毛玻璃工具类
│   │   │   └── utilities.css         # 布局/间距工具类
│   │   └── index.ts
│   ├── components/
│   │   ├── base/                     # 基础原子组件
│   │   │   ├── Button.svelte         # 样式变体: primary/secondary/ghost/danger
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
│   │   └── events.ts                 # listen 封装（chat:chunk 等）
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
| `sessions` | 会话 | N:1 agents, 1:N messages |
| `messages` | 消息（含工具调用） | N:1 sessions |
| `skills` | 技能元数据 | N:N agents |
| `mcp_servers` | MCP 服务器配置 | N:N agents |
| `wikis` | Wiki 知识库 | 1:N rag_documents |
| `rag_documents` | RAG 文档 | N:1 wikis, 1:N rag_chunks |
| `rag_chunks` | 分块（含向量） | N:1 rag_documents |
| `meetings` | 会议 | 1:N meeting_transcripts |
| `meeting_transcripts` | 转写片段 | N:1 meetings |
| `asr_configs` | ASR 后端配置（§10.3.1） | - |
| `workflows` | 工作流定义 | 1:N workflow_runs |
| `translate_history` | 翻译历史 | - |
| `glossary_terms` | 翻译术语表（§10.5.2） | - |
| `memory_fts` | 记忆全文索引（FTS5，§10.7.2） | - |
| `preferences` | 键值偏好设置 | - |

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
    id         TEXT PRIMARY KEY,
    agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    title      TEXT,
    pinned     INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
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
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE workflow_runs (
    id          TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    status      TEXT NOT NULL DEFAULT 'running', -- running|done|failed|cancelled
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
        Ok(Self { pool })
    }
}

// 编译期 SQL 校验（sqlx::query_as!）
#[derive(sqlx::FromRow)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub system_prompt: Option<String>,
    pub model_id: Option<String>,
    // ...
}

pub async fn list_agents(pool: &SqlitePool) -> Result<Vec<AgentRow>, AppError> {
    Ok(sqlx::query_as::<_, AgentRow>(
        "SELECT id, name, description, system_prompt, model_id, temperature, max_tokens, order_key, created_at, updated_at FROM agents ORDER BY order_key"
    )
    .fetch_all(pool).await?)
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
    active: Mutex<HashMap<String, CancellationToken>>,   // session_id → token
}
pub async fn abort(&self, session_id: &str) -> Result<(), AppError> {
    if let Some(t) = self.active.lock().await.remove(session_id) { t.cancel(); }
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
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    try {
        return await window.__TAURI__.core.invoke<T>(cmd, args);
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

**会话域** `commands/session.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `session:list` | `{agent_id?}` | `Vec<SessionDto>` |
| `session:create` | `{agent_id, title?}` | `SessionDto` |
| `session:delete` | `{id}` | `()` |
| `session:rename` | `{id, title}` | `SessionDto` |
| `session:history` | `{id, before?, limit?}` | `Vec<MessageDto>` |

**对话域** `commands/chat.rs`

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `chat:send` | `{session_id, content, attachments?}` | `MessageDto` | 立即返回 user 消息，流式走事件 |
| `chat:abort` | `{session_id}` | `()` | 中断当前流 |
| `chat:regenerate` | `{session_id, message_id}` | `()` | 重新生成最后一条助手消息 |

**模型域** `commands/model.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `model:list` | `{}` | `Vec<ModelDto>` |
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
| `meeting:start-recording` | `{id, asr_config}` | `()` | 转写走事件 |
| `meeting:stop-recording` | `{id}` | `{transcript}` |
| `meeting:summary` | `{id}` | `{summary}` |
| `meeting:qa` | `{id, question}` | `{answer}` |
| `meeting:export` | `{id, format}` | `{path}` | md/docx |

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
| `translate:translate` | `{text, source?, target, model_id?}` | `{translated, source}` |
| `translate:history` | `{limit?}` | `Vec<TranslateEntry>` |
| `ocr:recognize` | `{image_path, lang?}` | `{text, blocks?}` |

**工作流域** `commands/workflow.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `workflow:list` | `{}` | `Vec<WorkflowDto>` |
| `workflow:run` | `{workflow_id, inputs}` | `{run_id}` | 进度走事件 |
| `workflow:stop` | `{run_id}` | `()` |
| `workflow:result` | `{run_id}` | `WorkflowResultDto` |

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

**LSP 域** `commands/lsp.rs`

| 命令 | 参数 | 返回 |
|------|------|------|
| `lsp:list` | `{workdir}` | `Vec<LspServerInfo>` | 启用的语言服务器状态 |
| `lsp:diagnostics` | `{path}` | `Vec<Diagnostic>` | 当前文件诊断（错误/警告） |
| `lsp:start` | `{server_id, workdir}` | `()` | 启动语言服务器 |
| `lsp:stop` | `{server_id}` | `()` | 停止语言服务器 |

**Agent 上下文域** `commands/context.rs`（侧边栏聚合）

| 命令 | 参数 | 返回 |
|------|------|------|
| `context:agent` | `{agent_id, session_id?}` | `AgentContext` | 侧边栏聚合：用量/工作目录/指令/MCP/LSP/目录（见 9.10） |

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
| `model:list-changed` | `{}` | 模型配置变更 |
| `agent:changed` | `{event, agent}` | agent CRUD 事件 |
| `usage:updated` | `{stats}` | 用量更新（消息完成后推送，刷新面板） |
| `lsp:status-changed` | `{server_id, status}` | LSP 服务器状态变化 |
| `lsp:diagnostics` | `{path, diagnostics}` | 诊断更新 |
| `workspace:changed` | `{path}` | 工作目录切换 |

---

## 9. Svelte 5 前端详细设计

### 9.1 Apple 设计令牌（tokens/colors.ts）

```ts
export const semanticColors = {
    light: {
        bg: "#F5F5F7", fg: "#1D1D1F", fgSecondary: "#6E6E73",
        accent: "#0071E3", green: "#34C759", red: "#FF3B30",
        orange: "#FF9500", purple: "#AF52DE", teal: "#30B0C7",
        separator: "rgba(60,60,67,0.29)", glass: "rgba(255,255,255,0.72)",
        glassBorder: "rgba(255,255,255,0.5)",
    },
    dark: {
        bg: "#000000", fg: "#F5F5F7", fgSecondary: "#98989D",
        accent: "#0A84FF", green: "#30D158", red: "#FF453A",
        orange: "#FF9F0A", purple: "#BF5AF2", teal: "#64D2FF",
        separator: "rgba(84,84,88,0.6)", glass: "rgba(28,28,30,0.72)",
        glassBorder: "rgba(255,255,255,0.08)",
    },
} as const;
```

### 9.2 排版令牌（tokens/typography.ts）

```css
:root {
    --font-sans: -apple-system, "SF Pro Text", "PingFang SC", "Segoe UI", "Microsoft YaHei", sans-serif;
    --font-mono: "SF Mono", "JetBrains Mono", "Cascadia Code", Consolas, monospace;
    --text-xs: 11px; --text-sm: 13px; --text-base: 15px; --text-lg: 17px;
    --text-xl: 20px; --text-2xl: 24px; --text-3xl: 28px; --text-4xl: 34px;
}
```

### 9.3 毛玻璃实现（glass.css）

```css
.glass {
    backdrop-filter: saturate(180%) blur(20px);
    background: var(--color-glass);
    border: 1px solid var(--color-glass-border);
}
```

### 9.4 动画令牌（tokens/motion.ts）

```ts
export const motion = {
    spring: "cubic-bezier(0.34, 1.56, 0.64, 1)",   // iOS 弹性
    easeInOut: "cubic-bezier(0.42, 0, 0.58, 1)",
    fast: 150, base: 250, slow: 400, sheet: 500,
} as const;
```

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

```svelte
<script lang="ts">
    type Variant = "primary" | "secondary" | "ghost" | "danger";
    let { variant = "primary", disabled = false, onclick, children }: {
        variant?: Variant; disabled?: boolean; onclick?: () => void; children: Snippet;
    } = $props();
</script>

<button class={`btn btn-${variant}`} {disabled} {onclick}>
    {@render children()}
</button>

<style>
    .btn {
        border-radius: 980px; border: none; cursor: pointer;
        font-weight: 600; font-size: 15px; padding: 7px 16px;
        transition: transform 0.15s var(--spring);
    }
    .btn:active { transform: scale(0.96); }
    .btn-primary { background: var(--color-accent); color: #fff; }
    .btn-secondary { background: var(--color-bg-secondary); color: var(--color-fg); }
    .btn-ghost { background: transparent; color: var(--color-accent); }
    .btn-danger { background: var(--color-red); color: #fff; }
    .btn:disabled { opacity: 0.4; cursor: not-allowed; }
</style>
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

### 9.9 主页面板（Home Dashboard）

**定位**：应用首页（`/`），是 Agent 的"总控制台"。参考 **lobehub/lobehub**（Agent 卡片网格 + 状态展示）、**hermes-control-interface**（系统状态面板）、**crewAI**（任务/Agent 看板）的设计语言。

**布局**（滚动式面板，网格布局）：

```
┌──────────────────────────────────────────────────────────────┐
│  Home Dashboard (滚动)                                          │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Header: 欢迎语 + 搜索框(⌘K) + 当前工作目录 + Provider状态 │  │
│  └────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────┐           │
│  │ 用量统计卡（4 列 StatCard）                     │           │
│  │ [今日 Tokens] [本周 Tokens] [本月费用 ¥] [调用次数]│           │
│  └───────────────────────────────────────────────┘           │
│  ┌────────────────────────────────────────┐ ┌─────────────┐  │
│  │ Agent Launcher（多 Agent 调用入口）      │ │ 用量趋势图   │  │
│  │ ┌──────┐ ┌──────┐ ┌──────┐             │ │ (7 日 SVG   │  │
│  │ │Agent1│ │Agent2│ │Agent3│ ...         │ │  折线/柱状)  │  │
│  │ └──────┘ └──────┘ └──────┘             │ │             │  │
│  │ [+ 新建 Agent]  [+ 市场]               │ │             │  │
│  └────────────────────────────────────────┘ └─────────────┘  │
│  ┌────────────────────────────────────────┐ ┌─────────────┐  │
│  │ Skill Overview（技能总览）               │ │ MCP Overview│  │
│  │ [已启用 x/y] 快捷开关列表 + 去市场        │ │ [服务器状态] │  │
│  │                                        │ │ [工具数量]   │  │
│  └────────────────────────────────────────┘ └─────────────┘  │
│  ┌───────────────────────────────────────────────────────┐   │
│  │ 多 Agent 任务设计区（TaskDesigner，核心新增）            │   │
│  │  ┌─ 模板栏 ─┐ ┌─ 画布（拖拽编排阶段）─────────────────┐ │   │
│  │  │ 深度研究 │ │ [Agent A]→[Agent B]→[Agent C]       │ │   │
│  │  │ 代码审查 │ │    │▲阶段1  │▲阶段2  │▲阶段3         │ │   │
│  │  │ 头脑风暴 │ │  依赖连线 依赖连线                    │ │   │
│  │  │ 翻译校对 │ │  ── 节点属性检查器（右）──────────────  │ │   │
│  │  └─────────┘ └──────────────────────────────────────┘ │   │
│  │  ┌─ 运行面板 ────────────────────────────────────────┐ │   │
│  │  │ [▶启动] [⏸暂停] [■停止] 进度条 │ 运行时间线(阶段流) │ │   │
│  │  └───────────────────────────────────────────────────┘ │   │
│  └───────────────────────────────────────────────────────┘   │
│  ┌───────────────────────────────────────────────┐           │
│  │ 最近会话（RecentSessions）+ 任务历史             │           │
│  └───────────────────────────────────────────────┘           │
└──────────────────────────────────────────────────────────────┘
```

**数据聚合**（单命令拉取，`dashboard:overview`）：

```rust
#[derive(Serialize)]
pub struct DashboardOverview {
    pub agents: Vec<AgentSummary>,        // 全部 Agent（含启用技能数/MCP 数/最近使用时间）
    pub usage: UsageStats,                // 用量统计（今日/本周/本月）
    pub usage_trend: Vec<UsagePoint>,     // 近 7 日趋势
    pub skills: SkillOverview,            // 已启用/总数/热门技能
    pub mcp_servers: Vec<McpServerStatus>,// 服务器状态 + 工具数
    pub recent_sessions: Vec<SessionSummary>, // 最近 10 个会话
    pub models: Vec<ModelStatus>,         // Provider/模型健康状态
    pub workflows: Vec<WorkflowSummary>,  // 预置工作流（任务模板）
    pub task_runs: Vec<TaskRunSummary>,   // 最近任务运行（含进行中任务的实时状态）
}
```

**Agent Launcher 卡片**（核心交互：点击即用）：

```
┌─────────────────────────┐
│  [Avatar] 名称            │
│  描述（1 行省略）          │
│  🟢 状态徽标 · 模型名      │
│  [技能 x] [MCP y]         │
│  ──────────────────────   │
│  [▶ 开始对话]  [⋮ 菜单]    │
└─────────────────────────┘
```

- 点击卡片 → `session:create` + 跳转 `/chat/{sessionId}`（直接进入对话）
- 卡片右键/⋮ 菜单：编辑 Agent、复制、删除、绑定工作目录
- 卡片拖拽到任务画布 → 自动创建该 Agent 的阶段节点（见 9.9.2）
- 排序：最近使用优先；支持拖拽重排（order_key）
- 空态：无 Agent 时显示引导 + 创建按钮 + 预设模板（研究员/写作/翻译等）

**用量统计实现**：从 `messages.usage` 聚合（见 §5），按 `created_at` 分组；费用估算用 `preferences` 中的单价表（provider/model → 每 1K token 价格）。`usage:updated` 事件在每条消息完成后推送，面板增量刷新。

### 9.9.1 多 Agent 任务设计区（核心新增）

**定位**：主页面板的中央工作区，让用户**可视化设计 → 派发 → 监控**多 Agent 协作任务，替代 / 补充预置工作流。参考 **AutoGen Studio / Dify / Flowise** 的节点编排画布，但保持轻量（桌面应用，不做完整低代码平台）。

**三种使用模式（视图切换，TaskBoard 顶部 Tab）**：

| 模式 | 用途 | 交互 |
|------|------|------|
| **模板** | 快速启动 | 模板卡片网格（预置工作流 + 用户保存的模板），点击即填参数运行 |
| **设计** | 自定义编排 | 画布拖拽阶段节点 + 连线依赖，实时校验，可保存为模板 |
| **运行** | 监控与回溯 | 运行时间线（阶段流状态着色）+ 阶段结果展开 + 历史列表 |

**① 模板模式（TaskTemplateCard 网格）**：

- 每个模板卡：名称 + 描述 + 阶段数 + 预估成本/耗时标签
- 预置模板与 §10.6 预置工作流一致：深度研究 / 代码审查 / 头脑风暴 / 翻译校对
- 点击模板 → 打开"参数填充对话框"（TaskSaveDialog 复用）：
  - 输入字段来自模板声明的 `inputs`（如深度研究 → `topic`、`depth`）
  - 可选择覆盖各阶段使用的 Agent（默认用阶段角色对应的默认 Agent）
  - [运行] → 直接进入运行模式

**② 设计模式（TaskDesigner 画布）**：

```
┌────────────────────────────────────────────────────┐
│ 左侧：Agent 池（可拖入）      │ 画布（横向流）        │
│ ┌─────────────────┐          │                     │
│ │ ▤ 研究员 agent   │──拖入──▶│  ┌───────────┐      │
│ │ ▤ 分析师 agent   │          │  │ stage1    │─┐    │
│ │ ▤ 写手 agent     │          │  │ 研究员     │ │    │
│ │ ▤ ...           │          │  └───────────┘ │    │
│ └─────────────────┘          │                ▼    │
│ 底部：模板/新建空画布         │  ┌───────────┐      │
│                              │  │ stage2    │─┐    │
│                              │  │ 分析师     │ │    │
│                              │  └───────────┘ │    │
│ 节点属性检查器（右侧浮层）     │                ▼    │
│ ┌─────────────────────────┐  │  ┌───────────┐      │
│ │ stage1                   │  │  │ stage3    │      │
│ │ 角色: 研究员              │  │  │ 写手      │      │
│ │ Agent: [下拉]             │  │  └───────────┘      │
│ │ 提示模板: {{topic}}...    │  │                     │
│ │ 工具白名单: [web_search]  │  │                     │
│ │ 依赖: [无/前一阶段]        │  └─────────────────────┘
│ │ [删除节点] [复制节点]      │
│ └─────────────────────────┘
└────────────────────────────────────────────────────┘
```

**画布交互规则**：

| 交互 | 行为 |
|------|------|
| 从 Agent 池拖入 | 创建阶段节点，默认角色 = Agent 名，提示模板 = 空 |
| 从 Agent 卡片拖入 | 同上（复用当前 Agent 配置：模型/工具/技能） |
| 节点间拖拽连线 | 建立 `depends_on` 依赖；禁止环（实时拓扑校验，红色告警） |
| 双击节点 | 打开节点属性检查器（TaskNodeInspector） |
| 节点底部 handle | 点击拖出连线到下一节点 |
| 右键节点 | 删除 / 复制 / 禁用（禁用后不参与运行） |
| 画布空白区 | 点击取消选中；拖拽平移；滚轮缩放（0.8~1.5x） |
| 顶部工具栏 | 撤销/重做、自动布局、清空、保存为模板 |

**节点属性检查器（TaskNodeInspector）**：

```
┌─ 阶段节点属性 ─────────────────┐
│ 名称        [研究阶段]           │
│ 角色        [研究员]             │
│ 使用的 Agent [▾ 研究员-Agent]    │  ← 决定模型/工具/技能
│ 提示模板     [textarea]          │
│           变量提示: {{topic}}    │
│           {{stage1.output}}     │  ← 引用依赖阶段输出
│ 工具白名单   [web_search] [read] │  ← 从该 Agent 可用工具多选
│ 最大轮次     [10]                │
│ 模型覆盖     [▾ 使用 Agent 默认]  │
│ ──────────────────────────────  │
│ [保存] [删除] [复制]              │
└────────────────────────────────┘
```

**依赖输出引用**：提示模板支持 `{{stage.id.output}}` 占位符，运行前由 `render_template` 解析（见 §10.6）。检查器提供"插入变量"按钮 + 语法高亮 + 悬停预览实际值。

**③ 运行模式（TaskRunnerPanel + TaskRunTimeline）**：

- [▶ 启动] → `workflow:run` 或新命令 `task:run`（自定义设计任务）→ 返回 `run_id`
- 运行面板：
  - 顶部：进度条（完成阶段/总阶段）+ 当前阶段名 + 耗时
  - 时间线（垂直）：每个阶段一节点行——状态着色（⬜待运行/🟦运行中/🟩完成/🟥失败/⬛已取消）+ 输出摘要（折叠）
  - 阶段详情展开（TaskStageResult）：最终文本 + 该阶段全部工具调用卡片（名称/参数/耗时/结果）
  - 控制：⏸ 暂停（当前阶段完成后停）、■ 停止（取消 token）、▶ 继续
- [保存结果] → 阶段输出写入会话（生成对话页消息），用户可继续对话式追问
- 运行中任务在 `task_runs` 中轮询 + `workflow:stage` 事件实时推进（无需刷新）

**任务定义数据结构**（可保存为模板，落库 `workflows` 表，`definition` JSON）：

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct TaskDefinition {
    pub id: String,               // 新建时生成
    pub name: String,
    pub description: String,
    pub inputs: Vec<TaskInput>,   // 运行前用户填写的参数声明
    pub stages: Vec<TaskStageDef>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskStageDef {
    pub id: String,               // 如 "stage1"（供 {{stage1.output}} 引用）
    pub name: String,
    pub role: String,             // 角色名（展示用）
    pub agent_id: Option<String>, // 指定 Agent（空 = 按角色找默认）
    pub prompt_template: String,  // 支持 {{input}} / {{stage.x.output}}
    pub tools: Vec<String>,       // 工具白名单
    pub max_iterations: u32,
    pub depends_on: Vec<String>,  // 依赖阶段 id
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskInput {
    pub key: String,              // 如 "topic"
    pub label: String,            // 如 "研究主题"
    pub kind: InputKind,          // Text | Textarea | Select | Number
    pub default: Option<Value>,
    pub required: bool,
}
```

**新增 IPC 命令**：

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `task:save-template` | `{definition}` | `WorkflowDto` | 保存自定义任务为模板（写入 workflows 表） |
| `task:run` | `{definition, inputs}` | `{run_id}` | 运行自定义任务（不走 DB，直接执行） |
| `task:validate` | `{definition}` | `{ok, errors}` | 画布保存前校验（环检测/变量引用/工具存在性） |
| `task:rerun` | `{run_id, inputs?}` | `{run_id}` | 用相同定义重跑（历史列表复用） |

**后端执行**：`task:run` 将 `TaskDefinition` 转换为 `Workflow`（`WorkflowStage` 映射），交给 `WorkflowEngine.run()`（§10.6），事件流与预置工作流完全一致。**模板与自定义任务共用同一执行引擎，零额外路径。**

### 9.10 Agent 侧边栏（Agent Context Sidebar）

**定位**：对话页右侧可折叠面板（`RightPanel` 的增强版），展示当前 Agent + 会话的**运行时上下文**。参考 **OpenHands**（工作目录 + 文件树 + 会话上下文）、**hermes-control-interface**（终端/文件/指标面板）、**lobehub**（会话上下文侧栏）的设计。

**布局结构**（Tab 式，默认展开）：

```
┌────────────────────────────────┐
│ Agent Sidebar (320px，可调宽)    │
│ ┌────────────────────────────┐ │
│ │ Agent 头部                  │ │
│ │ [Avatar] 名称 · 状态徽标    │ │
│ │ 模型名 · 工作目录（截断）    │ │
│ │ (点击头部 → Agent 编辑页)   │ │
│ └────────────────────────────┘ │
│ ┌────────────────────────────┐ │
│ │ [用量][目录][指令][MCP]      │ │ ← Tab 栏
│ │ [LSP][文件]                │ │    （6 Tab，横向滚动）
│ └────────────────────────────┘ │
│ ┌────────────────────────────┐ │
│ │                            │ │
│ │     当前 Tab 内容区         │ │ ← 独立滚动区域
│ │   （下方各 Tab 详设）        │ │
│ │                            │ │
│ └────────────────────────────┘ │
│ ┌────────────────────────────┐ │
│ │ 底部: 会话计数 · token 汇总 │ │
│ │ [⚙ Agent 配置] [↗ 全屏]    │ │
│ └────────────────────────────┘ │
└────────────────────────────────┘
```

**全局交互规则**：

| 规则 | 说明 |
|------|------|
| 展开/折叠 | `⌘\` 切换；折叠为 44px 竖条（仅图标），悬停浮出预览 |
| 宽度 | 默认 320px，拖拽 Splitter 调宽（280~480px），记忆在 `preferences` |
| Tab 记忆 | 上次激活的 Tab 持久化，切会话/切 Agent 不重置 |
| 数据加载 | 首次展开时 `context:agent` 一次拉全量；之后增量事件更新 |
| 空态 | 无会话/无 Agent 时显示引导文案 + 指向主页面的按钮 |
| 多会话 | 切换会话时保留 Tab 位置，仅刷新数据（`context:agent` 带 session_id） |

### 9.10.1 Tab 1 — 用量 (SidebarUsage)

**目标**：让用户随时看到"这个会话花了多少、上下文还剩多少"，避免超出模型上下文窗口。

**布局**：

```
┌─ 用量 ────────────────────────┐
│ 上下文窗口                     │
│ ████████████░░░░░ 38%         │ ← 进度条（超 80% 变橙，95% 变红）
│ 3,840 / 10,240 tokens         │
│ ─────────────────────────────  │
│ 本次会话                       │
│  输入 tokens     2,150        │
│  输出 tokens     1,690        │
│  工具调用次数     12           │
│  会话费用         ¥0.042       │
│ ─────────────────────────────  │
│ 今日累计（此 Agent）           │
│  调用次数        34           │
│ 总 tokens       82,500       │
│ 费用             ¥0.86        │
│ ─────────────────────────────  │
│ [查看用量趋势 →] [重置会话计数] │
└───────────────────────────────┘
```

**数据来源**：

| 项 | 计算方式 | 更新时机 |
|----|----------|----------|
| 上下文占用 | 会话内 `messages.usage` 求和 ÷ 模型 `max_tokens` | 每消息完成 |
| 会话费用 | 单价表（preferences） × 会话 tokens | 每消息完成 |
| 工具调用次数 | 会话内 `tool_calls` 计数 | 每工具返回 |
| 今日累计 | `usage:stats` 按 agent_id 分组 | `usage:updated` 事件 |

**交互规则**：

- 进度条阈值着色：`<80%` 默认 / `80~95%` 橙 / `>95%` 红 + "建议开启新会话"提示条
- 点击"查看用量趋势" → 展开内嵌 7 日迷你图（复用 UsageChart 组件，只读）
- "重置会话计数" → 二次确认（ConfirmDialog），不删除消息仅清计数展示（纯前端状态）
- 长会话自动截断提示：后端 PromptBuilder 已做滑动窗口（§10.7），此处仅展示窗口内有效 tokens

### 9.10.2 Tab 2 — 工作目录 (SidebarWorkdir)

**目标**：展示/切换当前 Agent 的工作目录，是"指令/MCP/LSP/文件"四个 Tab 的上下文根。

**布局**：

```
┌─ 工作目录 ────────────────────┐
│ 当前目录                       │
│ ┌──────────────────────────┐  │
│ │ C:\Users\me\projects\... │  │ ← 路径（省略号截断，hover 显示全量）
│ └──────────────────────────┘  │
│ [✏ 编辑] [📋 复制] [📂 打开]  │
│ ─────────────────────────────  │
│ 最近目录                       │
│  🕐 C:\Users\me\projects\app  │ ← 点击切换
│  🕐 D:\repos\prism-agent      │
│  🕐 C:\Users\me\notes         │
│  [+ 添加到最近目录]            │
│ ─────────────────────────────  │
│ 绑定状态                       │
│  当前 Agent 固定目录: 未设置    │
│  [绑定当前目录] [解绑]         │
└───────────────────────────────┘
```

**数据来源**：`workspace:get` / `workspace:set`；最近目录与 Agent 绑定持久化在 `preferences`（JSON 数组）。

**交互规则**：

| 操作 | 行为 |
|------|------|
| ✏ 编辑 | 内联输入框 + 路径建议（历史目录自动补全）；回车提交 → `workspace:set` |
| 📂 打开 | `workspace:open-file`（shell 打开资源管理器） |
| 点击最近目录 | `workspace:set` → 切换 → 刷新全部依赖 Tab |
| 绑定 | Agent 绑定固定目录后，启动该 Agent 会话自动使用（不随全局切换） |
| 切换成功 | 触发 `workspace:changed` 事件 → 指令/MCP/LSP/文件四 Tab 自动刷新 |

**边界**：目录不存在 → 红色警告 + 建议重新选择；无权限 → 只读提示。

### 9.10.3 Tab 3 — 指令文件 (SidebarInstructions)

**目标**：展示当前工作目录对 Agent 生效的指令/规则文件，支持快速查看与编辑，并明确标注"是否已注入会话"。

**布局**：

```
┌─ 指令文件 ────────────────────┐
│ 生效文件（按优先级排序）        │
│ ┌──────────────────────────┐  │
│ │ 📄 CLAUDE.md   ✅ 已注入   │  │ ← 徽标: 注入状态
│ │    工作目录根  · 42 行     │  │
│ └──────────────────────────┘  │
│ ┌──────────────────────────┐  │
│ │ 📄 AGENTS.md   ✅ 已注入   │  │
│ │    工作目录根  · 18 行     │  │
│ └──────────────────────────┘  │
│ ┌──────────────────────────┐  │
│ │ 📄 .cursor/rules/backend  │  │
│ │    ⚠️ 存在未注入           │  │
│ └──────────────────────────┘  │
│ ┌──────────────────────────┐  │
│ │ 📄 .prism/memory.md       │  │
│ │    ✅ 已注入（项目记忆）    │  │
│ └──────────────────────────┘  │
│ [＋ 新建指令文件] [↻ 重新扫描]  │
│ ─────────────────────────────  │
│ 文件预览（点击文件后展开）      │
│ ┌──────────────────────────┐  │
│ │ # CLAUDE.md              │  │
│ │ 只读渲染/编辑切换          │  │
│ │ [✏ 编辑] [💾 保存] [✖]    │  │
│ └──────────────────────────┘  │
└───────────────────────────────┘
```

**探测与优先级规则**：

| 优先级 | 文件 | 注入方式 |
|--------|------|----------|
| 1 | `{workdir}/CLAUDE.md` | 全量注入系统提示 |
| 2 | `{workdir}/AGENTS.md` | 全量注入（存在时与 CLAUDE.md 并存） |
| 3 | `{workdir}/.cursor/rules/*.mdc` | 摘要注入（每文件前 100 行 + 文件名） |
| 4 | `{workdir}/.prism/memory.md` | 全量注入（项目记忆，§10.7） |
| 5 | `{workdir}/README.md` | 不注入，仅展示（避免噪声；可手动"注入此文件"） |

**交互规则**：

- 文件卡片点击 → 内嵌预览（`workspace:read-file`，默认只读渲染 Markdown/纯文本）
- ✏ 编辑 → 切换 Textarea（语法高亮），💾 保存 → `workspace:write-instructions` → 重新注入标记 ✅
- "未注入"文件提供 [注入此文件] 按钮 → 加入本次会话的 PromptBuilder（`session:inject-file` 命令，见 8.2 补充）
- 文件变更（外部编辑器修改）→ 前端轮询或 `fs:watcher` 事件 → 显示"文件已变更，点击刷新"角标

### 9.10.4 Tab 4 — MCP (SidebarMcp)

**目标**：展示当前 Agent 绑定的 MCP 服务器实时状态与可用工具，可快速禁用/启用工具。

**布局**：

```
┌─ MCP ─────────────────────────┐
│ 服务器（agent 绑定）           │
│ ┌──────────────────────────┐  │
│ │ 🟢 filesystem    已连接   │  │ ← 状态点 + 文本
│ │    ▸ 展开工具 (6)         │  │
│ │   ├─ read_file           │  │
│ │   ├─ write_file   [✖]    │  │ ← 单工具禁用开关
│ │   └─ ...                 │  │
│ └──────────────────────────┘  │
│ ┌──────────────────────────┐  │
│ │ 🟡 playwright  连接中…    │  │ ← 转圈动画
│ └──────────────────────────┘  │
│ ┌──────────────────────────┐  │
│ │ 🔴 github 连接失败        │  │
│ │    错误: timeout 30s      │  │ ← hover 显示日志尾部
│ │    [重试] [查看日志]       │  │
│ └──────────────────────────┘  │
│ [＋ 添加服务器] [管理全部 →]   │
└───────────────────────────────┘
```

**数据来源**：`mcp:status-all`（server_id/name/status/last_error/tools_count/tools）；绑定关系来自 `agent:get` 的 `mcps` 字段。

**交互规则**：

| 操作 | 行为 |
|------|------|
| 展开服务器 | 懒加载 `mcp:tools`（缓存，勿每次展开都请求） |
| 工具 ✖ 开关 | `agent:update {disabled_tools}` → 立即生效于下一轮生成 |
| 重试 | `mcp:test` + 重新连接 → 状态刷新 |
| 查看日志 | 弹 Sheet 显示 `ServerLogBuffer` 尾部 200 行 |
| 添加服务器 | 跳设置页 MCP 区（锚点定位） |

**边界**：服务器进程崩溃 → 状态自动置 🔴（McpRuntime 监听子进程退出）；工具目录变化 → `mcp:tools-changed` 事件刷新计数。

### 9.10.5 Tab 5 — LSP (SidebarLsp)

**目标**：展示工作目录启用的语言服务器状态 + 当前打开文件的实时诊断（错误/警告），让 Agent 侧也能看到"代码健康度"。

**布局**：

```
┌─ LSP ─────────────────────────┐
│ 语言服务器                     │
│ ┌──────────────────────────┐  │
│ │ 🟢 rust-analyzer  运行中  │  │
│ │    workspace: 3 crates    │  │
│ │    索引文件: 1,204        │  │
│ │    [■ 停止]               │  │
│ └──────────────────────────┘  │
│ ┌──────────────────────────┐  │
│ │ ⚪ typescript 未启动       │  │
│ │    [▶ 启动]               │  │
│ └──────────────────────────┘  │
│ ─────────────────────────────  │
│ 当前文件诊断: src/main.rs     │
│ 🔴 E0502 无法借用可变引用       │
│     :12:5                    │
│ 🟡 未使用的变量 unused_var    │
│     :8:3                     │
│ [查看全部诊断 →]              │
└───────────────────────────────┘
```

**LSP 检测与启动**（`lsp:list` / `lsp:start` / `lsp:stop`）：

```rust
/// 根据工作目录内容推断应启用的语言服务器
pub fn detect_lsp_servers(workdir: &Path) -> Vec<LspCandidate> {
    let mut out = Vec::new();
    if workdir.join("Cargo.toml").exists() {
        out.push(LspCandidate { id: "rust-analyzer", cmd: "rust-analyzer", langs: vec!["rust"] });
    }
    if workdir.join("package.json").exists() || workdir.join("tsconfig.json").exists() {
        out.push(LspCandidate { id: "typescript", cmd: "typescript-language-server", langs: vec!["ts", "js"] });
    }
    if workdir.join("pyproject.toml").exists() || workdir.join("requirements.txt").exists() {
        out.push(LspCandidate { id: "pyright", cmd: "pyright-langserver", langs: vec!["python"] });
    }
    if workdir.join("go.mod").exists() {
        out.push(LspCandidate { id: "gopls", cmd: "gopls", langs: vec!["go"] });
    }
    // ... (toml/json/yaml/html/css 等按需)
    out
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LspServerInfo {
    pub id: String,                    // "rust-analyzer"
    pub cmd: String,                   // 可执行文件
    pub status: LspStatus,             // Running | Stopped | Error(String)
    pub langs: Vec<String>,
    pub index_file_count: Option<u64>,
    pub last_error: Option<String>,
}
```

**协议实现**：LSP 客户端通过 `lsp-types` + 轻量 JSON-RPC 通道连接子进程（stdio），订阅 `textDocument/publishDiagnostics`；与 MCP 传输层复用 JSON-RPC 基础设施。服务器是否安装检测：`which` 查找可执行文件（**Windows 用 `where`，见 §14.5**），未安装 → 状态"未安装" + 显示安装命令提示（安装命令按平台区分，如 `cargo install rust-analyzer` / `npm i -g typescript-language-server` 通用，`pyright` 用 `pip install pyright`）。

**交互规则**：

| 操作 | 行为 |
|------|------|
| 启动 | `lsp:start {server_id, workdir}` → 子进程拉起 → 状态 🟢 |
| 停止 | `lsp:stop` → 优雅关闭 → 状态 ⚪ |
| 诊断列表 | `lsp:diagnostics {path}`；当前激活文件变更时自动刷新 |
| 点击诊断 | 定位到文件（`workspace:open-file` + 行号参数，外部编辑器跳转） |
| 服务器缺失 | 显示"未找到可执行文件：xxx，安装命令：cargo install rust-analyzer" |

**事件**：`lsp:status-changed`（启动/停止/崩溃）、`lsp:diagnostics`（文件保存或变更后推送）。崩溃自动重启（最多 3 次，指数退避）。

### 9.10.6 Tab 6 — 目录 (SidebarFiles)

**目标**：文件树浏览 + 轻量预览，让 Agent 与用户共享同一份"代码地图"。

**布局**：

```
┌─ 文件 ─────────────────────────┐
│ 🔍 过滤: 文件名包含…            │
│ 📁 src  (12)                   │
│  ├─ 📁 components (5)          │
│  │   ├─ Button.svelte  TS 120  │ ← 语言徽标 + 行数
│  │   └─ ...                    │
│  ├─ 📄 main.rs         Rust 98  │
│  └─ ...                        │
│ 📁 tests (2)                   │
│ 📄 Cargo.toml         TOML 45  │
│ 📄 README.md          MD  30   │
│ ─────────────────────────────   │
│ 文件预览（点击文件后展开）       │
│ ┌──────────────────────────┐   │
│ │ Button.svelte · 120 行    │   │
│ │ 语法高亮只读视图           │   │
│ │ [外部打开] [复制路径]      │   │
│ └──────────────────────────┘   │
└────────────────────────────────┘
```

**数据来源**：`workspace:tree`（懒加载：根层一次拉取，展开目录时按需 `workspace:tree {path}`）；过滤在本地执行（已加载部分），全量搜索用 `file:list`/全局搜索页。

**忽略规则**：`.git`、`.svn`、`node_modules`、`target`、`dist`、`build`、`__pycache__`、`.venv`、`vendor`（后端统一过滤，前端无需感知）。

**交互规则**：

| 操作 | 行为 |
|------|------|
| 点击目录 | 展开/收起（懒加载子层 + loading 骨架） |
| 点击文件 | 右侧/下方预览（文本类语法高亮；图片显示缩略图；二进制显示大小 + 提示） |
| 双击文件 | `workspace:open-file` 外部打开 |
| 过滤框 | 本地过滤已加载节点；深度 >4 层自动折叠 |
| 右键 | 复制路径 / 外部打开 / 在新会话引用（attach 到 Composer） |
| 大文件 | `>200KB` 拒绝全量读取，只显示头部 100 行 + "文件过大"提示 |

### 9.10.7 聚合命令与事件

**聚合命令** `context:agent`（一次拉全量，Tab 本地渲染）：

```rust
#[derive(Serialize)]
pub struct AgentContext {
    pub agent: AgentDto,
    pub session_usage: SessionUsage,       // Tab1
    pub workspace: WorkspaceInfo,          // Tab2 + Tab3 路径
    pub instructions: Vec<InstructionFile>,// Tab3
    pub mcp: Vec<McpServerStatus>,         // Tab4
    pub lsp: Vec<LspServerInfo>,           // Tab5
    pub tree: DirTree,                     // Tab6（仅根层，懒加载）
}

#[derive(Serialize)]
pub struct SessionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub context_used: u64,                 // input + output
    pub context_limit: u64,                // 模型 max_tokens
    pub tool_calls: u64,
    pub cost_est: f64,                     // 估算费用
    pub today_calls: u64,
    pub today_tokens: u64,
    pub today_cost: f64,
}
```

**增量事件**（Tab 更新不整页刷新）：

| 事件 | 影响的 Tab | 触发 |
|------|-----------|------|
| `usage:updated` | 用量 | 消息完成 |
| `workspace:changed` | 目录/指令/MCP/LSP/文件 | 目录切换 |
| `mcp:tools-changed` / `mcp:status-changed` | MCP | 服务器状态/工具变化 |
| `lsp:status-changed` / `lsp:diagnostics` | LSP | 服务器/诊断变化 |
| `fs:watcher`（新增） | 指令/文件 | 工作目录文件变更（`notify` crate 监听） |

**新增命令**（8.2 补充）：

| 命令 | 参数 | 返回 |
|------|------|------|
| `session:inject-file` | `{session_id, path}` | `()` | 将指令文件注入本次会话 |
| `lsp:detect` | `{workdir}` | `Vec<LspCandidate>` | 推断候选 LSP（无进程启动） |
| `fs:watch` | `{workdir, enable}` | `()` | 开启/关闭工作目录变更监听 |

**布局关系**：主页面的 `AgentLauncher` 点击后进入 `/chat/{sessionId}`，对话页默认展开 Agent 侧边栏；`⌘\` 折叠。主页面板与侧边栏共用 `usage`/`mcp` 数据源，一次请求双端复用。侧边栏六 Tab 中"用量/指令"与 PromptBuilder（§10.7）共享注入状态，"MCP/LSP"与 RigAgent 工具执行共享运行状态。

---

## 10. 特色功能详细设计

### 10.1 LLM Wiki 知识库系统

**文件结构**（磁盘即数据）：

```
{app_data}/wiki/{wikiId}/
├── SCHEMA.md              # 知识库结构说明（LLM 写作指引）
├── raw/                   # 源文档（导入的原始文件）
├── wiki/                  # 处理后页面（Markdown）
│   ├── index.md
│   ├── log.md             # 变更日志
│   ├── entities/          # 实体页面
│   ├── concepts/          # 概念页面
│   ├── sources/           # 来源页面
│   ├── comparisons/       # 对比页面
│   └── synthesis/         # 综合页面
```

**服务接口**：

```rust
pub struct WikiService { db: Database, base_dir: PathBuf }

impl WikiService {
    pub fn create_wiki(&self, name: &str, desc: Option<&str>) -> Result<Wiki, AppError>;
    pub fn read_page(&self, wiki_id: &str, path: &str) -> Result<String, AppError>;
    pub fn write_page(&self, wiki_id: &str, path: &str, content: &str) -> Result<(), AppError>;
    pub fn search_pages(&self, wiki_id: &str, query: &str) -> Vec<WikiPageHit>;   // 全文搜索（无 RAG 时回退）
    pub async fn write_ai(&self, wiki_id: &str, info: &str, model: &ModelProvider) -> Result<String, AppError>;
}
```

**write_ai 流程**（LLM 主动更新 Wiki）：

1. 读取 SCHEMA.md + index.md + log.md 作为上下文
2. LLM 决定更新现有页面 / 新建页面（输出结构化操作）
3. 解析操作 → 执行文件写入 → 追加 log.md

#### 10.1.1 write_ai 详细设计（核心）

**触发入口**（3 种）：

| 入口 | 场景 | 调用 |
|------|------|------|
| 对话内工具 | Agent 在对话中调用 `wiki_write` 工具 | `WikiWriteTool` → `write_ai` |
| 文件导入后 | 用户导入文档 → 可选"自动入库" | `wiki:ingest-ai` |
| 手动触发 | Wiki 页面 UI"让 AI 更新"按钮 | `wiki:write-ai` |

**上下文组装**（`build_wiki_context`）：

```rust
pub struct WikiWriteContext {
    pub schema: String,        // SCHEMA.md（分类规则，权威）
    pub index: String,         // index.md（现有页面索引，最多前 200 行）
    pub log: String,           // log.md（变更历史，最多前 50 行）
    pub existing_pages: Vec<WikiPageMeta>,  // 页面清单（路径 + 标题 + 前 20 行摘要）
    pub info: String,          // 待写入的新信息（用户/工具提供）
}

/// 组装上下文：全部拼进 system + user 消息，限制总量 ≤ 8K tokens
async fn build_wiki_context(&self, wiki_id: &str, info: &str) -> Result<WikiWriteContext, AppError>;
```

**LLM 决策输出格式**（结构化 JSON，Zod 校验等价物 = serde）：

```rust
/// 一次 write_ai 调用可能产生多个操作
#[derive(Serialize, Deserialize)]
pub struct WikiWritePlan {
    pub operations: Vec<WikiOp>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WikiOp {
    /// 新建页面（path 相对 wiki/ 根）
    CreatePage { path: String, title: String, content: String },
    /// 更新现有页面（content 为全文替换，需 LLM 提供完整新内容）
    UpdatePage { path: String, content: String, summary: String },
    /// 删除页面（引用清理，谨慎）
    DeletePage { path: String, reason: String },
    /// 追加到 index.md（仅索引条目，不重建全文）
    UpdateIndex { entries: Vec<String> },
    /// 跳过（信息与现有内容重复，无变更）
    Noop { reason: String },
}
```

**执行流程**（含校验与回滚）：

```
write_ai(info)
  1. build_wiki_context → 组装 prompt
  2. LLM generate（temperature 0.2，强制 JSON schema 输出）
  3. serde 解析 WikiWritePlan
     ├─ 解析失败 → 重试 1 次（附错误信息）→ 仍失败 → 返回可读错误
     ├─ 空操作 / 全 Noop → 返回 "未产生变更，原因: ..."
     └─ 通过 → 进入执行
  4. 逐操作执行（事务式：先全部写入临时目录，成功后再原子移动）
     ├─ path 安全校验：canonicalize 前缀必须是 {wikiDir}/wiki/，防目录穿越
     ├─ CreatePage → 写入 {path}.md；UpdatePage → 覆盖（先备份 .bak）
     ├─ DeletePage → 移入 {wikiDir}/.trash/（软删除，可恢复）
     └─ UpdateIndex → 追加条目到 index.md
  5. 全部成功 → 原子提交 + 追加 log.md 变更记录
  6. 任一失败 → 回滚（删除新建、恢复 .bak、还原 index）→ 返回错误
```

**log.md 变更记录格式**：

```markdown
# Log

## [2026-08-04T10:30:00Z] ai-write | Wiki Updated

Source: 对话导入 · 触发: write_ai 工具
Ops:
- CreatePage: concepts/kubernetes.md (新页面)
- UpdatePage: entities/k8s-cluster.md (补充 Ingress 章节)
Result: 2 ops applied, 1 noop
```

**校验规则**（`validate_plan`）：

| 规则 | 失败处理 |
|------|----------|
| `path` 含 `..` / 绝对路径 / 非 `.md` 后缀 | 拒绝该 op，记录错误 |
| 目标分类目录不存在（如 `entities/` 未创建） | 自动创建目录 |
| CreatePage 目标已存在 | 转为 UpdatePage 语义（提示 LLM 下次合并）或报错 |
| 单次 op 内容 > 8K tokens | 拆分为多次 write_ai（返回"内容过大，已拆分"） |
| 操作数 > 10 | 截断并警告（防 LLM 失控批量写） |

**对话内工具接入**（`WikiWriteTool` 实现 ADK ToolExecutor）：

```rust
pub struct WikiWriteTool { wiki_service: WikiService }

#[async_trait]
impl ToolExecutor for WikiWriteTool {
    fn name(&self) -> &str { "wiki_write" }
    fn description(&self) -> &str { "将新知识写入指定知识库（自动分类到 entities/concepts 等页面），返回变更摘要" }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "wiki_id": { "type": "string", "description": "知识库 ID" },
                "info": { "type": "string", "description": "要写入的知识内容" }
            },
            "required": ["wiki_id", "info"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let wiki_id = args["wiki_id"].as_str().ok_or(AgentError::InvalidArgs)?;
        let info = args["info"].as_str().ok_or(AgentError::InvalidArgs)?;
        let summary = self.wiki_service.write_ai(wiki_id, info, &self.summary_model()).await?;
        Ok(ToolOutput::text(format!("Wiki 更新完成：\n{summary}")))
    }
}
```

**前端反馈**（Wiki 页面"AI 更新"区）：

```
┌─ AI 写入 ─────────────────────────┐
│ 输入新知识（或粘贴文档片段）：      │
│ ┌──────────────────────────────┐  │
│ │ "Kubernetes 1.30 引入 ..."    │  │
│ └──────────────────────────────┘  │
│ [▶ 让 AI 入库]                    │
│ 结果预览（操作计划，确认后执行）：   │
│  ✓ 新建 concepts/kubernetes.md   │
│  ✓ 更新 entities/k8s-cluster.md │
│  ⚠ 跳过: 重复内容                │
│ [确认执行] [取消]                 │
│ 执行后: log.md 已更新 · 3 ops    │
└──────────────────────────────────┘
```

**相关命令**：

| 命令 | 参数 | 返回 |
|------|------|------|
| `wiki:write-ai` | `{wiki_id, info, preview?}` | `{plan?}` (preview=true 仅返回计划不执行) |
| `wiki:ingest-ai` | `{wiki_id, file_path}` | `{summary}` | 导入文件 + 自动入库 |
| `wiki:apply-plan` | `{wiki_id, plan}` | `{result}` | 用户确认计划后执行（防呆） |
| `wiki:restore-trash` | `{wiki_id, path}` | `()` | 从 .trash 恢复已删页面 |

### 10.2 RAG 引擎详细设计

**分块算法**：

```rust
/// 策略：优先段落边界（\n\n）→ 句子边界（。！？）→ 固定窗口
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.chars().count() <= chunk_size { return vec![text.to_string()]; }
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut end = (start + chunk_size).min(chars.len());
        if end < chars.len() {
            let half = start + chunk_size / 2;
            if let Some(rel) = chars[half..end].iter().rposition(|c| matches!(c, '\n' | '。' | '！' | '？' | '.' | '!' | '?')) {
                end = half + rel + 1;
            }
        }
        chunks.push(chars[start..end].iter().collect());
        start = end.saturating_sub(overlap);
    }
    chunks.retain(|c| !c.trim().is_empty());
    chunks
}
```

**嵌入模式**：

- **API 模式**：OpenAI `text-embedding-3-small` / 本地 Ollama `nomic-embed-text`，batch 20
- **本地模式**：无网络回退 `fastembed`（ONNX 量化）离线嵌入
- 向量以 **f32 小端 BLOB** 存储（比 JSON 省 75% 空间，检索快 5 倍）

**混合检索**（向量 + BM25）：

```rust
pub async fn hybrid_search(&self, wiki_id: &str, query: &str, top_k: usize) -> Result<Vec<RagHit>, AppError> {
    let q_vec = self.embedding.embed(query).await?;
    let chunks = self.db.query_chunks_with_vectors(wiki_id).await?;
    let mut scored: Vec<(RagHit, f32)> = chunks.iter()
        .map(|c| { let s = 0.7 * cosine_sim(&q_vec, &c.embedding) + 0.3 * bm25(query, &c.content); (c.into(), s) })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    Ok(scored.into_iter().take(top_k).map(|(h, _)| h).collect())
}
```

**摄取流程**（后台任务）：

```
文件 → file:parse 提取文本 → chunker 分块 → rag_documents(pending)
     → batch 嵌入 → rag_chunks(BLOB) → 状态 ready
进度走 rag:progress 事件；失败标记 error
```

### 10.3 会议纪要系统详细设计

**参考实现**：prism-agent 原项目（`MeetingService.ts` / `AsrServiceFactory.ts` / `AudioStreamManager.ts` / `ExportService.ts` / `MeetingToAgentService.ts`）与 **huiji（言记）**（`asr_service.dart` 1202 行 / `sherpa_adapter.dart` / `model_download_service.dart` / `audio_recorder_service.dart`）。本设计吸收两者的架构，并**扩展为多 ASR 后端可插拔架构**——不再局限于 MiMo 与 FunASR。

**状态机**：`idle → recording → transcribing → ready`

```
idle ──create──▶ recording ──stop──▶ transcribing ──done──▶ ready
                 │  ▲                    │
                 │  └── pause/resume ─────┤   （可恢复录音）
                 └── cancel ──▶ cancelled
```

**录音流程总览**：

1. `meeting:create` → 建目录 `{app_data}/meetings/{id}/` + DB 记录
2. `meeting:start-recording {asr_config}` → 前端 Web Audio API 采集
3. Rust `AudioStreamManager` 双写：存 `recording.wav`（原始）+ 实时转发 ASR 后端
4. ASR 后端按 `AsrBackend` 配置选择（见 10.3.1），流式或分片上传
5. 识别结果 → `meeting:transcript` 事件 → 每 N 段增量落库（见 10.3.4）
6. 可选实时翻译：`is_final` 片段 → LLM 翻译 → `meeting:translation` 事件
7. `meeting:stop-recording` → 最终保存 → 状态 transcribing → 转写完成 → ready

#### 10.3.1 ASR 可插拔架构（核心新增）

**设计目标**：同一套会议流程，支持任意 ASR 后端；新增后端只需实现一个 Trait，无需改动上层。

```rust
// data/services/asr/mod.rs — ASR 抽象层
#[async_trait]
pub trait AsrBackend: Send + Sync {
    /// 后端类型标识（用于配置与 UI 展示）
    fn kind(&self) -> AsrKind;
    /// 健康检查（启动会议前调用，失败则 UI 提前提示）
    async fn health_check(&self) -> Result<(), AsrError>;
    /// 开始识别：接收 16kHz PCM 音频块流，结果通过回调推送
    async fn start(&mut self, audio: AudioSource, events: AsrEventSink) -> Result<(), AsrError>;
    /// 停止识别，返回最终结果
    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError>;
    /// 支持的语言列表
    fn languages(&self) -> &[String];
}

/// 音频源：异步块流（与 prism-agent AudioStreamManager 同思路）
pub struct AudioSource { pub stream: Pin<Box<dyn AsyncStream<Item = PcmChunk> + Send>> }

/// 事件回调（增量转写 / 状态变化）
#[derive(Clone)]
pub struct AsrEventSink {
    pub on_segment: Arc<dyn Fn(AsrSegment) + Send + Sync>,
    pub on_status: Arc<dyn Fn(AsrStatus) + Send + Sync>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AsrSegment {
    pub index: u64,
    pub text: String,
    pub is_final: bool,          // false = 中间结果（会持续修正），true = 定稿
    pub start_ms: u64,
    pub end_ms: u64,
    pub language: Option<String>,
    pub confidence: Option<f32>,
    pub speaker_id: Option<u32>, // 说话人分离（支持的后端提供）
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum AsrKind {
    DashScopeFunasr,   // 云端 WebSocket 流式
    MiMoHttp,          // 云端 HTTP（OpenAI 兼容）
    SherpaOnnx,        // 本地 sherpa-onnx（SenseVoice / Paraformer / Whisper 中文）
    LocalFunasrWs,     // 本地 FunASR WebSocket 服务
    WhisperApi,        // OpenAI Whisper API（分片上传式）
    Vosk,              // 本地 Vosk（轻量，离线）
    AzureSpeech,       // Azure Speech-to-Text（流式，可选）
    Custom,            // 自定义 OpenAI 兼容端点
}

/// 后端工厂：按配置创建
pub fn create_asr_backend(cfg: &AsrConfig) -> Box<dyn AsrBackend> {
    match cfg.kind {
        AsrKind::DashScopeFunasr => Box::new(DashScopeFunasrBackend::new(cfg)),
        AsrKind::MiMoHttp        => Box::new(MiMoHttpBackend::new(cfg)),
        AsrKind::SherpaOnnx      => Box::new(SherpaOnnxBackend::new(cfg)),
        AsrKind::LocalFunasrWs   => Box::new(LocalFunasrWsBackend::new(cfg)),
        AsrKind::WhisperApi      => Box::new(WhisperApiBackend::new(cfg)),
        AsrKind::Vosk            => Box::new(VoskBackend::new(cfg)),
        AsrKind::AzureSpeech     => Box::new(AzureSpeechBackend::new(cfg)),
        AsrKind::Custom          => Box::new(CustomOpenAiBackend::new(cfg)),
    }
}
```

**支持的后端矩阵**：

| 后端 | 传输方式 | 优点 | 依赖 | 备注 |
|------|----------|------|------|------|
| **DashScope FunASR Realtime** | WebSocket 双工流式 | 中文流式最佳之一、免部署 | 无 | 同 prism-agent `FunASRRealtimeService` |
| **MiMo ASR** | HTTP（OpenAI 兼容 `/chat/completions`） | 免费额度、中文好 | 无 | 同 prism-agent `MiMoAsrService`（3s PCM→WAV→base64） |
| **本地 sherpa-onnx** | 进程内推理 | 完全离线、隐私、多模型 | 模型文件 + onnx | 同 huiji `sherpa_adapter.dart`；SenseVoice-Small / Paraformer-Large / Whisper |
| **本地 FunASR WS** | WebSocket 到本地服务 | 复用已有部署 | 外部服务 | 同 prism-agent `LocalFunASRService`（ws://localhost:10095） |
| **Whisper API** | HTTP 分片上传（15s 切片） | 多语言、OpenAI 生态 | 无 | 离线缓存 + 增量拼接 |
| **Vosk** | 进程内推理 | 轻量（~50MB 模型） | 模型文件 | 支持热词、多语言 |
| **Azure Speech** | WebSocket 流式 | 企业级、说话人分离 | 无 | 可选（需要 Azure Key） |
| **Custom** | HTTP（OpenAI 兼容） | 接任意兼容端点 | 无 | 用户填 base_url + api_key |

**本地模型管理**（借鉴 huiji `model_download_service.dart` + `ai_model.dart`）：

```rust
// data/services/asr/model_manager.rs
pub struct AsrModelManager { models_dir: PathBuf }

impl AsrModelManager {
    /// 可下载模型清单（内置 manifest，含大小/URL/校验和）
    pub fn catalog(&self) -> Vec<AsrModelInfo>;
    /// 下载（断点续传 + 进度事件 model:download-progress）
    pub async fn download(&self, model_id: &str, progress: ProgressSink) -> Result<PathBuf, AppError>;
    /// 已安装模型列表
    pub fn installed(&self) -> Vec<InstalledAsrModel>;
    /// 删除模型
    pub fn remove(&self, model_id: &str) -> Result<(), AppError>;
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AsrModelInfo {
    pub id: String,                    // "sherpa-sensevoice-small"
    pub name: String,                  // "SenseVoice-Small (中文流式)"
    pub backend: AsrKind,              // SherpaOnnx | Vosk
    pub size_mb: u64,
    pub lang: Vec<String>,
    pub url: String,                   // 官方镜像/HF 镜像
    pub sha256: String,                // 校验和（下载后验证）
    pub requires_vad: bool,            // 是否需要 Silero VAD
}
```

**内置模型清单**（首次启动提示下载，可后台下载）：

| model_id | 模型 | 大小 | 语言 |
|----------|------|------|------|
| `sherpa-sensevoice-small` | SenseVoice-Small（onnx int8） | ~228MB | 中/英/日/韩/粤 |
| `sherpa-paraformer-large` | Paraformer-Large | ~230MB | 中文 |
| `sherpa-whisper-tiny` | Whisper tiny（中文蒸馏） | ~80MB | 中/英 |
| `vosk-cn-small` | Vosk 中文小模型 | ~42MB | 中文 |
| `vosk-en-small` | Vosk 英文小模型 | ~40MB | 英文 |

**前端模型管理 UI**：`model_management`（借鉴 huiji `model_management_screen.dart`）——列表（已装/可下载）、下载进度条、删除确认、健康检查按钮。

#### 10.3.2 音频采集与传输（Rust 主进程处理）

**采集端**（渲染进程，借鉴 prism-agent `useAudioRecorder.ts`）：

```
getUserMedia({audio}) → AudioContext → AudioWorklet（替代 ScriptProcessorNode）
  → 采样率重采样至 16kHz → Float32 → Int16 PCM（小端）
  → 每 100ms 一包 → Tauri 事件 meeting:audio-chunk {meeting_id, pcm: base64}
```

- **AudioWorklet** 优于原项目的 ScriptProcessorNode（无主线程阻塞、低延迟）
- 静音检测（VAD）可在前端做轻量 RMS 阈值，也可交由后端（sherpa 内置 Silero VAD）
- 采样率配置：默认 16kHz 16bit 单声道（所有 ASR 后端通用格式）

**主进程 AudioStreamManager**（参考 prism-agent，规避其时序缺陷）：

```rust
// data/services/meeting/audio_stream.rs
pub struct AudioStreamManager {
    sources: Mutex<HashMap<String, mpsc::Sender<PcmChunk>>>,  // meeting_id → 块通道
    pending: Mutex<HashMap<String, VecDeque<PcmChunk>>>,      // 未创建 stream 前的缓冲（规避旧版丢块）
}

impl AudioStreamManager {
    pub fn push_chunk(&self, meeting_id: &str, pcm: Vec<u8>) -> Result<(), AppError>;
    pub fn create_stream(&self, meeting_id: &str) -> Receiver<PcmChunk>;   // ASR 消费端
    pub fn drop_stream(&self, meeting_id: &str);
}
```

**⚠️ 时序规避（旧版实测缺陷）**：prism-agent 中 renderer 的 `startRecording()` 立即发送 IPC chunks，但主进程的 stream 在 `Meeting_StartRecording` handler 里才创建 → 早期 chunks 被丢弃（`pushChunk` 中 `if (!buffer || !consumers) return`）。**本设计规避**：① 启动顺序改为"先建 stream 后启动录音"；② `pending` Map 缓冲先到的块，`create_stream()` 时 flush 给新消费者。

**双写策略**：音频块同时写入 `recording.wav`（WAV 头 + PCM 追加，流式写）与推给 ASR。录音文件可在停止后用于"重新转写/换 ASR 模型"（离线二次转写，见 10.3.5）。

#### 10.3.3 各后端协议细节（完整实现规范）

**① DashScope FunASR Realtime（WebSocket 双工流式）**

```
端点: wss://dashscope.aliyuncs.com/api/v1/services/audio/asr/recognition?model=paraformer-realtime-v2
鉴权: Header `Authorization: Bearer {api_key}` + `X-DashScope-DataInspection: enable`
协议: WebSocket 二进制 + JSON 文本帧
```

```json
// 客户端 → 服务端（打开后先发 start）
{ "header": { "action": "start", "task": "asr", "streaming": "duplex" },
  "parameter": {
    "model": "paraformer-realtime-v2",
    "format": "pcm", "sample_rate": 16000,
    "language_hints": ["zh"], "enable_partial_results": true
  },
  "payload": { "audio": { "data": "", "track": 1 } } }

// 客户端 → 服务端（持续二进制音频帧）
{ "header": { "action": "send-audio" }, "payload": { "audio": { "data": "<base64>", "track": 1 } } }

// 服务端 → 客户端（增量结果，sentence 未结束）
{ "header": { "action": "result", "status_code": 20000000 },
  "payload": { "result": { "transcripts": [{ "sentence_id": 0, "text": "今天天气", "begin_time": 0, "end_time": 800, "is_sentence_end": false }] } } }

// 服务端 → 客户端（句子定稿）
{ "header": { "action": "result" },
  "payload": { "result": { "transcripts": [{ "sentence_id": 0, "text": "今天天气很好。", "is_sentence_end": true }] } } }
```

**映射**：`is_sentence_end=true` → `AsrSegment.is_final`；`sentence_id` 递增 → `index`；错误码 `4xxxxxxx` 需展示可读错误。

```rust
pub struct DashScopeFunasrBackend {
    ws: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    api_key: String, model: String,
    sentence_id: u64,
}
```

**② MiMo ASR（HTTP，OpenAI 兼容）**

```
端点: {base_url}/chat/completions   （如 https://api.xiaomi.com/v1）
鉴权: Header `Authorization: Bearer {api_key}`
协议: HTTP POST，音频以 data URL 内联，非流式（无 WS）
```

```json
// 请求（每 3s 合并一次缓冲 PCM → WAV）
{ "model": "MiMo-V2.5-ASR",
  "messages": [{ "role": "user", "content": [
      { "type": "audio_url", "audio_url": { "url": "data:audio/wav;base64,<base64>" } }
  ]}],
  "max_tokens": 1024 }

// 响应
{ "choices": [{ "message": { "content": "今天天气很好。" } }] }
```

**增量策略**：每 3s 上传 → 返回全量文本 → 与上一段做**差集**（`new = full_text[len(prev_trimmed):]`）→ 若差集非空则 `on_segment(is_final=true)`。词边界不精确时可整体覆盖当前句（前端覆盖渲染）。

**③ 本地 sherpa-onnx（进程内推理，huiji 移植）**

```
依赖: sherpa-rs crate（onnxruntime 静态链接）
模型: SenseVoice-Small / Paraformer-Large / Whisper-tiny（onnx int8，~80-230MB）
```

```rust
pub struct SherpaOnnxBackend {
    recognizer: sherpa_rs::OnlineRecognizer,
    vad: Option<sherpa_rs::Vad>,          // Silero VAD
    hotwords: Vec<String>,
}

impl SherpaOnnxBackend {
    pub fn new(cfg: &AsrConfig) -> Result<Self, AsrError> {
        let config = sherpa_rs::OnlineRecognizerConfig {
            model: sherpa_rs::OnlineModelConfig {
                sense_voice: cfg.model.ends_with("sensevoice").then(|| ModelFile::from_path(&cfg.model_path)),
                paraformer: cfg.model.ends_with("paraformer").then(|| ModelFile::from_path(&cfg.model_path)),
                // ...
            },
            ..Default::default()
        };
        let mut rec = sherpa_rs::OnlineRecognizer::new(&config)?;
        for w in &cfg.hotwords { rec.add_hotword(w); }   // 热词: "Prism" 等专有名词
        Ok(Self { recognizer: rec, vad: None, hotwords: cfg.hotwords.clone() })
    }

    pub fn feed(&mut self, pcm: &[i16]) {
        // 若启用 VAD：先过 Silero 判断语音段，静音丢弃
        if let Some(vad) = &mut self.vad {
            let voiced = vad.process(pcm);
            if !voiced { return; }
        }
        self.recognizer.accept_waveform(16000, pcm);
        if let Some(result) = self.recognizer.get_result() {
            // result.is_final() → final 段（句子完整）；否则中间结果
            self.emit(AsrSegment { text: result.text, is_final: result.is_final(), .. });
        }
    }
}
```

**流式行为**：`accept_waveform` 逐块喂入 → `get_result` 轮询（每 200ms 或每块后）→ `is_final=true` 代表一句定稿（语音端点检测）。

**④ 本地 FunASR WebSocket（自建服务）**

```
端点: ws://localhost:10095（用户自建 FunASR 服务）
协议: WebSocket 二进制音频帧 + JSON 文本帧（类 DashScope，简化版）
```

```json
// 客户端 → 服务端
{ "mode": "2pass", "chunk_size": [5,10,5], "wav_name": "meeting",
  "is_speaking": true, "itn": true }
// 持续发二进制 PCM 帧（16kHz int16）

// 服务端 → 客户端（离线/在线混合）
{ "mode": "2pass-online", "text": "今天天气", "is_final": false }
{ "mode": "2pass-offline", "text": "今天天气很好。", "is_final": true, "timestamp": "[[0,800]]" }
```

**⑤ Whisper API（分片上传）**

```
端点: {base_url}/v1/audio/transcriptions
鉴权: Header `Authorization: Bearer {api_key}`
协议: HTTP multipart/form-data
```

```
请求字段:
  file          = <WAV 切片二进制>（15s 一片，重叠 1s）
  model         = whisper-1（或配置的本地兼容端点模型）
  language      = zh（可选，auto 默认）
  response_format = verbose_json（含 segments，便于拼接）
  temperature   = 0（确定性）

响应（verbose_json）:
{ "text": "今天天气很好。", "segments": [ { "start": 0.0, "end": 14.9, "text": "今天天气很好。" } ] }
```

**增量策略**：15s 切片 + 1s 重叠 → 拼接时丢弃重叠区尾部重复文本（`dedup_overlap`：新片前 1s 文本与上一片尾部 1s 文本去重）→ 每片完成即 `on_segment(is_final=true)`。延迟 ~15s（非实时，适合"录后转写"场景）。

**⑥ Vosk（本地轻量）**

```
依赖: vosk-rs crate + 模型文件（~40-50MB）
模型: vosk-model-small-cn-0.22 / vosk-model-small-en-us-0.15
协议: 进程内，接受 f32 PCM 样本
```

```rust
pub struct VoskBackend { recognizer: vosk::Recognizer }

impl VoskBackend {
    pub fn feed(&mut self, pcm_i16: &[i16]) {
        // 转 f32 → accept_waveform
        let samples: Vec<f32> = pcm_i16.iter().map(|&s| s as f32 / 32768.0).collect();
        if self.recognizer.accept_waveform(&samples) {
            // 一句定稿
            let result: vosk::FinalResult = self.recognizer.result().into();
            self.emit(AsrSegment { text: result.text, is_final: true, .. });
        } else {
            let partial: vosk::PartialResult = self.recognizer.partial_result().into();
            self.emit(AsrSegment { text: partial.partial, is_final: false, .. });
        }
    }
}
```

**⑦ Azure Speech（可选）**

```
端点: wss://{region}.stt.speech.microsoft.com/speech/recognition/conversation/cognitiveservices/v1
鉴权: Header `Ocp-Apim-Subscription-Key: {key}`
协议: WebSocket（Speech SDK 或原生 WS）
首帧: {"context": {"system": {"name": "PrismAgent", "version": "1.0"}}}
后续: 二进制音频帧（16kHz PCM 或 opus）
接收: SpeechHypothesis（中间）/ SpeechFragment（定稿，含 SpeakerId 说话人分离）
```

- 说话人分离：`SpeechFragment.speaker_id` → `AsrSegment.speaker_id`（会议场景核心价值）
- 语言：`language=zh-CN` 等；支持同时多语言 `zh-CN,en-US`

**⑧ Custom（OpenAI 兼容端点）**：与 ② MiMo 共享代码路径（`CustomOpenAiBackend` = `MiMoHttpBackend` 仅 base_url/api_key/model 不同），接口一致。

**通用错误处理**（所有后端）：

```rust
pub enum AsrError {
    Unauthorized,          // 401：Key 无效
    QuotaExceeded,         // 429：配额/限流
    Network(String),       // 连接失败/超时
    ModelNotFound(String), // 模型文件缺失
    Protocol(String),      // 协议解析失败（重试或降级）
}
```

- 连接失败自动重试（指数退避，3 次）
- 运行中断流 → 状态置 Error + 前端提示"是否切换后端继续"（保留已转写部分，断点续转）

#### 10.3.4 转写持久化与展示

**增量落库策略**（借鉴 prism-agent）：

- 内存保留全部 `transcript_segments`（含中间结果）；`is_final` 段按 index 覆盖写
- 每 **10 个 final 段** 或 **每 30s** 落库一次 `meeting_transcripts`（upsert by index）
- 转写上限 `MAX_TRANSCRIPT_LENGTH = 500KB`（超出截断最旧段，前端提示）
- 停止时最终保存 + 写 `transcript_translated.md`（翻译后的完整稿）

**实时渲染**：`meeting:transcript` 事件携带完整段列表增量（index 可覆盖），前端滚动定位——中间结果灰显 + 摆动光标，final 段正常显示。

#### 10.3.5 离线二次转写（新增）

- 录音停止后，用户可更换 ASR 模型**重新转写**（`meeting:retranscribe {id, asr_config}`）
- 读取 `recording.wav` → 走相同 `AsrBackend.start`（离线模型或上传式）
- 结果替换 `transcript` 并更新 `meeting_transcripts`；UI 提示"使用 XX 模型重新转写"
- 用途：云端转写不满意 → 换本地模型；或本地设备识别差 → 换云端

#### 10.3.6 摘要 / 清洗 / 问答 / 推送 Agent

**转写清洗**（借鉴 prism-agent `cleanTranscript`）：

```rust
pub async fn clean_transcript(&self, raw: &str, model: &ModelProvider) -> Result<String, AppError> {
    // LLM 指令: 修正错别字、补充标点、按语义分段、保留原意
    // 输出 Markdown 段落（## 小节）
}
```

**摘要生成**（`meeting:summary`）：

```
输入: title + participants + cleaned transcript
输出: 主题 / 主要讨论 / 关键决策 / 待办事项（含负责人）/ 行动项
```

- 超长转录（>8K tokens）：先分段摘要 → 再合并摘要（map-reduce）
- 摘要结果保存到 `meetings.summary`，前端会议详情页展示

**会议问答**（`meeting:qa`）：

- 上下文 = title + participants + transcript + summary（限 8K tokens）
- 超长 → 转录先入 RAG（按 meeting 建临时 wiki 索引）→ 检索增强问答

**推送给 Agent**（借鉴 prism-agent `MeetingToAgentService`）：

```
meeting:push-to-agent {meeting_id, agent_id, session_id?}
→ 构建消息: [会议纪要 + 摘要] → 注入 Agent 会话 → 用户可继续追问
→ UI 显示"已推送至 XX Agent，开始分析..."
```

#### 10.3.7 导出

| 格式 | 实现 | 内容 |
|------|------|------|
| Markdown | 直接生成（模板） | 标题/时间/参会人/清洗后转写/摘要 |
| DOCX | `docx-rs` crate | 同 Markdown 内容，样式化标题 |
| 纯文本 | 直接生成 | 简化版 |

- 导出前可选"包含摘要 / 包含翻译"开关
- 导出路径：默认 `{meetings}/{id}/export.{ext}`，前端提供保存对话框（`file:pick` 反向保存）

#### 10.3.8 会议 IPC 命令完整清单

| 命令 | 参数 | 返回 |
|------|------|------|
| `meeting:create` | `{title, participants?}` | `MeetingDto` |
| `meeting:list` | `{}` | `Vec<MeetingDto>` |
| `meeting:get` | `{id}` | `MeetingDto` |
| `meeting:delete` | `{id}` | `()` |
| `meeting:start-recording` | `{id, asr_config}` | `()` |
| `meeting:stop-recording` | `{id}` | `{transcript}` |
| `meeting:pause-recording` | `{id}` | `()` |
| `meeting:resume-recording` | `{id}` | `()` |
| `meeting:cancel-recording` | `{id}` | `()` |
| `meeting:retranscribe` | `{id, asr_config}` | `()` |
| `meeting:clean` | `{id}` | `{cleaned}` |
| `meeting:summary` | `{id}` | `{summary}` |
| `meeting:qa` | `{id, question}` | `{answer}` |
| `meeting:push-to-agent` | `{meeting_id, agent_id, session_id?}` | `()` |
| `meeting:export` | `{id, format, options?}` | `{path}` |
| `asr:backends` | `{}` | `Vec<AsrBackendInfo>`（含语言/健康状态） |
| `asr:model-catalog` | `{}` | `Vec<AsrModelInfo>` |
| `asr:model-installed` | `{}` | `Vec<InstalledAsrModel>` |
| `asr:model-download` | `{model_id}` | `()`（进度走事件） |
| `asr:model-remove` | `{model_id}` | `()` |
| `asr:test` | `{asr_config}` | `{ok, latency_ms, error?}` | 后端连通性测试 |

**事件**：`meeting:transcript` / `meeting:translation` / `meeting:status`（现有）+ 新增 `asr:model-download-progress` / `asr:backend-status`。

**数据库补充**（迁移 003 扩展）：

```sql
-- 会议增加 ASR 配置记录
ALTER TABLE meetings ADD COLUMN asr_kind TEXT;
ALTER TABLE meetings ADD COLUMN asr_model TEXT;
ALTER TABLE meetings ADD COLUMN retranscribed_at INTEGER;

-- ASR 后端配置（用户预设）
CREATE TABLE asr_configs (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,               -- "云端 DashScope" / "本地 SenseVoice"
    kind        TEXT NOT NULL,               -- AsrKind 字符串
    base_url    TEXT,                        -- Custom/兼容端点
    api_key_enc TEXT,                        -- AES-GCM 加密
    model       TEXT,                        -- 模型名
    lang        TEXT,
    is_default  INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
```

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

**市场搜索**（三源聚合，futures join_all 并发，逐源容错）：

```rust
pub async fn search_market(&self, query: &str) -> Vec<SkillSearchHit> {
    let (a, b, c) = tokio::join!(
        search_skills_sh(query), search_claude_plugins(query), search_clawhub(query)
    );
    [a, b, c].concat()   // 每源内部 try 容错
}
```

#### 10.4.1 三源 API 协议细节

| 源 | API | 参数 | 返回字段 | 备注 |
|----|-----|------|----------|------|
| **skills.sh** | `GET https://skills.sh/api/search` | `q` | `{name, description, author, tags, download_url}` | 官方注册中心；下载走 zip |
| **claude-plugins.dev** | `GET https://claude-plugins.dev/api/skills` | `q` | `{name, description, github: owner/repo[/path], tags}` | 按 GitHub 仓库定位 |
| **clawhub.ai** | `GET https://clawhub.ai/api/v1/search` | `query` | `{name, description, source, stats}` | 含 star/下载数统计 |

**统一命中结构**（三源归一化）：

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct SkillSearchHit {
    pub id: String,                  // 源内部 id / 唯一标识
    pub name: String,
    pub description: String,
    pub source: SkillSource,         // SkillsSh | ClaudePlugins | Clawhub | Local
    pub install_source: String,      // 安装指令："skills.sh:xxx" / "github:owner/repo[/path]" / "zip" / "local:path"
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub stars: Option<u64>,          // clawhub 提供，用于排序
    pub url: Option<String>,
    pub installed: bool,             // 是否已安装（按 name/folder_name 匹配）
}
```

#### 10.4.2 搜索流程与去重合并

```
用户输入 query（防抖 300ms）
→ 并发请求三源（每源 5s 超时，超时/失败静默跳过该源，不阻塞）
→ 结果归一化为 SkillSearchHit
→ 合并去重（按 name 归一化：小写 + 去空格 + 去"agent"/"skill"后缀）
→ 排序（内置权值）：stars>0 优先 · 已安装排后 · 描述含精确词优先
→ 缓存：每 query 缓存 60s（内存 LRU），翻页/筛选本地处理
```

**排序规则**（`score = 0.5·normalized_stars + 0.3·desc_relevance + 0.2·source_trust`）：

| 维度 | 计算 |
|------|------|
| `normalized_stars` | `min(stars, 5000) / 5000`（对数缩放更佳：`log10(1+stars)/log10(5001)`） |
| `desc_relevance` | 描述中包含完整 query → 1.0；包含任一 token → 0.5；否则 0 |
| `source_trust` | skills.sh=1.0 / clawhub=0.9 / claude-plugins=0.8 |

#### 10.4.3 前端搜索 UI（SkillMarket）

```
┌─ 技能市场 ────────────────────────────────────────┐
│ 🔍 [搜索技能...                          ] (⌘F)   │
│ 源过滤: [全部] [skills.sh] [Claude插件] [ClawHub]  │  ← 单选 chips
│ ┌────────────────────┐ ┌────────────────────┐     │
│ │ 🧩 技能名          │ │ 命中来源徽标        │     │
│ │    描述 2 行省略   │ │ ⭐ 1.2k  · 🏷 3     │     │
│ │    [安装]  [详情]   │ │   [已安装 ✓]        │     │
│ └────────────────────┘ └────────────────────┘     │
│ 加载中骨架 / 空态("未找到，试试换个关键词")          │
│ 已加载 42 个 · 源: 3/3 可用（1 源超时已跳过）       │  ← 源健康提示
└──────────────────────────────────────────────────┘
```

- **筛选**：源 chips + 本地过滤（tags/作者）
- **详情**：SkillDetail 弹窗——README 预览 + 安装前置条件（依赖/权限）+ 截图（如有）
- **安装确认**：显示来源 + 目标目录 + 磁盘占用预估 → 确认 → `skill:install` → 进度 Toast
- **已安装标记**：名称匹配 `skills` 表 folder_name → 徽标"已安装"→ 按钮变"重新安装"
- **本地技能**：顶部独立分区显示 `skill:list-local` 结果（项目 `.claude/skills`）

#### 10.4.4 安装状态与依赖检查

```rust
pub async fn install(&self, source: &str) -> Result<InstalledSkill, AppError> {
    // 1. 解析 install_source 前缀（skills.sh/github/zip/local）
    // 2. 依赖预检：github 需 git 命令可用；zip 需解压库；skills.sh 需网络
    //    → 失败返回可读错误（如 "未检测到 git，请先安装"）
    // 3. 执行安装（§10.4 主流程）
    // 4. 安装后自动 health-check：SKILL.md 可解析 + 引用脚本存在
    // 5. 返回 InstalledSkill（含 folder_name/版本/启用状态）
}
```

**重名冲突策略**：目标目录已存在 → 对比 content_hash——相同则提示"已安装最新版"；不同则询问"覆盖（备份 .bak）/ 跳过 / 装为副本"。

**版本更新**：`skill:install` 同源重复执行 = 更新（hash 变更 → 覆盖 + 保留旧版 .bak + 标记 `updated_at`）；市场详情页显示"有新版本"徽标（源端 latest hash ≠ 本地 hash）。

### 10.5 翻译 + OCR 详细设计

#### 10.5.1 翻译服务（TranslateService）

**能力总览**：

| 能力 | 说明 |
|------|------|
| 多目标语言 | 中/英/日/韩/法/德/西/俄 等（ISO 639-1 码） |
| 自动检测 | `source=auto` → 先检测语言再翻译（检测 + 翻译两次调用，或一次 prompt 内完成） |
| 多 Provider | 复用 LLM Provider 体系（OpenAI/Anthropic/MiMo/DashScope/Ollama），可配置默认翻译模型 |
| 专用翻译模型（可选） | 若配置了翻译专用模型（如 DeepSeek 翻译版），优先使用；否则回退 summary_model |
| 批量翻译 | 文本列表批处理（并发 + 结果对应返回） |
| 历史记录 | 落库 `translate_history`，支持搜索/删除/复制 |
| 术语表 | 用户定义术语映射（见 10.5.2） |
| 文件翻译 | 整文件翻译（Markdown/纯文本，保留格式） |

**核心实现**：

```rust
// data/services/translate_service.rs
pub struct TranslateService {
    db: Database,
    model_service: ModelService,
    glossary: GlossaryService,     // 术语表
    histories: Mutex<LruCache<String, String>>,  // 短文本翻译缓存（key: text|src|tgt）
}

impl TranslateService {
    pub async fn translate(
        &self, text: &str, source: Option<&str>, target: &str,
        model_id: Option<&str>,
    ) -> Result<TranslateResult, AppError> {
        let source_lang = source.unwrap_or("auto");

        // 1. 缓存命中（仅 <500 字符文本，TTL 24h）
        let cache_key = format!("{text}|{source_lang}|{target}");
        if text.chars().count() <= 500 {
            if let Some(hit) = self.histories.lock().await.get(&cache_key) {
                return Ok(TranslateResult { translated: hit.clone(), source_lang, from_cache: true });
            }
        }

        // 2. 组装提示（含术语表注入）
        let glossary_ctx = self.glossary.build_prompt(text, source_lang, target);
        let prompt = format!(
            "Translate the following text from {source_lang} to {target}.\n\
             {glossary_ctx}\
             Rules: preserve code, formatting, proper nouns and placeholders like {{var}};\n\
             output ONLY the translation without quotes.\n\n{text}"
        );

        // 3. 选模型：显式指定 > 翻译专用模型 > summary_model
        let model = self.resolve_model(model_id).await?;
        let resp = model.generate(GenerationRequest {
            messages: vec![ChatMessage { role: ChatRole::User, content: MessageContent::Text(prompt.into()), name: None }],
            temperature: Some(0.3),   // 翻译用低温，保持一致性
            ..Default::default()
        }).await?;

        // 4. 校验输出（去包裹引号/代码块围栏）
        let cleaned = strip_artifacts(&resp.text);

        // 5. 写历史 + 缓存
        self.db.insert_translate_history(text, &source_lang, target, &cleaned).await?;
        self.histories.lock().await.put(cache_key, cleaned.clone());
        Ok(TranslateResult { translated: cleaned, source_lang, from_cache: false })
    }

    /// 批量翻译：并发执行，保持输入顺序
    pub async fn batch(&self, texts: &[String], source: Option<&str>, target: &str) -> Result<Vec<TranslateResult>, AppError> {
        let mut results = Vec::with_capacity(texts.len());
        let sem = Arc::new(Semaphore::new(4));   // 限并发
        let mut handles = Vec::new();
        for t in texts {
            let sem = sem.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                self.translate(t, source, target, None).await
            }));
        }
        for h in handles { results.push(h.await??); }
        Ok(results)
    }

    /// 整文件翻译（Markdown 保留结构）：按段落分块 → 批量翻译 → 重组
    pub async fn translate_file(&self, content: &str, source: Option<&str>, target: &str) -> Result<String, AppError> {
        let blocks = split_markdown_blocks(content);       // 代码块/行内代码/正文分离
        let mut out = String::new();
        for b in blocks {
            match b.kind {
                BlockKind::Code => out.push_str(&b.text),                    // 代码不翻译
                BlockKind::Text => {
                    let r = self.translate(&b.text, source, target, None).await?;
                    out.push_str(&r.translated);
                }
                BlockKind::Heading => { /* 标题翻译（保留 # 前缀） */ }
            }
        }
        Ok(out)
    }
}
```

**输出清洗**（`strip_artifacts`）：去除 LLM 常见的包裹——首尾引号（`"`/`"`）、``` 代码围栏、`Translation:` 前缀。

#### 10.5.2 术语表（Glossary）

```sql
-- 术语表（迁移 005_glossary.sql）
CREATE TABLE glossary_terms (
    id           TEXT PRIMARY KEY,
    source_lang  TEXT NOT NULL,
    target_lang  TEXT NOT NULL,
    source_term  TEXT NOT NULL,          -- 原文术语
    target_term  TEXT NOT NULL,          -- 强制译文
    category     TEXT,                   -- 产品名/人名/公司/专业术语
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   INTEGER NOT NULL,
    UNIQUE (source_lang, target_lang, source_term)
);
```

- `build_prompt` 将启用的术语注入提示："Use these exact translations: 术语A → 译文A; ..."
- 前端管理 UI：术语列表 + 增删改 + 导入/导出 CSV；支持从翻译历史"添加为术语"
- 作用域：全局 + 项目级（`.prism/glossary.csv`，工作目录内自动加载）

#### 10.5.3 OCR 服务（OcrService）

**能力总览**：

| 后端 | 类型 | 优点 | 依赖 |
|------|------|------|------|
| **MiMo OCR** | 在线 API | 中文准确率高、表格/版面 | 无 |
| **DashScope OCR** | 在线 API | 文档版式还原（paraformer-ocr） | 无 |
| **本地 tesseract** | 本地 | 离线、免费、多语言 | tesseract 可执行文件（按需检测） |
| **PaddleOCR（可选）** | 本地 | 中文最佳开源 | 需用户安装（外部服务） |

```rust
// data/services/ocr_service.rs
pub struct OcrService {
    providers: HashMap<OcrProvider, Box<dyn OcrBackend>>,
}

#[async_trait]
pub trait OcrBackend: Send + Sync {
    fn kind(&self) -> OcrProvider;              // MiMo | DashScope | Tesseract
    async fn recognize(&self, image_path: &Path, lang: Option<&str>) -> Result<OcrResult, AppError>;
}

pub struct OcrResult {
    pub text: String,
    pub blocks: Vec<OcrBlock>,     // 版面块（含置信度）
    pub lang: String,
    pub provider: OcrProvider,
}

pub struct OcrBlock {
    pub text: String,
    pub bbox: (f32, f32, f32, f32),   // 归一化坐标
    pub confidence: f32,
    pub kind: BlockKind,              // Text | Table | Title
}
```

**执行策略**：默认在线优先（MiMo → DashScope 失败回退）；用户可在设置选择"仅本地"模式（tesseract 检测不到则报错并提示安装）。识别结果可一键送入翻译（`translate:translate`）。

**OCR 前端**（TranslatePage 的"图片翻译"区）：

```
┌─ OCR 翻译 ──────────────────────────┐
│ [拖拽图片 / 点击选择]  (file:pick)   │
│ ┌──────────────────────────────┐   │
│ │ 图片预览（缩略）              │   │
│ │ [识别文字] [识别并翻译→EN]    │   │
│ └──────────────────────────────┘   │
│ 识别结果（可编辑）：                 │
│ ┌──────────────────────────────┐   │
│ │ 识别出的文本…                 │   │
│ │ （provider 徽标 + 置信度）     │   │
│ └──────────────────────────────┘   │
│ 翻译结果：                         │
│ ┌──────────────────────────────┐   │
│ │ Translated text…              │   │
│ │ [复制] [保存到历史]            │   │
│ └──────────────────────────────┘   │
└──────────────────────────────────┘
```

#### 10.5.4 翻译 IPC 命令

| 命令 | 参数 | 返回 |
|------|------|------|
| `translate:translate` | `{text, source?, target, model_id?}` | `{translated, source, from_cache}` |
| `translate:batch` | `{texts, source?, target}` | `Vec<TranslateResult>` |
| `translate:file` | `{path, source?, target, out_path?}` | `{content}` | 整文件翻译（预览或落盘） |
| `translate:history` | `{query?, limit?, offset?}` | `{items, total}` |
| `translate:detect` | `{text}` | `{lang, confidence}` |
| `glossary:list` | `{lang_pair?}` | `Vec<GlossaryTerm>` |
| `glossary:add` | `{term}` | `()` |
| `glossary:remove` | `{id}` | `()` |
| `glossary:import-csv` | `{path}` | `{imported, failed}` |
| `ocr:recognize` | `{image_path, lang?, provider?}` | `OcrResult` |
| `ocr:providers` | `{}` | `Vec<OcrProviderInfo>` | 可用性与版本 |

**事件**：无流式需求（翻译是原子请求），但批量翻译提供 `translate:batch-progress` 事件（`{done, total}`）。

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

#### 10.6.1 阶段模板系统（详细设计）

**模板格式**（JSON 存储在 `workflows` 表 definition 字段；预置模板编译期内嵌为 Rust 常量，首次启动写入）：

```rust
/// 阶段模板 = 可复用阶段定义（预置 + 用户保存）
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
}

/// 预置工作流定义（= 阶段模板的有序组合 + 输入声明）
pub struct BuiltinWorkflow {
    pub id: String,                 // "deep-research"
    pub name: String,
    pub inputs: Vec<TaskInput>,     // 复用 §9.9.1 TaskInput 定义
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

**校验规则**（`validate_definition`，§9.9.1 `task:validate` 复用）：

- 模板中引用的 `{{stage.x.output}}` 必须存在 `depends_on` 依赖（或为前序阶段）
- 变量引用缺失 → 构建期报错（带缺哪个变量）
- 阶段图环检测（拓扑排序失败 → 拒绝）
- 每阶段输出注入下一阶段前做 `truncate:8000` 上限保护

#### 10.6.4 模板管理与用户自定义

| 操作 | 说明 |
|------|------|
| 预置模板 | 内嵌常量，只读；首次启动写入 workflows 表 `source=builtin` |
| 用户模板 | `task:save-template`（§9.9.1）保存为 `source=user`，可编辑/删除 |
| 阶段模板复用 | 用户可保存单个 StageTemplate 到 `stage_templates` 表，编排时拖入 |
| 模板继承 | 用户模板可基于预置修改（复制 → 改 inputs/stages） |

```sql
-- 迁移 006_workflow_templates.sql
CREATE TABLE stage_templates (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    role          TEXT NOT NULL,
    description   TEXT,
    prompt_template TEXT NOT NULL,
    tools         TEXT NOT NULL DEFAULT '[]',
    max_iterations INTEGER DEFAULT 10,
    source        TEXT NOT NULL DEFAULT 'user',   -- builtin | user
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
```

### 10.7 记忆系统

**设计参考**：MiMo-Code 记忆架构（`src/memory/paths.ts` 4 scope + 9 type、checkpoint-writer 子 agent 唯一策展、SQLite FTS5 索引 + BM25 搜索、主动召回协议）。本设计移植为 Rust + Tauri 实现。

#### 10.7.1 分层与存储路径

```
{app_data}/memory/
├── global/MEMORY.md                 # 全局记忆：跨项目用户偏好/规则
└── projects/{pid}/MEMORY.md         # 项目记忆：pid = 仓库绝对路径 sha256 前 12 位
    └── {workdir}/.prism/
        ├── memory.md                # 项目记忆（工作目录内，可随仓库提交）
        └── notes.md                 # 项目草稿本
sessions/{session_id}/
├── checkpoint.md                    # 会话检查点（11 节结构，writer 专属）
├── notes.md                         # 会话草稿本
└── tasks/{task_id}/progress.md      # 任务进度（子 agent 汇报）
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

#### 10.7.2 存储实现（SQLite FTS5 索引 + Markdown 文件）

```rust
// data/services/memory/store.rs
pub struct MemoryStoreImpl {
    db: Database,
    base_dir: PathBuf,                     // {app_data}/memory
    fts: RwLock<FtsIndex>,                 // 内存中 FTS5 句柄
}

// SQLite FTS5 索引表（迁移 005_memory.sql）
// CREATE VIRTUAL TABLE memory_fts USING fts5(
//     body, fingerprint, scope UNINDEXED, type UNINDEXED, path UNINDEXED
// );

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

    /// BM25 搜索（对齐 MiMo-Code memory tool：OR 连接 token，相对分数下限 0.15）
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

**搜索细节**（对齐 MiMo-Code `memory` 工具语义）：

- **token 化**：`query` 按非字母数字切分 → 每个 token 一个词项（`A OR B`），不要求全部命中
- **相对分数下限**：`score >= max_score * 0.15` 才返回（滤掉低相关噪音）
- **scope/type 过滤**：默认全 scope；支持 `scope=projects`、`type=checkpoint` 等精确过滤
- **命中即权威**：返回的路径可直接 Read 全文（snippet 只展示前 ~200 字符）

#### 10.7.3 checkpoint-writer 策展机制（核心，移植 MiMo-Code）

**角色**：checkpoint-writer 是一个独立子 agent（Rust 内通过 AutoAgents Actor 实现），是会话 checkpoints 的**唯一策展人**。

```
触发条件（上下文使用率阈值，默认 40% / 60% / 80%）：
  ├─ 达到阈值 → 唤醒 writer
  ├─ 用户显式触发（/checkpoint 命令）
  └─ 会话结束（summary 写入）

writer 执行：
  1. 读取本会话对话原文（messages 表，role 过滤）
  2. 生成/更新 checkpoint.md（11 节固定结构，见 10.7.4）
  3. 提炼新知识 → 追加/更新 MEMORY.md（Rules / Architecture decisions / Discovered durable knowledge）
  4. 清理过期任务（done/abandoned 归档）

约束：
  - 主 agent 不得直接写 checkpoint.md（仅可编辑 MEMORY.md 规则类 + notes.md）
  - writer 每次运行有 token 预算（如 8K），超限拆分
```

**checkpoint.md 11 节结构**（对齐 MiMo-Code）：

```markdown
# Session Checkpoint
## 1. Active intent          — 当前会话目标
## 2. Next action           — 下一步
## 3. Directives            — 用户指令/优先级
## 4. Task tree             — 任务树（含状态）
## 5. Current work          — 正在进行的任务详情
## 6. Files                 — 涉及文件
## 7. Learnings             — 学到的知识
## 8. Errors                — 遇到的错误/教训
## 9. Live resources        — 运行中的资源（端口/进程）
## 10. Design decisions     — 设计决策记录
## 11. Open notes           — 未决问题
```

**notes.md 草稿本**：主 agent 的合法 scratchpad（引用/未决问题/跨项目观察），writer 在 checkpoint 时整理归纳进对应节。

#### 10.7.4 注入与召回（Active Recall）

**上下文重建注入**（对齐 MiMo-Code 的 4 段注入，token 预算可配置）：

| 段 | 内容 | 预算（默认） |
|----|------|--------------|
| checkpoint.md | 会话检查点（全量或预算截断） | 11K tokens |
| MEMORY.md（project + global） | 项目/全局记忆 | 10K tokens |
| notes.md | 会话草稿 | 6K tokens |
| tasks/*/progress.md | 进行中任务的进度 | 4K tokens |

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

**事件**：`memory:changed`（文件被 writer/agent 更新后广播，前端记忆面板刷新）。

#### 10.7.5 记忆前端（设置页 → 记忆管理）

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

### 10.8 文件与附件

- `file:pick` 使用 Tauri dialog 插件
- `file:parse` 支持 txt/md/pdf/doc/docx/html/json/csv/xml → 文本（`pdf-extract`、`docx-rs`、`scraper`、`html2md`）
- 对话附件：解析后作为 user 消息 attachments 元数据，注入 prompt 或走 RAG

---

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

---

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
| 6 | **AudioStreamManager 时序丢块**：renderer 先发 chunk，主进程 stream 后建 | 录音开头 1-2s 音频丢失 | §10.3.2 已规避：先建 stream + `pending` 缓冲 flush |
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
| 本地 ASR 二进制（sherpa-onnx） | `sherpa-onnx.exe` | `sherpa-onnx` | `sherpa-onnx` | 按平台打包对应二进制（§10.3.1）；未找到时降级提示 |
| LSP 可执行文件查找 | `where` 命令 | `which` | `which` | `std::process` 按 `cfg!(windows)` 分支选 `where`/`which`（§9.10.5） |
| 路径分隔符 | `\` | `/` | `/` | 一律 `std::path::PathBuf`，禁止字符串拼接路径 |
| 命令行工具调用 | `cmd /c` | `sh -c` | `sh -c` | 统一封装 `run_command(cmd, args)` 抽象（§10.3/§10.5 复用） |
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
| 45 | **删目录前未停 watcher**：fs watcher 事件复活已删目录 | 目录删不掉 | 删除前先 dispose watcher（§9.10.7 fs:watch 生命周期） |
| 46 | **Windows 相对路径反斜杠**：断言/比较用字符串字面 | 测试失败、路径不匹配 | 一律 `std::path::Path` 操作；测试断言用 `path.join()` 构造预期（§14.5 已提） |
| 47 | **覆盖文件前未查 git 历史**：误删已提交的回归测试 | 静默丢代码 | 覆写/删除前 `git log -- <path>` 核对；Rust 项目同样适用（删除源文件前确认） |
| 48 | **缓存致旧代码生效**：rolldown-vite 不清缓存 | 改代码无效 | Tauri dev：改 Rust 后等 cargo 增量编译；前端 Vite 遇怪问题先清 `node_modules/.vite` |
| 49 | **CI 交叉编译/发布陷阱**：macOS 无法从 Windows 交叉编译；electron-builder 隐式 publish | 构建失败/误发布 | CI 三平台各自原生 runner；`tauri build` 显式 `--no-bundle` 或控制发布动作（§14.5） |
| 50 | **主进程测试需 mock 全局单例**：模块加载期执行 app.getPath | 测试崩 | Rust 侧依赖注入（AppState 注入 paths），测试用临时目录，无全局状态 |
| 51 | **批量写 JSON 覆写现有值**：按路径 setPath 把现有值置空 | 配置丢失 | 配置文件更新用读-改-写原子流程（temp + rename），禁止局部覆写 |

---

## [S3] Out of Scope

- **会议 ASR 引擎**：支持多后端（云端 DashScope/MiMo/Whisper/Azure + 本地 sherpa-onnx/Vosk/FunASR-WS/Custom），见 §10.3；**本地模型（sherpa/vosk）默认不打包，按需下载**；Azure 为可选后端（需 Key）
- **移动端**：Tauri 支持移动但本次仅桌面（Win/macOS/Linux）
- **云端同步**：WebDAV/S3 备份（后续迭代）
- **国际化**：i18n 框架（后续迭代，UI 先中文）
- **Electron 兼容层**：不保留原 Node.js 代码，完全重写
- **插件系统**：不对齐 VS Code 扩展体系；通过技能 + MCP 满足扩展需求
- **语音合成（TTS）**：本次不做
- **本地大模型推理**：Ollama 仅作为远程 provider 接入，不在应用内嵌推理引擎

## MVP 范围与阶段规划

**目标**：先交付可用的 Agent 核心闭环——创建 Agent → 对话 → 流式生成 → 工具/MCP 调用 → 多 Agent 任务 → 记忆，配合最简对话界面。面板（主页面板/侧边栏）与扩展功能（Wiki/RAG、翻译/OCR、会议）在 MVP 之后迭代。

**阶段划分**（任务 ID 保持全局唯一，不随阶段重排，不影响后续开发引用）：

### Phase 1 — MVP（Agent 核心闭环）

| 任务 | 内容 | MVP 验收 |
|------|------|----------|
| T1 | 项目初始化 | Tauri + Svelte 5 可 `npm run tauri dev`；**CI 三平台构建矩阵（Windows/macOS/Linux）跑通** |
| T2 | 设计系统（MVP 子集：设计令牌 + 基础组件 15 个） | 主题切换可用 |
| T3 | 数据库层 | 迁移全跑通，CRUD 可用 |
| T4 | ADK 组件层 | 三个 Trait + PromptBuilder 编译通过 |
| T5 | Rig 核心层 | 至少 2 个 Provider（OpenAI + Ollama）可流式对话 |
| T6 | 服务层（MVP 子集） | Agent/Session/Chat/Model 命令可用；dashboard/usage/workspace/lsp 命令随 Phase 2 补充 |
| T8 | MCP 层（MVP 子集：stdio + http 传输） | 挂一个本地 MCP 服务器，工具可调用 |
| T9 | 技能系统（MVP 子集：安装/启停/注入） | 安装一个技能并在对话中生效；市场搜索随 Phase 2 |
| T11 | 对话前端（MVP 核心，不依赖侧边栏） | 三栏布局 + 流式渲染 + 会话切换，可完整对话 |
| T15 | AutoAgents 编排（MVP 子集：Actor/Coordinator + 1 个预置工作流"深度研究"） | 面板任务设计区可用前，先支持预置工作流 CLI/简单触发 |
| T16 | 记忆系统（MVP 子集：分层记忆 + 会话注入，FTS 搜索随 Phase 2） | 跨会话记忆生效 |

**MVP 依赖链**：T1 → T2/T3 → T4 → T5 → T6 → T11（对话闭环）；T8/T9 并行挂接；T15/T16 在 T5 后串行。MVP 完成标志：用户能创建 Agent、对话、调用 MCP 工具、跑通"深度研究"工作流，重启后记忆仍在。

### Phase 2 — 面板功能（Agent 功能之后优先）

| 任务 | 内容 |
|------|------|
| T10 | Agent 侧边栏（六 Tab 运行时上下文） |
| T7 | 主页面板 + 多 Agent 任务设计区（依赖 T15 完成） |
| T6 补充 | dashboard/usage/workspace/lsp 命令落地 |
| T9 补充 | 市场三源搜索 + 去重排序 |

### Phase 3 — 扩展功能

| 任务 | 内容 |
|------|------|
| T12 | Wiki + RAG（write_ai / 分块 / 混合检索） |
| T13 | 翻译 + OCR |
| T14 | 会议系统（8 后端 ASR） |
| T17 | 安全与设置（API Key 加密可提前在 T6 引入基础版） |
| T18 | 测试与验证（贯穿各阶段，Phase 3 汇总） |

---

## Tasks

**Phase 1 — MVP（Agent 核心闭环）**

- [ ] T1: 项目初始化 — Tauri 2.x + Svelte 5 + SvelteKit 脚手架、Cargo 工作区、Vite 配置、**CI 三平台构建矩阵（Windows/macOS/Linux）** (covers: S2-1, S2-2)
- [ ] T2: 设计系统（MVP 子集） — 设计令牌（colors/typography/spacing/motion）、glass 工具类、基础组件库 15+ (covers: S2-9.1~9.4; depends: T1)
- [ ] T3: 数据库层 — sqlx 连接池 + 5 个迁移 + 全部模型 (covers: S2-5; depends: T1)
- [ ] T4: ADK 组件层 — ModelProvider/ToolExecutor/MemoryStore Trait + PromptBuilder + AgentError (covers: S2-3.2; depends: T3)
- [ ] T5: Rig 核心层 — RigAgent agentic loop + 流式管道 + 内置工具 + Provider 适配器（MVP 先 OpenAI/Ollama，其余 Phase 3 补） (covers: S2-3.3, S2-7; depends: T4)
- [ ] T6: 服务层（MVP 子集） — Agent/Session/Chat/Model 服务；dashboard/usage/workspace/lsp 命令随 Phase 2 落地 (covers: S2-8; depends: T3, T5)
- [ ] T8: MCP 层（MVP 子集） — McpTransport stdio/http 两传输 + McpRuntime + 工具目录缓存 + commands (covers: S2-6; depends: T5)
- [ ] T9: 技能系统（MVP 子集） — 安装/卸载/启停 + PromptBuilder 注入；市场三源搜索随 Phase 2 (covers: S2-10.4; depends: T4, T8)
- [ ] T15: AutoAgents 编排（MVP 子集） — Actor/Coordinator/WorkflowEngine + "深度研究"预置工作流 + render_template；其余模板随 Phase 2 (covers: S2-3.4, S2-10.6; depends: T5)
- [ ] T16: 记忆系统（MVP 子集） — 分层记忆（global/projects/sessions）+ 会话注入；FTS 搜索/checkpoint-writer 随 Phase 2 (covers: S2-10.7; depends: T3, T5)
- [ ] T11: 对话前端（MVP 核心，不依赖侧边栏） — 三栏布局 AppShell + MessageList/Composer/流式渲染 + 会话管理 (covers: S2-9.5~9.8; depends: T2, T6)

**Phase 2 — 面板功能**

- [ ] T10: **Agent 侧边栏** — AgentSidebar 六 Tab 详设（用量进度条/工作目录切换/指令文件注入/LSP 检测与诊断/文件树懒加载） + context:agent 聚合命令 + session:inject-file/lsp:detect/fs:watch 命令 (covers: S2-9.10, S2-9.10.1~9.10.7; depends: T2, T6, T7, T15)
- [ ] T7: **主页面板** — HomePage + AgentLauncher + UsageStats/Chart + Skill/Mcp Overview + 多 Agent 任务设计区（TaskDesigner 画布/运行器/历史） + task 命令 (covers: S2-9.9, S2-9.9.1; depends: T2, T6, T9, T15)
- [ ] T11 增强: 对话前端嵌入 Agent 侧边栏（T10 完成后合并） (covers: S2-9.10; depends: T10)
- [ ] T9 补充: 市场三源搜索（协议细节/去重排序/缓存） (covers: S2-10.4.1~10.4.4; depends: T9)
- [ ] T6 补充: dashboard/usage/workspace/lsp 命令 + 单价表与用量聚合 (covers: S2-9.9 数据源; depends: T6, T10)

**Phase 3 — 扩展功能**

- [ ] T12: Wiki + RAG — WikiService + write_ai 计划执行（结构化操作/校验回滚/工具接入）+ 分块/嵌入/混合检索 + 摄取后台任务 + 前端知识库页 (covers: S2-10.1, S2-10.1.1, S2-10.2; depends: T3, T5)
- [ ] T13: 翻译 + OCR — TranslateService（多 Provider/批量/文件翻译/术语表/缓存）+ OcrService 多后端 + 前端翻译页 (covers: S2-10.5, S2-10.5.1~10.5.4; depends: T5)
- [ ] T14: 会议系统 — AsrBackend 可插拔架构（8 后端协议级实现）+ 本地 sherpa-onnx 集成 + 模型下载管理 + 录音流通道 + 离线二次转写 + 清洗/摘要/问答/推送 Agent/导出 + 前端 (covers: S2-10.3, S2-10.3.1~10.3.8; depends: T5, T6)
- [ ] T17: 安全与设置 — Key 加密存储 + capabilities 权限 + 设置页 (covers: S2-12; depends: T6)
- [ ] T18: 测试与验证 — 单元测试（分块/检索/错误映射/任务校验）、集成测试（对话流/任务流）、性能基准、**三平台打包验证（Windows NSIS / macOS dmg / Linux deb+AppImage）**；**§14 规避回归**：模型 ID 格式/upsert 幂等/音频时序丢块/目录穿越/配置合并/事件清理 (covers: S2-11, S2-13, S2-14; depends: T6, T8, T12, T15)
