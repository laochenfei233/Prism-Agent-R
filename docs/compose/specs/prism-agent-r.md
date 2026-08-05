---
feature: prism-agent-r
status: in-progress
updated: 2026-08-05
branch: main
commits: # filled at delivery
platform: windows | macos | linux
---

# Prism Agent R — Rust 重构版

> **平台定调：本项目为跨平台桌面应用，正式支持 Windows、macOS、Linux 三大桌面操作系统。**
> 所有功能（Agent 核心、面板、MCP、会议 ASR、Wiki/RAG、翻译/OCR、记忆系统）在三平台行为一致；
> 前端使用 WebView（Tauri 2.x 内置），后端 Rust 编译原生二进制（无 Node.js 运行时依赖）。
> 涉及平台差异的板块（路径处理、LSP 检测、本地 ASR 二进制、CI 构建矩阵、打包分发）已在各阶段文档中明确标注。

> **读者假设**：本系列文档面向熟悉 Rust（tokio/sqlx/serde）、Svelte 5（runes）、Tauri 2.x（IPC/WebView）的开发者；不解释语言/框架基础语法。

## 📚 文档导航

本设计文档按 **开发阶段** 拆分为 3 份详细设计 + 本总索引。按需阅读对应文件：

| 文件 | 阶段 | 内容 | 约行数 |
|------|------|------|--------|
| [`phase1-core.md`](../design/phase1-core.md) | **Phase 1 — Agent 核心闭环** | §3 后端三层架构 · §4 目录结构 · §5 数据库（含 §5.7 数据存储跨阶段基础：PRAGMA/消息FTS/分页/保留/索引） · §6 MCP · §7 流式响应 · §8 IPC 命令 · §9.1-9.8 前端基础（设计系统+对话） · §10.4 Skill（含市场搜索详设） · §10.6 工作流引擎+模板系统 · §10.7 记忆系统（完整设计） · §10.8 文件 · §11 错误日志 · §12 安全 · §13 性能基线（§13.1 见 phase3） · §14 旧版规避 | ~3150 |
| [`phase2-panel.md`](../design/phase2-panel.md) | **Phase 2 — 面板功能** | §9.9 主页面板 · §9.10 Agent 侧边栏（六 Tab） · §10.4.1-10.4.4 市场搜索 · §10.10 人机协同（工具审批） · §5.7.4 会话标题搜索（迁移 012） | ~889 |
| [`phase3-extend.md`](../design/phase3-extend.md) | **Phase 3 — 扩展功能** | §10.1 Wiki · §10.2 RAG · §10.3 会议 · §10.5 翻译/OCR · §10.9 反思 · §10.11 目标监控 · §10.12 安全护栏 · §10.13 评估监控 · §11A 无障碍 · §13.1 上下文压缩 · §5.7.5 翻译历史搜索（迁移 013） | ~1828 |
| 本文件 | 总览 | 设计模式参考 · 问题定义 · 架构总览（含 §1.1/§1.2） · 技术选型 · MVP 规划 · 各功能 MVP 清单 · [S4] 错误矩阵 · [S5] 功能建议 · Tasks · Phase 1 完成报告 | ~817 |

**阅读建议**：
- **新对话/新 agent 起步**：先读本索引（[S0]/[S1]/§1/§2 + MVP 清单 + Tasks）了解全局，再按任务阶段读对应详细文件。
- **做 Phase 2/3 任务**：读对应阶段文件；若涉及后端基础（数据库/流式/IPC 命名），回查 `phase1-core.md`。
- **数据存储是横切关注点**：§5.7 完整设计在 `phase1-core.md`（建库即做 PRAGMA/索引/分页/保留策略）；Phase 2/3 各阶段的 FTS 搜索补充在对应文件（§5.7.4 会话→迁移 012 / §5.7.5 翻译→迁移 013）。新增表/索引必须同步更新 §5.7.7 关键索引，且**迁移编号必须递增**（§14.3 #28：禁止在已应用迁移上追加）。
- **跨文件引用**：各文件保留原章节编号（§N），引用时按「阶段文件 → §N」定位；§9/§10 章节头分散在多个文件（§9.1-9.8 在 phase1、§9.9-9.10 在 phase2；§10.1-10.3/10.5/10.9-10.13 在 phase3、§10.4/10.6-10.8 在 phase1）。

**迁移编号总表**（唯一权威，新增迁移必须在此登记并递增编号）：

| 迁移 | 内容 | 表 | 定义位置 | 阶段 |
|------|------|-----|---------|------|
| 001_init | 核心表 | providers/models/agents/sessions/messages/skills/mcp_servers + 关联表 | phase1 §5.2 | 🟦 |
| 002_rag | RAG 表 | wikis/rag_documents/rag_chunks | phase1 §5.3 | 🟩 |
| 003_meeting | 会议表 | meetings/meeting_transcripts | phase1 §5.4 | 🟩 |
| 004_workflow | 工作流/翻译/偏好 | workflows/workflow_runs/translate_history/preferences | phase1 §5.5 | 🟦 |
| 005_glossary | 术语表 | glossary_terms | phase3 §10.5.2 | 🟩 |
| 006_memory | 记忆 FTS | memory_fts | phase1 §10.7.2 | 🟧 |
| 007_workflow_templates | 阶段模板 | stage_templates | phase1 §10.6.4 | 🟧 |
| 008_agent_traces | 评估轨迹 | agent_traces | phase3 §10.13.1 | 🟩 |
| 009_message_search | 消息 FTS | messages_fts | phase1 §5.7.2 | 🟦 |
| 010_indexes | 性能索引 | idx_messages_id 等 | phase1 §5.7.7 | 🟦 |
| 011_asr | ASR 配置 | asr_configs + meetings 扩展 | phase3 §10.3.8 | 🟩 |
| 012_session_fts | 会话标题 FTS | sessions_fts | phase2 §5.7.4 | 🟧 |
| 013_translate_fts | 翻译历史 FTS | translate_fts | phase3 §5.7.5 | 🟩 |
| 014_session_archive | 会话归档 | sessions.archived_at 列 | phase1 §9.5.1 | 后续 |
| 015_prompt_templates | 提示词模板 | prompt_templates | phase1 §9.8.2 | 后续 |
| 016_workflow_versions | 工作流版本 | workflow_versions | phase1 §10.6.4.1 | 后续 |

> ⚠️ **本文档由原单文件 `docs/compose/specs/prism-agent-r.md` 按阶段拆分而来**，章节编号与设计内容保持不变。

## Report

**当前进度**：Phase 1（MVP Agent 核心闭环）已完成（T1-T16，见本文件「MVP Phase 1 完成报告」）；Phase 2（面板功能）与 Phase 3（扩展功能）进行中。

## [S0] 设计模式参考

### Agentic Design Patterns

本设计参考 *Agentic Design Patterns*（Antonio Gulli 著，Springer 2025）中的 21 种智能体设计模式，将模式映射到 Prism Agent R 的架构中，确保设计覆盖完整。

**核心模式映射**：

| 设计模式 | 书中章节 | Prism Agent R 覆盖 | 状态 |
|----------|----------|-------------------|------|
| 提示词链 (Prompt Chaining) | Ch.1 | Workflow 阶段模板 `render_template`，前一阶段输出注入下一阶段 | ✅ 已覆盖 |
| 路由 (Routing) | Ch.2 | Coordinator 按角色匹配 AgentActor，任务派发到合适角色 | ✅ 已覆盖 |
| 并行化 (Parallelization) | Ch.3 | tokio 并发 + TaskScheduler 任务池，多 Agent 可并行执行 | ✅ 已覆盖 |
| 反思 (Reflection) | Ch.4 | §10.9 反思模式：生产者-评审者循环 + ReflectionConfig | ✅ 已设计 |
| 工具使用 (Tool Use) | Ch.5 | ToolExecutor trait + MCP 工具注册表 + RigAgent 内置/MCP 分发 | ✅ 已覆盖 |
| 规划 (Planning) | Ch.6 | Workflow 定义 = 动态计划；预置工作流 = 固定计划；LLM 可通过工具自行规划 | ✅ 已覆盖 |
| 多智能体协作 (Multi-Agent) | Ch.7 | AutoAgents Actor 模型 + Coordinator 层次化/顺序/辩论协作 | ✅ 已覆盖 |
| 记忆管理 (Memory) | Ch.8 | 分层记忆 + checkpoint-writer + FTS5 搜索 + Active Recall 协议 | ✅ 已覆盖 |
| 学习与适应 (Learning) | Ch.9 | **未覆盖**：Agent 无显式学习机制 | ❌ Out of Scope |
| MCP 协议 | Ch.10 | 完整 MCP 客户端（stdio/SSE/HTTP）+ 工具目录缓存 + 权限控制 | ✅ 已覆盖 |
| 目标设定与监控 (Goal/Monitoring) | Ch.11 | §10.11 目标设定与监控：TaskGoal/GoalCriterion + GoalMonitor | ✅ 已设计 |
| 异常处理与恢复 (Exception/Recovery) | Ch.12 | AppError 统一错误 + MCP 重试（指数退避）+ LSP 崩溃重启 | ✅ 已覆盖 |
| 人机协同 (Human-in-the-Loop) | Ch.13 | §10.10 人机协同：工具审批分级 + ToolApprovalDialog + 升级机制 | ✅ 已设计 |
| 知识检索 (RAG) | Ch.14 | Wiki + RAG 引擎（分块/嵌入/混合检索） | ✅ 已覆盖 |
| 智能体间通信 (A2A) | Ch.15 | **部分覆盖**：Actor 消息传递通信，但缺少跨进程/跨会话的 A2A 协议 | ⚠️ 后续迭代 |
| 资源感知优化 | Ch.16 | **部分覆盖**：Token 预算管理、上下文窗口监控，但缺少动态资源调度 | ⚠️ 后续迭代 |
| 推理技术 (Reasoning) | Ch.17 | **部分覆盖**：LLM 自带推理，但缺少显式 CoT/ToT/GoT 推理框架 | ⚠️ 后续迭代 |
| 安全护栏 (Guardrails) | Ch.18 | §10.12 安全护栏：四层防御 + InjectionDetector + ToxicityFilter | ✅ 已设计 |
| 评估与监控 (Evaluation) | Ch.19 | §10.13 评估与监控：AgentTrace 轨迹 + AgentJudge + 性能仪表盘 | ✅ 已设计 |
| 优先级管理 (Prioritization) | Ch.20 | **未覆盖** | ❌ Out of Scope |
| 探索与发现 (Exploration) | Ch.21 | **未覆盖** | ❌ Out of Scope |

**关键增强点（基于书中模式）**：

1. **反思模式增强**（Ch.4）：在 RigAgent 循环中增加可选的「生成-评审」子循环——Agent 生成输出后，可用另一个 LLM 调用（不同 system prompt）评审输出质量，不满足标准时自动重试。用于代码审查、翻译校对等高精度场景。

2. **目标设定与监控增强**（Ch.11）：为 Workflow/TaskDefinition 增加显式 `goals` 字段（可衡量的成功标准），运行时持续检查目标达成状态，偏离时触发重新规划或升级。

3. **人机协同增强**（Ch.13）：实现工具审批 UI（ToolApprovalDialog）——当 Agent 调用需审批的工具（write/edit/delete）时，暂停执行，前端弹出审批对话框展示工具名称、参数、影响范围，用户批准/拒绝后继续。

4. **安全护栏增强**（Ch.18）：在 PromptBuilder 注入前增加输入内容过滤层（敏感词/注入检测），在 Agent 输出后增加输出过滤层（毒性/偏见检测），使用轻量模型（如 Gemini Flash）作为快速预筛。

5. **评估与监控增强**（Ch.19）：增加 Agent 轨迹记录（每步工具调用、推理过程、耗时），支持 LLM-as-Judge 评估输出质量，提供 Agent 性能仪表盘（成功率、平均延迟、Token 消耗趋势）。

### Cherry Studio 设计参考

本设计参考 Cherry Studio（Electron 桌面 AI 助手）的设计系统，提取其设计哲学与组件架构模式。

**设计哲学对比**：

| 维度 | Cherry Studio | Prism Agent R |
|------|--------------|---------------|
| 风格 | Neutral-First Utilitarian-Modern（中性优先，实用主义） | Apple Design（毛玻璃 + 半透明 + 圆角） |
| 色彩空间 | oklch（感知均匀） | hex/rgba |
| 颜色策略 | 界面本身退让，内容是最彩色的东西 | 系统色 + 语义色，毛玻璃增强层次 |
| 暗色模式 | 真反转（`#0A0A0A` 背景） | 纯黑背景（`#000000`） |
| 字体 | Inter（单一字体覆盖全部 UI） | SF Pro / PingFang SC（系统字体链） |
| 阴影 | 静止时扁平，仅交互时浮出 | 毛玻璃 + 轻阴影 |
| 动画 | Framer Motion spring（damping:30, stiffness:350） | CSS cubic-bezier 弹性 |
| 组件库 | shadcn/ui (New York style) + Radix UI | 自建设计系统（Svelte 5） |

**可借鉴的 Cherry Studio 模式**：

1. **两层 Token 架构**：原始 token（`--cs-*`）→ 主题别名（`--color-*`），通过 `pnpm theme:build` 生成。Prism Agent R 可采用类似模式：原始设计令牌 → CSS 变量 → Svelte 组件消费。
2. **oklch 色彩空间**：感知均匀，暗色模式只需调整 lightness 而非重新选色。Prism Agent R 当前用 hex，可考虑迁移。
3. **状态色板完整定义**：error/success/warning/info 各有 base/text/bg/border/hover/active 6 变体。Prism Agent R 的语义色应补全这些变体。
4. **圆角重映射**：Cherry 将 Tailwind 默认圆角从 6px→8px、8px→10px、12px→14px，使视觉更柔和。Prism Agent R 可参考此策略。
5. **主题定制化**：通过 CSS 变量覆盖实现社区主题（cherrycss.com），Prism Agent R 可支持用户自定义主题。
6. **组件分层**：50+ 原子组件（primitives）+ 25+ 复合组件（composites），原子组件无业务逻辑，复合组件组合原子组件。

### Compose-Next 工作流参考

本设计参考 MiMo-Code 的 compose-next 工作流模式，将 8 阶段编排管线映射到 Prism Agent R 的开发与运行时。

**Compose-Next 8 阶段管线**：

| 阶段 | 职责 | Prism Agent R 对应 |
|------|------|-------------------|
| Orient（定位） | 检查仓库/指令/最近变更，决定工作形状 | Agent 启动时扫描工作目录指令文件（CLAUDE.md/AGENTS.md） |
| Grill（决策） | 用 `question` 工具逐轴解析用户决策 | 工作流参数填充对话框 + Agent 配置确认 |
| Spec（规格） | 维护 `docs/compose/spec/<feature>.md`，带 `[Sn]` 锚点和 tasks | 本设计文档本身就是 spec 产物 |
| Workspace（工作区） | 创建 linked worktree，不在 main 上实现 | Tauri 项目结构：`src-tauri/`（Rust）+ `src/`（Svelte） |
| Implement（实现） | 按依赖序执行 tasks，并行任务分发给 subagent | WorkflowEngine 按 `depends_on` 拓扑排序执行阶段 |
| Verify（验证） | 运行测试/typecheck/build，记录结果 | `model:test` + MCP 连通性检查 + 前端构建验证 |
| Review（评审） | 分派 fresh subagent 审查完整变更 | 工作流 `critic` 阶段（头脑风暴/代码审查） |
| Finalize（终结） | 更新 spec 文档（status/delivered/报告） | 工作流 `done` 状态 + 结果持久化 |

**MiMo-Code 编排模式映射**（compose-next 的工作流模式如何指导 Prism Agent R 的多 Agent 任务编排）：

| 编排模式 | MiMo-Code 实现 | Prism Agent R 应用 |
|----------|---------------|-------------------|
| 8 阶段管线 | Orient→Grill→Spec→Workspace→Implement→Verify→Review→Finalize | WorkflowEngine 的阶段模板系统（§10.6），每个阶段 = 一个 StageTemplate |
| 任务依赖拓扑排序 | 按 `depends_on` 顺序执行 | `topological_sort(&wf.stages)` 已实现 |
| 并行任务分发 | 独立 task 分发给 subagent，各带 worktree | tokio 并发 + Semaphore 限流（§3.4） |
| Verify/Review 分离 | 先跑验证命令，再分派 fresh reviewer | 工作流 `verify` 阶段 + `review` 阶段（critic 角色） |
| 决策解析（Grill） | `question` 工具逐轴解析用户选择 | 工作流 `inputs` 参数填充对话框（§9.9.1，见 phase2-panel.md） |
| 规格文档持久化 | `docs/compose/spec/<feature>.md`，status: designed→in-progress→delivered | WorkflowRun 状态机 + 结果持久化到 workflow_runs 表 |
| 终结不自动完成 | 报告分支/SHA，由用户决定 merge/PR/push | 工作流 `done` 状态 + 前端结果展示，用户手动触发下一步 |

**MiMo-Code 权限模型映射**：

```
三层权限合并（MiMo-Code runtimePermission）：
  1. agent.permission        → Agent 基础权限
  2. user/session config     → 用户/会话配置覆盖
  3. agent.hardPermission    → 不可放松的安全不变量（最后胜出）

Prism Agent R 对应：
  1. Agent.disabled_tools    → Agent 级工具禁用列表
  2. RiskLevel 分级          → 工具审批分级（§10.10）
  3. 安全护栏                → 输入/输出过滤（§10.12，不可绕过）
```

**MiMo-Code Task 追踪映射**：

| MiMo-Code 概念 | Prism Agent R 实现 |
|----------------|-------------------|
| 层级 ID（T1, T1.1, T1.2） | TaskDefinition.stages[].id（如 "stage1", "stage2"） |
| 状态生命周期（open → in_progress → blocked → done/abandoned） | WorkflowRun.stage_status（pending/running/done/failed/cancelled） |
| `task` 工具（create/start/block/done/abandon） | `workflow:run`/`task:run` + `workflow:stage` 事件 |
| 任务归档（默认 7 天） | workflow_runs 表保留策略（可配置） |

---

## [S1] Problem

原 Prism Agent 基于 Electron + Node.js + React 构建，存在以下问题：

- **包体过大**：Electron 运行时 + Chromium ~150MB+，安装包臃肿
- **内存占用高**：Node.js + Chromium 常驻内存 ~300MB+，低配置机器卡顿
- **性能瓶颈**：Node.js 事件循环模型难以高效处理大规模并发 Agent 任务
- **类型安全缺失**：IPC 通信缺乏端到端类型检查，运行时错误难以排查
- **Agent 能力有限**：缺乏多 Agent 协作、工作流编排能力
- **生态依赖重**：Vercel AI SDK 等 JS 生态依赖多，锁版本困难

目标：用 **Rust 重写全部后端**（Tauri 2.x 壳），**Svelte 5 重写前端**，构建一个高性能、轻量级（包体 <20MB、内存 <120MB）、类型安全的 **跨平台（Windows / macOS / Linux）** AI Agent 平台，保留原项目的全部核心功能（Agent 系统、技能系统、MCP、LLM Wiki、RAG、会议纪要、翻译、OCR），并新增 **主页面板（多 Agent 总控制台 + 任务设计区）** 与 **Agent 运行时侧边栏**。

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
    ├─ 收到 text delta → emit('chat:stream:delta', delta) → 前端渲染
    ├─ 收到 tool_call → ToolExecutor 执行（内置/MCP）
    │   └─ 结果回填 → Rig 继续生成
    └─ 收到 finish → emit('chat:stream:done') → 前端收尾
  → 消息持久化 SQLite → 记忆系统更新
```

### 1.1 多窗口 / 独立对话窗口（后续迭代，[S5] ⭐ 中）

**定位**：从会话拖出独立窗口，支持同时查看多个 Agent 的对话。

**实现方案**（Tauri 2.x multiwindow）：

```
主窗口 (main)                   独立窗口 (child: session_id)
┌──────────────────────┐        ┌──────────────────────┐
│ SideNav | 对话 | ... │ ──拖出──▶ │ 仅对话区（MessageList │
│ [↗ 独立窗口] 按钮     │        │  + Composer）          │
└──────────────────────┘        └──────────────────────┘
```

- **多窗口共享后端状态**：所有窗口连同一 Rust 进程（Tauri 单进程多 Webview），SQLite 连接池/会话状态天然共享
- **路由**：子窗口加载 `window.<label>` + `?session_id=xxx`，SvelteKit 用 label 路由到只读对话组件
- **事件广播**：Tauri `emitTo(label)` 定向；`chat:stream:*` 只发给绑定该 session 的窗口
- **关闭语义**：子窗口关闭仅销毁视图，会话/流不受影响（后端持有）；主窗口关闭 = 退出应用（或托盘驻留）
- **限制**：MVP 后做；同一 session 允许多窗口只读，写操作（Composer）仅主窗口

**数据流**：子窗口 `chat:send` → 同一 ChatService（共享 active_streams）→ 事件 `emitTo(child_label)` 定向渲染。

**可能错误 + 处理方法**：

| 错误 | 检测 | 处理 | 反馈 |
|------|------|------|------|
| 子窗口创建失败（资源不足） | Webview 创建异常 | 降级为弹窗内嵌只读视图 | 「无法打开独立窗口，已改为内嵌」 |
| 事件广播错窗（label 未注册） | emitTo 抛错 | 回退 emitAll + 前端按 session_id 过滤 | 无感（自动） |
| 多窗口并发写同一 session | 无（设计约束） | Composer 仅主窗口可用，子窗口只读 | 子窗口输入框禁用 |

### 1.2 托盘驻留 + 全局快捷键（后续迭代，[S5] 🔸 低）

**定位**：关闭窗口时驻留系统托盘，全局快捷键快速唤起。

**实现方案**：

```
Tauri tray-icon + menu:
  ├─ 显示/隐藏主窗口
  ├─ 新建会话（唤起 + 跳转）
  └─ 退出
全局快捷键（global-shortcut 插件）:
  └─ Ctrl+Shift+Space → 唤起/隐藏
```

- **托盘图标**：`tauri-plugin-tray`（或 2.x 内置 tray），菜单项走事件 → Rust handler
- **窗口隐藏**：`window.hide()`（非 destroy），进程常驻；macOS 菜单栏图标 + `activationPolicy=Accessory`
- **全局快捷键**：`tauri-plugin-global-shortcut`，注册/注销按平台（Windows/macOS/Linux 键位差异见 §14.5）
- **唤起行为**：窗口显示 + 聚焦 + 若有关联会话则恢复最后激活 Tab
- **生命周期**：托盘退出 → 正常关闭（先 flush 未保存状态，§14.6#39 autosave 教训）

**可能错误 + 处理方法**：

| 错误 | 检测 | 处理 | 反馈 |
|------|------|------|------|
| 全局快捷键冲突（被其他应用占用） | 注册失败 | 提示 + 允许改键 | 「快捷键被占用，请更换」 |
| 托盘图标加载失败 | 图标资源异常 | 降级为窗口关闭即退出 | 无感（日志） |
| 托盘点击无响应（Linux DE 差异） | 事件未触发 | 菜单 + 左键双击双通道 | 无感（兼容层） |
| 退出时 flush 失败 | 保存异常 | 保留未保存数据到恢复文件 | 「部分内容未保存，已暂存」 |

### 新会话创建流程（参考 prism-agent 原版）

**双模式创建**（对齐 prism-agent 的经典/Draft 设计）：

| 模式 | 触发 | 行为 | 适用场景 |
|------|------|------|----------|
| **直接创建** | 点击 Agent 旁的 `+` / `Ctrl+N` | 立即在 DB 创建空 Session（title=''），跳转到会话 | 经典布局、明确要新建 |
| **Draft 模式** | 侧边栏点击 Agent 名称 | 不立即创建 Session，显示空对话界面，发送首条消息时才创建 | 快速进入、不确定是否需要持久化 |

**直接创建流程**：
```
用户触发 → session:create { agent_id, title: '' }
  → DB 插入（order_key=0，置顶）
  → setActiveSession(newSession)
  → 跳转对话界面
  → 用户发送首条消息 → chat:send → AI 自动重命名
```

**Draft 模式流程**：
```
用户触发 → 显示空对话界面（无 session_id）
  → 用户输入首条消息 → session:create + chat:send 同时执行
  → 流式响应 → AI 自动重命名
```

**自动重命名**（AI 生成标题）：
- 触发条件：会话内消息数 ≥ 2（至少一来一回）
- 实现：取最后 5 条消息 → LLM 生成简短标题（≤30 字符）
- 前端通过 `session:rename` 更新 title
- 用户可手动编辑标题（`isNameManuallyEdited` 前端标记：手动编辑过则后续不自动重命名）

**复用空 Session 机制**：
- 同一 Agent 下如果已有未使用过的空 Session（无消息），直接复用而非重复创建
- 判断条件：无消息（`COUNT(messages) == 0`，通过 SQL 聚合计算）+ `created_at == updated_at`

**IPC 命令扩展**：
| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `session:create` | `{agent_id, title?}` | `SessionDto` | 创建会话（默认 title=''） |
| `session:rename` | `{id, title}` | `SessionDto` | 重命名（含 AI 自动重命名） |
| `chat:send` | `{session_id, content}` | `MessageDto` | 发送消息 |

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
| 加密 | aes-gcm | - | API Key 加密存储（AES-256-GCM） |
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

## [S3] Out of Scope

- **会议 ASR 引擎**：支持多后端（云端 DashScope/MiMo/Whisper/Azure + 本地 sherpa-onnx/Vosk/FunASR-WS/Custom），见 §10.3；**本地模型（sherpa/vosk）默认不打包，按需下载**；Azure 为可选后端（需 Key）
- **移动端**：Tauri 支持移动但本次仅桌面（Win/macOS/Linux）
- **云端同步**：WebDAV/S3 备份（后续迭代）
- **国际化**：i18n 框架（后续迭代，UI 先中文）
- **Electron 兼容层**：不保留原 Node.js 代码，完全重写
- **插件系统**：不对齐 VS Code 扩展体系；通过技能 + MCP 满足扩展需求
- **语音合成（TTS）**：本次不做
- **本地大模型推理**：Ollama 仅作为远程 provider 接入，不在应用内嵌推理引擎

## [S4] 各模块错误处理矩阵

> **用途**：每个功能模块的常见错误 → 检测方式 → 处理策略 → 用户反馈。实现时按本矩阵落地错误处理，避免遗漏。详细错误类型定义见 phase1-core.md §11；本矩阵为跨模块的运维视角汇总。

### 1. Agent 核心 / LLM 调用（phase1 §3/§7）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| Provider 超时 | 请求超时（可配置，默认 30s） | 重试 1 次（指数退避）→ 仍失败则中止 | 提示「请求超时，已重试，请检查网络」 |
| API Key 无效（401） | 响应状态码 | 不重试，标记 provider 不可用 | 引导到设置页更新 Key |
| 配额不足（429） | 响应状态码 + 重试头 | 退避重试（30s）→ 仍失败则暂停该 provider | 「配额已用尽，请稍后或切换模型」 |
| 模型不存在（400/404） | 响应错误码 | 标记模型失效，`model:list` 刷新 | 「模型不可用，请重新选择」 |
| 上下文溢出 | provider 返回 overflow 错误 | §13.1 上下文压缩（Head/Tail 选择） | 自动压缩，无需用户干预 |
| 流中断（网络断开） | 流式连接断开 | `chat:stream:error` + 保留已收内容 | 可点击「继续生成」恢复 |
| 工具调用循环卡死 | 超过 max_iterations | 强制终止 + 记录轨迹 | 「达到最大迭代次数，已停止」 |

### 2. 数据库 / 存储（phase1 §5）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| 数据库损坏（SQLITE_CORRUPT） | 查询异常 | 备份 `prism.db` → 重建空库 → 恢复日志 | 「数据库损坏，已重建并备份旧文件」 |
| 磁盘空间不足 | 写入失败（ENOSPC） | 触发数据保留策略（§5.7.6）清理旧数据 | 「磁盘空间不足，已自动清理」 |
| 迁移失败 | sqlx migrate 异常 | 单迁移 try/catch 隔离，失败不阻断启动（§14.3#30） | 日志告警，继续启动 |
| 写锁冲突（busy） | busy_timeout 超时 | 5s 重试 → 仍失败则报错 | 「数据库繁忙，请重试」 |
| 消息表膨胀 | 行数监控 | 游标分页 + 数据保留策略（§5.7.8） | 无感（自动） |

### 3. MCP 协议（phase1 §6）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| MCP 进程崩溃 | 子进程退出监听 | 状态置 Error + 日志缓冲 | 侧边栏红色状态 + 「重试/查看日志」 |
| 工具调用超时 | timeout_ms（默认 30s） | 取消 + 记录 | 「工具调用超时」 |
| 工具不存在 | tools/list 目录比对 | 报错 + 提示可用工具 | 「工具 X 不存在，可用：...」 |
| 远程 MCP 认证失败 | OAuth/header 错误 | 状态置 Error | 引导重新授权 |
| 工具参数错误 | 参数 schema 校验 | 拒绝 + 返回错误给 LLM | Agent 收到错误后可自纠 |

### 4. 会议 / ASR（phase3 §10.3）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| 麦克风无权限 | getUserMedia 拒绝 | 引导系统权限设置 | 「请在系统设置中允许麦克风」 |
| ASR 后端不可用 | health_check 前置 | 启动前提示 + 禁止开始 | 「所选 ASR 后端不可用，请更换」 |
| 网络断流（云端 ASR） | WS 断开 | 自动重连（指数退避 3 次） | 「连接中断，正在重连…」 |
| 本地模型缺失 | 模型文件检查 | 提示下载（AsrModelManager） | 「本地模型未安装，点击下载」 |
| 转写超长 | 500KB 上限 | 截断最旧段 + 提示 | 「转写已达上限，已截断」 |

### 5. 翻译 / OCR（phase3 §10.5）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| 翻译 API 失败 | 网络/HTTP 错误 | 重试 1 次 → 失败则返回缓存或原文 | 「翻译失败，已返回原文」 |
| 语言不支持 | 语言码校验 | 明确报错 | 「不支持从 X 翻译到 Y」 |
| OCR 识别失败 | 图片解析失败 | 降级到下一 provider（MiMo→Tesseract） | 「识别失败，已尝试备用引擎」 |
| 术语表冲突 | 同源同目标重复 | 去重 + 提示 | 「术语已存在」 |

### 6. Wiki / RAG（phase3 §10.1-10.2）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| 文档解析失败 | file:parse 异常 | 标记 error + 跳过该文件 | 「文档 X 解析失败，已跳过」 |
| 嵌入失败 | 嵌入 API 错误 | 批量重试 → 部分失败标记 | 「部分文档嵌入失败」 |
| 写入越界（路径穿越） | canonicalize 校验 | 拒绝该 op + 回滚（§10.1.1） | 「非法路径，操作已回滚」 |
| write_ai 计划解析失败 | serde 解析异常 | 重试 1 次 → 返回可读错误 | 「AI 写入失败，请重试」 |

### 7. 记忆系统（phase1 §10.7）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| 记忆写入失败 | 文件/DB 异常 | 重试 → 失败则跳过该次记录 | 日志告警（不打断对话） |
| FTS 索引损坏 | 搜索异常 | 自动 rebuild + reconcile | 「记忆索引已重建」 |
| checkpoint 校验失败 | CheckpointViolation | quarantine + 通知 writer 重试 | 无感（后台重试） |
| 写入权限越界 | 写入沙箱校验 | 拒绝 + 日志 | 「该 agent 无权写入此记忆」 |

### 8. 前端 / IPC（phase1 §8/§9）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| IPC 命令不存在 | invoke 抛错 | 前端捕获 + toast | 「命令不可用」 |
| WebView 崩溃/重载 | 窗口事件 | 重新初始化 IPC + 恢复会话 | 「界面已恢复」 |
| 事件订阅泄漏 | 组件卸载未清理 | listen 返回 unlisten，卸载时调用（§14.6#35） | 无感（预防性） |
| 渲染大数据卡顿 | 性能监控 | 虚拟滚动 + content-visibility（§13） | 无感（自动） |

### 9. 工作流 / 多 Agent（phase1 §10.6 + phase2 §9.9.1）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| 阶段执行失败 | 阶段返回 Err | 可配置：跳过 / 重试 / 暂停等用户决策（§10.10.2，见 phase2-panel.md） | 时间线标红 + 选项 |
| 模板变量引用缺失 | render_template 校验 | 构建期报错（带缺失变量名） | 「模板引用了不存在的变量 X」 |
| 阶段图有环 | 拓扑排序失败 | 拒绝运行（task:validate） | 「检测到循环依赖」 |
| Agent 角色不匹配 | Coordinator 查找失败 | 报错 + 建议可用角色 | 「角色 X 无对应 Agent」 |
| 任务运行超时 | 阶段超时（默认 300s） | 中止 + 标记超时 | 「阶段 X 超时，已停止」 |

### 10. 升级 / 工具审批（phase2 §10.10）

| 错误场景 | 检测方式 | 处理策略 | 用户反馈 |
|----------|---------|---------|---------|
| 审批请求无响应 | 等待超时 | 默认拒绝 + 通知 Agent | 「审批超时，已拒绝」 |
| 连续工具失败 | 3 次失败计数 | 暂停 + 升级给用户 | 「连续失败，建议人工介入」 |
| 循环行为检测 | 相同操作 >5 次 | 自动中断 + 诊断报告 | 「检测到循环，已中断」 |

> **跨模块原则**：所有错误统一走 `AppError`（phase1 §11.1）；重试用指数退避；前端 toast 提示与日志并存；可恢复错误（网络/超时）自动重试，不可恢复错误（权限/校验）明确反馈。

## [S5] 功能建议（后续迭代候选）

> **用途**：当前设计未包含但值得考虑的功能，按价值排序。非承诺，供规划参考。

| 优先级 | 功能 | 说明 | 详细设计 |
|--------|------|------|---------|
| ⭐ 高 | **云端同步**（WebDAV/S3） | 配置/Agent/工作流/记忆跨设备同步，加密传输；已在 [S3] 列为后续迭代 | phase1 §5.8 |
| ⭐ 高 | **消息/session 导出与导入** | 会话导出 Markdown/JSON，可导入恢复；利于备份与分享 | phase1 §10.8.1 |
| ⭐ 高 | **用量预警** | 接近 token/费用阈值时主动通知（复用 §5.7.6 保留策略的信号） | phase2 §9.10.1 |
| ⭐ 中 | **快捷指令/命令面板扩展** | ⌘K 面板支持自定义命令序列（复用 §10.6 工作流引擎） | phase1 §9.8.1 |
| ⭐ 中 | **多窗口/独立对话窗口** | 从会话拖出独立窗口，支持同时查看多个 Agent | 本文件 §1.1 |
| ⭐ 中 | **主题商店** | 用户主题上传/下载（CSS 变量覆盖，参考 Cherry Studio） | phase1 §9.1.1 |
| ⭐ 中 | **提示词模板库** | 常用提示词片段管理，插入 Composer | phase1 §9.8.2 |
| ⭐ 中 | **会话归档/冻结** | 不删除但冻结的会话，减少列表噪音（复用 pinned） | phase1 §9.5.1 |
| 🔸 低 | **托盘驻留 + 全局快捷键** | 后台常驻，快速唤起 | 本文件 §1.2 |
| 🔸 低 | **Agent 市场** | 分享/下载 Agent 配置模板（复用技能市场机制） | phase1 §10.4.5 |
| 🔸 低 | **自更新** | Tauri updater 自动更新 | phase1 §14.5.1 |
| 🔸 低 | **工作流版本控制** | 模板历史版本对比/回滚 | phase1 §10.6.4.1 |
| 🔸 低 | **TTS 播报** | 会议纪要/通知语音播报（[S3] 暂缓） | phase3 §10.3.9 |
| 🔸 低 | **项目级 RAG 自动索引** | 工作目录变更自动增量索引（复用 fs:watch） | phase3 §10.2.1 |

> **决策原则**：新功能优先复用现有引擎（工作流/技能/记忆/MCP），不新增孤岛；敏感操作（同步、导出、Agent 市场）需人工确认（§10.10 审批模式）。

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
| T19 | 人机协同（HITL）— 工具审批流程 + 升级机制 |

### Phase 3 — 扩展功能

| 任务 | 内容 |
|------|------|
| T12 | Wiki + RAG（write_ai / 分块 / 混合检索） |
| T13 | 翻译 + OCR |
| T14 | 会议系统（8 后端 ASR） |
| T17 | 安全与设置（API Key 加密可提前在 T6 引入基础版） |
| T20 | 反思模式（Reflection）— 生产者-评审者循环 |
| T21 | 安全护栏（Guardrails）— 输入/输出过滤 |
| T22 | 目标设定与监控 — SMART 目标 + 进度评估 |
| T23 | 评估与监控 — Agent 轨迹 + LLM-as-Judge + 性能仪表盘 |
| T24 | 上下文压缩 — 压力等级 + 工具裁剪 + Head/Tail 选择 + 溢出恢复 + TokenBudget 统一配置 |
| T18 | 测试与验证（贯穿各阶段，Phase 3 汇总） |

---

## 各功能模块 MVP 内容清单

> **用途**：按**功能系统**逐模块列出「各阶段需要落地的最小内容」。每个功能系统给出定位、三阶段内容与验收标准，实施时以本清单为验收基准，一个模块一个模块核对。
> **图例**：🟦 Phase 1（Agent 核心闭环）· 🟧 Phase 2（面板）· 🟩 Phase 3（扩展）· ⬜ 后续迭代
> **实施顺序**：Agent 核心 → Skill/MCP/工作流/记忆 挂接 → 对话前端闭环（Phase 1）；面板/侧边栏（Phase 2）；扩展功能（Phase 3）。每阶段内按「后端核心 → IPC → 前端」推进。

### 1. Agent 核心系统（§3.2~3.3 + §8 agent/session/chat + §9.6）

**定位**：单 Agent 的创建、配置、执行闭环——用户创建 Agent → 选择模型 → 对话 → 流式生成 → 工具调用。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | ADK 三个 Trait（ModelProvider/ToolExecutor/MemoryStore）+ PromptBuilder；RigAgent agentic loop + StreamPipeline + OpenAI/Ollama 双 Provider；agent/session/chat/model 域 IPC；对话前端（三栏布局 + Composer + 流式渲染 + 会话管理） | 用户能创建 Agent、发起对话、看到流式输出、切换会话 |
| 🟧 | agent:stats 命令 + Agent 编辑页完善（模型/提示词/工具/技能配置） | Agent 配置全量可编辑 |
| 🟩 | 反思模式接入（ReflectionConfig）+ 评估轨迹（AgentTrace）+ 目标监控（TaskGoal） | 高精度场景可启用反思循环 |

### 2. Skill 技能系统（§10.4）

**定位**：技能的安装、管理、注入——让 Agent 通过 SKILL.md 获得专项能力。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | skill:list/install/uninstall/toggle + SKILL.md 解析（frontmatter）+ PromptBuilder 注入 + skill:list-local | 安装一个技能并在对话中生效 |
| 🟧 | 市场三源搜索（skills.sh/claude-plugins/clawhub）+ 去重排序 + 缓存 + 重名冲突处理 | 市场搜索可用，结果可排序筛选 |
| 🟩 | 版本更新检测 + 依赖预检（git/zip/网络）+ 安装后 health-check | 技能更新与依赖检查完整 |

### 3. MCP 协议系统（§6）

**定位**：MCP 服务器连接与工具调用——Agent 通过 MCP 获得外部工具能力。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | McpTransport（stdio + http 两传输）+ McpRuntime + McpCatalog + 工具目录缓存 + mcp:list/add/remove/test/tools/call-tool | 挂一个本地 MCP 服务器，工具可调用 |
| 🟧 | SSE 传输 + OAuth 回调（远程 MCP）+ `tools/list_changed` 监听 + mcp:status-changed 事件 + 侧边栏 MCP Tab | 远程 MCP 可用，状态实时 |
| 🟩 | ServerLogBuffer 查看 + 工具参数大小限制 + Agent 绑定/禁用工具粒度控制 | 权限控制完整 |

### 4. 记忆系统（§10.7）

**定位**：分层记忆（global/projects/sessions）+ 会话持久化——跨会话保持上下文。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | 分层目录（global/projects/sessions）+ MemoryStoreImpl + 会话构建时注入 + memory:search/read/write 基础命令 | 跨会话记忆生效 |
| 🟧 | FTS5 索引（006_memory）+ BM25 搜索 + reconcile + checkpoint-writer + 写入沙箱 + 主动召回注入 | 记忆可搜索，checkpoint 自动策展 |
| 🟩 | 校验重试机制（CheckpointViolation）+ 溢出文件 + 前端记忆管理面板 | 记忆管理 UI 完整 |

### 5. 多 Agent 工作流（§3.4 + §10.6 + §9.9.1）

**定位**：多 Agent 编排与任务执行——预置工作流 + 用户自定义任务设计。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | Actor Trait + Coordinator + TaskScheduler + WorkflowEngine + render_template + 1 预置工作流（深度研究）+ workflow:run/list/stop/result | 跑通"深度研究"工作流 |
| 🟧 | 其余预置工作流（代码审查/头脑风暴/翻译校对）+ 阶段模板系统（stage_templates）+ 任务设计区（TaskDesigner 画布）+ task:save-template/run/validate/rerun | 画布可编排自定义任务 |
| 🟩 | 目标监控（GoalMonitor）+ 并行阶段执行 + 阶段反思接入 | 任务运行可视化 + 目标可评估 |

### 6. 对话/聊天系统（§7 + §9.5~9.8）

**定位**：对话界面与流式体验——核心交互层。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | 三栏布局（SideNav/ContentArea/RightPanel）+ MessageList/Composer/ModelSelector + chat:stream:* 事件全量 + 取消（abort）+ 会话切换/搜索/固定 + **消息全文搜索（009_message_search，§5.7.2）** | 完整对话体验，流式可中断，历史可搜索 |
| 🟧 | 会话自动重命名（AI 生成标题）+ 编辑重发 + ToolCallCard 过程展示 + 快捷指令（⌘K）+ 会话标题搜索（012_session_fts，§5.7.4） | 对话增强功能可用 |
| 🟩 | 上下文压缩（§13.1：压力等级/工具裁剪/Head-Tail/溢出恢复）+ 翻译历史搜索（013_translate_fts，§5.7.5） | 长会话流畅 |

### 7. 主页面板（§9.9）

**定位**：应用首页 — Agent 总控制台 + 任务设计区入口。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | —（Phase 1 不依赖面板） | — |
| 🟧 | HomePage + AgentLauncher 卡片网格 + UsageStats/Chart + Skill/Mcp Overview + dashboard:overview + 最近会话 + 任务历史 | 首页聚合数据可用，点击 Agent 直接对话 |
| 🟩 | 用量趋势图表 + 单价表费用估算 + 任务模板卡片 + 运行中任务实时状态 | 面板完整 |

### 8. Agent 侧边栏（§9.10）

**定位**：对话页右侧运行时上下文面板（六 Tab）。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | —（Phase 1 不依赖侧边栏） | — |
| 🟧 | AgentSidebar 六 Tab（用量/工作目录/指令/MCP/LSP/文件）+ context:agent 聚合命令 + workspace/lsp 域命令 + fs:watch | 侧边栏展示完整运行时上下文 |
| 🟩 | LSP 诊断实时推送 + 文件树懒加载 + 指令文件注入（session:inject-file） | 侧边栏深度可用 |

### 9. Wiki/RAG 知识库（§10.1~10.2）

**定位**：LLM Wiki + RAG 检索——知识持久化与语义搜索。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | —（Phase 1 不涉及） | — |
| 🟧 | —（Phase 2 不涉及） | — |
| 🟩 | WikiService + write_ai 计划执行（结构化操作/校验回滚）+ 分块（段落/句子/窗口）+ 嵌入（API/本地）+ 混合检索（向量+BM25）+ 摄取后台任务 + 前端知识库页 | 导入文档可检索，AI 可写库 |

### 10. 翻译/OCR（§10.5）

**定位**：文本翻译 + 图片识别。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | —（Phase 1 不涉及） | — |
| 🟧 | —（Phase 2 不涉及） | — |
| 🟩 | TranslateService（多 Provider/批量/文件/缓存）+ Glossary 术语表 + OcrService 多后端 + 前端翻译页 | 翻译/OCR 全流程可用 |

### 11. 会议系统（§10.3）

**定位**：录音 → ASR 转写 → 清洗/摘要/问答/导出。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | —（Phase 1 不涉及） | — |
| 🟧 | —（Phase 2 不涉及） | — |
| 🟩 | AsrBackend 可插拔架构（8 后端）+ 录音流（AudioStreamManager 时序规避）+ 增量落库 + 离线二次转写 + 清洗/摘要/问答/推送 Agent + 导出 + 前端 | 完整会议闭环 |

### 12. 设计系统（§9.1~9.4）

**定位**：设计令牌 + 组件库 + 动画——视觉基础。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | 设计令牌（两层架构：oklch 原始 + CSS 语义别名）+ 基础组件 15+（Button/Input/Switch/Dialog）+ glass 工具类 | 主题切换可用，组件样式统一 |
| 🟧 | primitives/composites 分层补齐（ScrollArea/Separator/Kbd/ContextMenu 等）+ PageHeader/ConfirmDialog 等复合组件 | 组件库覆盖业务需要 |
| 🟩 | 无障碍适配（对比度/触摸目标/reduced-motion 三媒体查询）+ 八项设计原则落地 + 圆角/阴影体系校准 | 符合 iOS 18 无障碍标准 |

### 13. 数据库层（§5）

**定位**：SQLite 持久化 + 迁移体系 + 性能。

| 阶段 | MVP 内容 | 验收标准 |
|------|---------|---------|
| 🟦 | 001_init + 004_workflow 迁移 + sqlx 连接池 + PRAGMA 优化 + 核心 CRUD | 迁移全跑通，CRUD 可用 |
| 🟧 | 002_rag / 003_meeting / 005_glossary / 006_memory / 007_workflow_templates | 功能迁移就绪 |
| 🟩 | 008_agent_traces / 009_message_search / 010_indexes / 011_asr + 数据保留策略 + 游标分页 + 查询监控 | 大数据量流畅 |

### 14. 横切能力（§11~§14）

**定位**：错误/日志、安全、性能、旧版规避——贯穿各阶段。

| 能力 | 🟦 Phase 1 | 🟧 Phase 2 | 🟩 Phase 3 |
|------|-----------|-----------|-----------|
| 错误/日志（§11） | AppError 统一类型 + tracing 初始化 + 关键操作日志 | 命令层耗时埋点 | 慢查询监控 |
| 安全（§12） | API Key 加密（AES-GCM）+ capabilities 权限 | 人机协同审批（T19） | 护栏（InjectionDetector/ToxicityFilter）+ SSRF 防护 |
| 性能（§13） | 冷启动 <1s + 内存 <120MB 基线 | 用量统计聚合 | 上下文压缩（§13.1） |
| 旧版规避（§14） | §14.1~14.4 关键规避落地（模型 ID/upsert 幂等/时序丢块/目录穿越） | §14.5 平台差异逐项核对 | §14.6~14.7 全部 51 条对照回归 |

---

## Tasks

**Phase 1 — MVP（Agent 核心闭环）**

- [ ] T1: 项目初始化 — Tauri 2.x + Svelte 5 + SvelteKit 脚手架、Cargo 工作区、Vite 配置、**CI 三平台构建矩阵（Windows/macOS/Linux）** (covers: 索引 §1/§2)
- [ ] T2: 设计系统（MVP 子集） — 设计令牌（colors/typography/spacing/motion）、glass 工具类、基础组件库 15+ (covers: phase1 §9.1-9.4; depends: T1)
- [ ] T3: 数据库层 — sqlx 连接池 + 11 个迁移 + 全部模型 (covers: phase1 §5; depends: T1)
- [ ] T4: ADK 组件层 — ModelProvider/ToolExecutor/MemoryStore Trait + PromptBuilder + AgentError (covers: phase1 §3.2; depends: T3)
- [ ] T5: Rig 核心层 — RigAgent agentic loop + 流式管道 + 内置工具 + Provider 适配器（MVP 先 OpenAI/Ollama，其余 Phase 3 补） (covers: phase1 §3.3/§7; depends: T4)
- [ ] T6: 服务层（MVP 子集） — Agent/Session/Chat/Model 服务；dashboard/usage/workspace/lsp 命令随 Phase 2 落地 (covers: phase1 §8; depends: T3, T5)
- [ ] T8: MCP 层（MVP 子集） — McpTransport stdio/http 两传输 + McpRuntime + 工具目录缓存 + commands (covers: phase1 §6; depends: T5)
- [ ] T9: 技能系统（MVP 子集） — 安装/卸载/启停 + PromptBuilder 注入；市场三源搜索随 Phase 2 (covers: phase1 §10.4; depends: T4, T8)
- [ ] T15: AutoAgents 编排（MVP 子集） — Actor/Coordinator/WorkflowEngine + "深度研究"预置工作流 + render_template；其余模板随 Phase 2 (covers: phase1 §3.4/§10.6; depends: T5)
- [ ] T16: 记忆系统（MVP 子集） — 分层记忆（global/projects/sessions）+ 会话注入；FTS 搜索/checkpoint-writer 随 Phase 2 (covers: phase1 §10.7; depends: T3, T5)
- [ ] T11: 对话前端（MVP 核心，不依赖侧边栏） — 三栏布局 AppShell + MessageList/Composer/流式渲染 + 会话管理 (covers: phase1 §9.5-9.8; depends: T2, T6)

**Phase 2 — 面板功能**

- [ ] T10: **Agent 侧边栏** — AgentSidebar 六 Tab 详设（用量进度条/工作目录切换/指令文件注入/LSP 检测与诊断/文件树懒加载） + context:agent 聚合命令 + session:inject-file/lsp:detect/fs:watch 命令 (covers: phase2 §9.10; depends: T2, T6, T7, T15)
- [ ] T7: **主页面板** — HomePage + AgentLauncher + UsageStats/Chart + Skill/Mcp Overview + 多 Agent 任务设计区（TaskDesigner 画布/运行器/历史） + task 命令 (covers: phase2 §9.9; depends: T2, T6, T9, T15)
- [ ] T11 增强: 对话前端嵌入 Agent 侧边栏（T10 完成后合并） (covers: phase2 §9.10; depends: T10)
- [ ] T9 补充: 市场三源搜索（协议细节/去重排序/缓存） (covers: phase1 §10.4.1~10.4.4; depends: T9)
- [ ] T6 补充: dashboard/usage/workspace/lsp 命令 + 单价表与用量聚合 (covers: phase2 §9.9 数据源; depends: T6, T10)
- [ ] T19: **人机协同（HITL）** — 工具审批流程（ToolApprovalRequest/Response + ToolApprovalDialog） + RiskLevel 分级 + 升级机制 + 会话级始终批准 (covers: phase2 §10.10; depends: T5, T6, T10, T11)

**Phase 3 — 扩展功能**

- [ ] T12: Wiki + RAG — WikiService + write_ai 计划执行（结构化操作/校验回滚/工具接入）+ 分块/嵌入/混合检索 + 摄取后台任务 + 前端知识库页 (covers: phase3 §10.1-10.2; depends: T3, T5)
- [ ] T13: 翻译 + OCR — TranslateService（多 Provider/批量/文件翻译/术语表/缓存）+ OcrService 多后端 + 前端翻译页 (covers: phase3 §10.5; depends: T5)
- [ ] T14: 会议系统 — AsrBackend 可插拔架构（8 后端协议级实现）+ 本地 sherpa-onnx 集成 + 模型下载管理 + 录音流通道 + 离线二次转写 + 清洗/摘要/问答/推送 Agent/导出 + 前端 (covers: phase3 §10.3; depends: T5, T6)
- [ ] T17: 安全与设置 — Key 加密存储 + capabilities 权限 + 设置页 (covers: phase1 §12; depends: T6)
- [ ] T20: **反思模式（Reflection）** — ReflectionConfig + run_reflection_loop + 评审者 Agent 配置 + StageTemplate 反思字段 + 前端反思循环展示 (covers: phase3 §10.9; depends: T5, T15)
- [ ] T21: **安全护栏（Guardrails）** — GuardrailPipeline + InjectionDetector + ToxicityFilter + 输入/输出过滤器接口 + 前端护栏配置 (covers: phase3 §10.12; depends: T5, T6)
- [ ] T22: **目标设定与监控** — TaskGoal/GoalCriterion 数据结构 + GoalMonitor 运行时评估 + 前端目标进度条 (covers: phase3 §10.11; depends: T15, T7)
- [ ] T23: **评估与监控** — AgentTrace 轨迹记录 + agent_traces 表 + AgentJudge LLM-as-Judge + 性能仪表盘（agent:stats）+ 前端评估 Tab (covers: phase3 §10.13; depends: T6, T10)
- [ ] T24: **上下文压缩** — CompactionAgent + ContextWindow + 压力等级 + 工具输出裁剪（soft trim/hard prune）+ Head/Tail 选择 + 溢出检测与恢复 + 微压缩 + TokenBudget 统一配置 (covers: phase3 §13.1; depends: T5, T16)
- [ ] T18: 测试与验证 — 单元测试（分块/检索/错误映射/任务校验）、集成测试（对话流/任务流）、性能基准、**三平台打包验证（Windows NSIS / macOS dmg / Linux deb+rpm+AppImage）**；**§14 规避回归**：模型 ID 格式/upsert 幂等/音频时序丢块/目录穿越/配置合并/事件清理 (covers: phase1 §11/§13/§14, phase3 §13.1; depends: T6, T8, T12, T15)

---

## MVP Phase 1 完成报告

### 已完成任务

| 任务 | 状态 | 内容 |
|------|------|------|
| T1 | ✅ 完成 | Tauri 2.x + Svelte 5 + SvelteKit 脚手架、Cargo 工作区、Vite 6、CI 三平台构建矩阵 |
| T2 | ✅ 完成 | 设计系统 MVP — 19 个基础组件 + iOS 18 设计令牌 + 毛玻璃 CSS |
| T3 | ✅ 完成 | 数据库层 — sqlx + 11 个迁移（init/rag/meeting/workflow/glossary/memory/workflow_templates/agent_traces/message_search/indexes/asr）+ 数据模型 |
| T4 | ✅ 完成 | ADK 组件层 — ModelProvider/ToolExecutor/MemoryStore 三个 Trait + PromptBuilder + ToolRegistry |
| T5 | ✅ 完成 | Rig 核心层 — RigAgent agentic loop + StreamPipeline + OpenAI/Ollama Provider 适配器 |
| T6 | ✅ 完成 | 服务层 — Agent/Session/Chat/Model/Settings 服务 + 16 个 IPC 命令 |
| T8 | ✅ 完成 | MCP 层 — McpTransport (stdio/http) + McpRuntime + McpCatalog + 7 个 IPC 命令 |
| T9 | ✅ 完成 | 技能系统 — SkillService (安装/卸载/启停) + SKILL.md 解析 + PromptBuilder 集成 + 6 个命令 |
| T11 | ✅ 完成 | 对话前端 — Apple Design 风格三栏布局 + 流式渲染 + 会话管理 + 设置向导 |
| T15 | ✅ 完成 | AutoAgents 编排 — Actor/Coordinator/WorkflowEngine + 4 个预置工作流 + render_template |
| T16 | ✅ 完成 | 记忆系统 — MemoryService (全局/项目/会话分层) + MemoryStore 实现 + 4 个命令 |

### 已实现功能

1. **项目初始化**：Tauri 2.x + Svelte 5 + SvelteKit + Vite 6 + CI 三平台构建
2. **设计系统**：iOS 18 风格设计令牌、毛玻璃效果、19 个基础组件
3. **数据库层**：sqlx SQLite + 11 个迁移文件、完整数据模型
4. **ADK 组件层**：ModelProvider/ToolExecutor/MemoryStore 三个 Trait
5. **Rig 核心层**：RigAgent agentic loop + OpenAI/Ollama Provider 适配器
6. **服务层**：Agent/Session/Chat/Model/Settings CRUD + IPC 命令
7. **对话前端**：Apple Design 风格 UI、设置向导、对话界面、流式渲染
8. **图标导入**：从原版 prism-agent 项目导入 logo 和图标

### 技术要点

- **Tauri 2.x 参数命名**：Rust `snake_case` 自动转为前端 `camelCase`
- **Tauri 2.x API**：使用 `@tauri-apps/api/core` 而非 `window.__TAURI__`
- **Svelte 5 事件处理**：不支持事件修饰符，需用函数包装
- **Svelte 5 嵌套限制**：button 不能嵌套 button，改用 div + role="button"
- **iOS 18 设计规范**：系统颜色、SF Pro 字体、12px/16px 圆角、毛玻璃导航栏

### 后续工作（Phase 2 & 3）

- T7: 主页面板 + 任务设计区
- T10: Agent 侧边栏
- T12-T14: Wiki/RAG、翻译/OCR、会议系统
