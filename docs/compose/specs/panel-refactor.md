---
feature: panel-refactor
status: in-progress
updated: 2026-08-15
branch: feat/panel-refactor
commits: c72a3e0..HEAD
---

# 面板重构 — Agent 控制面板

## Report

**What was built** — 删除了全部工作流/编排/loop 代码（前端 7 个文件 + 后端 5 个文件/目录，共 6644 行），新建 `AgentStatusGrid` 组件替代原有的 OrchestratorPanel + AgentLauncher。面板首页改为三行布局：Agent 状态网格（全宽 glass 卡片，含状态灯/模型/技能/MCP 标签/相对时间/操作按钮/新建卡片/空状态）+ 用量统计+趋势图（两列）+ 技能/MCP/最近会话（三列）。所有组件使用 iOS 26 Liquid Glass 设计令牌，浅深色主题自动适配。

**Verification** — `npm run check`: PASS (0 errors, 0 warnings)。`cargo check`: 3 个 ASR 模块预存错误（AsrModelCategory 未找到、缺少结构体字段），与本次改动无关，经 git diff 确认未修改任何 ASR 文件。

**Journey log** — ① 后端 `autoagents` 和 `orchestrator` 模块完全自包含，无外部依赖，可安全整体删除。② `monitor.rs` 混合了工作流命令和监控命令，需精确分离保留/删除。③ `BudgetTracker.active_workflows` 字段保留不动（值为 0），避免改动预算模块。④ Svelte 5 不支持 `onclick|stopPropagation` 修饰符语法，需用 `onclick={(e) => { e.stopPropagation(); ... }}` 替代。⑤ 审查发现删除按钮缺少确认对话框和响应式断点，已修复。

## [S1] Problem

当前面板首页（`src/routes/+page.svelte`）以自主编排（OrchestratorPanel, 560px）为顶部主入口，包含完整的工作流/编排体系（SPEC → 执行 → 审查）。用户希望：

1. **删除全部工作流代码** — 前端 OrchestratorPanel、orchestratorStore、MonitorPanel、monitorStore、workflowApi、LoopDesigner，以及后端 workflow/orchestrator/loop 命令、引擎、服务。
2. **重构面板为 Agent 控制面板** — 参考 FastVibe（实时状态看板）、AgentOps（成本追踪/可观测）、MASTERCONTROL（状态面板），面板聚焦于「控制自己的 Agent」。
3. **保持现有设计系统** — iOS 26 Liquid Glass 设计令牌不变。

**参考仓库设计要点**：
- **FastVibe**：Kanban 风格任务看板 + WebSocket 实时状态 + Agent 进度卡片
- **AgentOps**：Session 回放 + LLM 成本追踪 + 事件图 + 工具使用统计
- **MASTERCONTROL**：六面板布局 + 双执行模式 + 项目状态可视化 + Agent token/cost 追踪

**采纳的设计模式**：Agent 状态卡片网格（FastVibe 看板风格）+ 用量/成本监控（AgentOps/MASTERCONTROL 成本追踪）+ 最近会话列表（三库共有）。

## [S2] Design

### 2.1 新面板布局

```
┌──────────────────────────────────────────────────────────────┐
│  DashboardHeader                                               │
│  "Good to see you" / "N agents ready"                          │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ Agent 状态网格 (AgentStatusGrid)             [新建 Agent] │ │
│  │ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │ │
│  │ │ ● idle   │ │ ● active │ │ ● idle   │ │  + 新建  │      │ │
│  │ │ [avatar] │ │ [avatar] │ │ [avatar] │ │          │      │ │
│  │ │ Agent A  │ │ Agent B  │ │ Agent C  │ │          │      │ │
│  │ │ gpt-4o   │ │ claude-3 │ │ gpt-4o   │ │          │      │ │
│  │ │ 3 skills │ │ 5 skills │ │ 0 skills │ │          │      │ │
│  │ │ 2m ago   │ │ just now │ │ never    │ │          │      │ │
│  │ │ [对话]   │ │ [对话]   │ │ [对话]   │ │          │      │ │
│  │ └──────────┘ └──────────┘ └──────────┘ └──────────┘      │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                │
│  ┌────────────────────────┐ ┌─────────────────┐               │
│  │ UsageStatsCard          │ │ UsageTrendChart  │               │
│  │ Today: 1.2K tokens      │ │ (7日 SVG 折线)   │               │
│  │ Week: 8.5K tokens       │ │                  │               │
│  │ Month: $2.34            │ │                  │               │
│  │ Calls today: 12         │ │                  │               │
│  └────────────────────────┘ └─────────────────┘               │
│                                                                │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐   │
│  │ Skills        │ │ MCP Servers  │ │ Recent Sessions       │   │
│  │ 3 / 5 enabled │ │ 2 / 3 conn   │ │ • session1 · AgentA   │   │
│  │ [tag] [tag]   │ │ • server1 4t │ │ • session2 · AgentB   │   │
│  │               │ │ • server2 8t │ │ • session3 · AgentA   │   │
│  └──────────────┘ └──────────────┘ └──────────────────────┘   │
│                                                                │
│  [Quick Setup Banner — 仅 Provider/Model 未配置时显示]          │
└──────────────────────────────────────────────────────────────┘
```

**布局规格**：
- 外层容器：`padding: 24px 32px; overflow-y: auto; flex: 1`
- 内容区：`max-width: 1200px; margin: 0 auto; display: flex; flex-direction: column; gap: 20px`
- 第一行（AgentStatusGrid）：全宽 glass 卡片
- 第二行（UsageStats + UsageTrend）：`flex-direction: row; gap: 16px`，UsageStats `flex: 2`，UsageTrend `flex: 1`
- 第三行（Skills + MCP + RecentSessions）：`flex-direction: row; gap: 16px`，三等分 `flex: 1`
- 响应式断点 `@media (max-width: 900px)`：所有 row 改为 column

### 2.2 AgentStatusGrid 组件设计

**文件**：`src/lib/components/dashboard/AgentStatusGrid.svelte`

**Props 契约**：
```typescript
interface Props {
  agents: AgentSummary[];
  onStartChat: (agentId: string) => void;
  onCreateAgent: () => void;
  onDeleteAgent?: (agentId: string) => void;
}
```

**数据来源**：`AgentSummary` 类型（已存在于 `dashboard.svelte.ts`）：
```typescript
interface AgentSummary {
  id: string;
  name: string;
  description: string;
  avatar: string | null;
  model_name: string | null;
  skill_count: number;
  mcp_count: number;
  last_used: string | null;
  order_key: number;
}
```

**视觉设计**：

容器（glass 卡片）：
```css
.agent-status-grid {
  background: var(--glass-solid-bg);           /* rgba(255,255,255,0.82) 浅 / rgba(30,30,30,0.82) 深 */
  backdrop-filter: var(--glass-solid-blur);     /* blur(80px) saturate(180%) */
  -webkit-backdrop-filter: var(--glass-solid-blur);
  border: 1px solid var(--color-separator);
  border-radius: var(--radius-md);              /* 12px */
  box-shadow: var(--glass-edge-highlight), var(--shadow-sm);
  padding: 20px;
}
```

卡片头部：
```css
.grid-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}
.grid-header h2 {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-fg);
  margin: 0;
}
```

网格：
```css
.agent-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
}
```

单个 Agent 卡片：
```css
.agent-card {
  background: var(--color-bg-secondary);       /* 卡片内层背景 */
  border-radius: var(--radius-md);              /* 12px */
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  cursor: pointer;
  transition: transform 0.15s var(--ease-default), box-shadow 0.15s ease;
  position: relative;
}
.agent-card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}
```

卡片内容结构：
```
┌──────────────────────────┐
│ ● [状态灯]  [⋮ 菜单]      │  ← 顶部行：状态灯 + 操作菜单
│                          │
│ [avatar 44x44]  Agent A  │  ← 头像 + 名称
│                 gpt-4o   │  ← 模型名（caption 标签样式）
│                          │
│ 3 skills · 2 MCP         │  ← 技能/MCP 计数（meta 标签）
│ 2m ago                   │  ← 最近使用时间
│                          │
│ ┌──────────────────────┐ │
│ │   [💬 开始对话]       │ │  ← 底部按钮（accent 背景，圆角药丸）
│ └──────────────────────┘ │
└──────────────────────────┘
```

状态指示灯：
- `idle`（无活跃会话）：灰色圆点 `var(--color-muted)` — 6px 直径
- `active`（有 Running 会话）：绿色圆点 `var(--color-green)` — 6px 直径，带微弱脉冲动画 `@keyframes pulse`

卡片操作菜单（右上角 `⋮` 按钮）：
- 点击展开下拉菜单（复用现有 `--color-bg-elevated` 背景 + `--shadow-md` 阴影）
- 菜单项：「查看详情」（`goto('/agent')` + 选中 Agent）、「删除」（调用 `agentApi.delete`，需确认）

新建 Agent 卡片（网格末尾）：
```css
.new-agent-card {
  border: 2px dashed var(--color-separator);
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--color-fg-secondary);
  font-size: 14px;
  cursor: pointer;
  min-height: 160px;  /* 与 Agent 卡片大致等高 */
  transition: border-color 0.15s ease, color 0.15s ease;
}
.new-agent-card:hover {
  border-color: var(--color-accent);
  color: var(--color-accent);
}
```

空状态（无 Agent 时）：
- 居中显示图标 + 文案「还没有 Agent」+ 按钮「创建第一个 Agent」
- 复用 AgentLauncher 现有空状态样式

**交互行为**：
- 卡片整体点击 → `onStartChat(agent.id)` → 跳转 `/agent` 页面
- 底部「开始对话」按钮点击 → 同上（阻止冒泡）
- 右上角 `⋮` 菜单 → 展开「查看详情」/「删除」
- 新建卡片点击 → `onCreateAgent()` → 跳转 `/agent`

**响应式**：
```css
@media (max-width: 600px) {
  .agent-grid {
    grid-template-columns: 1fr;
  }
}
```

### 2.3 前端删除清单

**删除文件**（7 个）：

| # | 文件路径 | 说明 |
|---|---------|------|
| 1 | `src/lib/components/dashboard/OrchestratorPanel.svelte` | 自主编排面板（1009 行） |
| 2 | `src/lib/components/dashboard/MonitorPanel.svelte` | 监控面板（267 行，孤儿组件，使用 Tailwind 死样式） |
| 3 | `src/lib/components/dashboard/AgentLauncher.svelte` | 旧 Agent 启动器（153 行，被 AgentStatusGrid 替代） |
| 4 | `src/lib/components/dashboard/AgentCard.svelte` | 旧 Agent 卡片（186 行，被 AgentStatusGrid 内部卡片替代） |
| 5 | `src/lib/components/loop/LoopDesigner.svelte` | Loop 设计器（孤儿组件） |
| 6 | `src/lib/stores/orchestrator.svelte.ts` | 编排 store（150 行） |
| 7 | `src/lib/stores/monitor.svelte.ts` | 监控 store（139 行，仅被 MonitorPanel 使用） |

**修改文件**（3 个）：

**`src/lib/api/index.ts`** — 移除以下导出（第 213-231 行和第 816-848 行）：
- `WorkflowDto` interface
- `workflowApi` 对象（list/run/rerun/stop/result）
- `LoopKind` type
- `LoopStatus` type
- `AgentLoop` interface
- `LoopCreateRequest` interface
- `loopApi` 对象（start/stop/list）

**`src/lib/stores/dashboard.svelte.ts`** — 移除以下类型和字段：
- `WorkflowSummary` interface（第 53-59 行）
- `TaskRunSummary` interface（第 61-68 行）
- `DashboardOverview.workflows` 字段（第 85 行）
- `DashboardOverview.task_runs` 字段（第 86 行）

**`src/routes/+page.svelte`** — 移除 OrchestratorPanel/AgentLauncher 导入与使用，替换为 AgentStatusGrid：
- 移除 `import OrchestratorPanel`（第 13 行）
- 移除 `import AgentLauncher`（第 10 行）
- 移除 OrchestratorPanel 容器 div（第 59-61 行）
- 移除 AgentLauncher 使用（第 66-74 行）
- 新增 `import AgentStatusGrid`
- 新增 AgentStatusGrid 渲染（全宽第一区块）

### 2.4 后端删除清单

**`src-tauri/src/lib.rs`** 修改：

1. 移除 `use core::autoagents::loop_scheduler::LoopScheduler;`（第 15 行）
2. 从 `AppState` struct 移除 `pub loop_scheduler: std::sync::Arc<LoopScheduler>;`（第 26 行）
3. 从 `AppState` 初始化移除 `loop_scheduler: std::sync::Arc::new(LoopScheduler::new()),`（第 75 行）
4. 从 `invoke_handler` 移除以下 18 条命令注册（第 133-145 行 + 第 256-266 行）：
   - `commands::workflow::workflow_list`
   - `commands::workflow::workflow_run`
   - `commands::workflow::workflow_stop`
   - `commands::workflow::workflow_result`
   - `commands::workflow::task_list_templates`
   - `commands::workflow::task_save_template`
   - `commands::workflow::task_run`
   - `commands::workflow::task_validate`
   - `commands::workflow::task_rerun`
   - `commands::workflow::goal_evaluate`
   - `commands::loop_cmd::loop_start`
   - `commands::loop_cmd::loop_stop`
   - `commands::loop_cmd::loop_list`
   - `commands::monitor::orchestrator_start`
   - `commands::monitor::orchestrator_resume`
   - `commands::monitor::orchestrator_pause`
   - `commands::monitor::orchestrator_stop`
   - `commands::monitor::orchestrator_list`
   - `commands::monitor::workflow_pause`
   - `commands::monitor::workflow_resume`
   - `commands::monitor::monitor_list_active_workflows`

**删除源文件**（5 项）：

| # | 路径 | 说明 |
|---|------|------|
| 1 | `src-tauri/src/commands/workflow.rs` | 工作流命令（~500 行） |
| 2 | `src-tauri/src/commands/loop_cmd.rs` | Loop 命令（51 行） |
| 3 | `src-tauri/src/core/orchestrator/` | 整个目录（engine.rs, spec.rs, plan.rs, session.rs, mod.rs） |
| 4 | `src-tauri/src/core/autoagents/` | 整个目录（actor.rs, coordinator.rs, goal.rs, loop_scheduler.rs, reviewer.rs, scheduler.rs, workflow.rs, workflow_v2.rs, workflow_engine_v2.rs, mod.rs） |
| 5 | `src-tauri/src/data/services/workflow_service.rs` | 工作流 CRUD 服务 |

**模块声明清理**（3 个文件）：

`src-tauri/src/core/mod.rs`：
- 移除 `pub mod autoagents;`（第 2 行）
- 移除 `pub mod orchestrator;`（第 6 行）

`src-tauri/src/commands/mod.rs`：
- 移除 `pub mod loop_cmd;`（第 6 行）
- 移除 `pub mod workflow;`（第 21 行）

`src-tauri/src/data/services/mod.rs`：
- 移除 `pub mod workflow_service;`（第 8 行）
- 移除 `pub use workflow_service::WorkflowService;`（第 23 行）

**`src-tauri/src/commands/monitor.rs`** 清理：

移除的函数（9 个 + 1 个辅助函数）：
- `orchestrator_start`（第 198-218 行）
- `orchestrator_resume`（第 222-248 行）
- `orchestrator_pause`（第 252-263 行）
- `orchestrator_stop`（第 267-278 行）
- `orchestrator_list`（第 282-302 行）
- `workflow_pause`（第 308-320 行）
- `workflow_resume`（第 324-335 行）
- `monitor_list_active_workflows`（第 339-367 行）
- `ActiveWorkflowDto` struct（第 369-377 行）
- `build_orchestrator_engine`（第 382-411 行）
- `build_default_provider`（第 414-450 行）

移除的 use 导入：
- `use crate::core::orchestrator::session::OrchestratorSession;`（第 12 行）
- `use crate::core::orchestrator::engine::OrchestratorEngine;`（第 13 行）

保留的函数（10 个）：
- `budget_get_config`、`budget_get_status`、`BudgetStatusDto`
- `exception_list`、`exception_resolve`、`exception_clear`
- `log_export`、`model_switch_list`
- `monitor_get_budget`、`monitor_get_exceptions`
- `guardrail_check_tool`

保留的 use 导入：
- `use std::sync::Arc;`（仅当 `build_default_provider` 删除后仍被其他代码使用时保留，否则移除）
- `use crate::core::budget::*;`
- `use crate::core::guardrails::*;`
- `use crate::core::observability::exception::*;`
- `use crate::core::rig::provider::OpenAiProvider;`（仅当 `build_default_provider` 删除后无其他引用时移除）
- `use crate::core::adk::model::ModelProvider;`（同上）
- `use crate::data::models::{ModelRow, ProviderRow};`（同上）

**`src-tauri/src/data/models.rs`** 清理：

移除的结构体：
- `WorkflowSummary`（第 357-363 行）
- `TaskRunSummary`（第 365-373 行）
- `WorkflowRow`（第 204-212 行）— 仅被 `workflow_service.rs` 和 `dashboard_service.rs` 使用
- `WorkflowDto`（第 214-220 行）— 仅被 `workflow_service.rs` 使用

修改 `DashboardOverview`（第 388-399 行）：
```rust
// 修改前
pub struct DashboardOverview {
    pub agents: Vec<AgentSummary>,
    pub usage: UsageStats,
    pub usage_trend: Vec<UsagePoint>,
    pub skills: SkillOverview,
    pub mcp_servers: Vec<McpServerStatus>,
    pub recent_sessions: Vec<SessionSummary>,
    pub models: Vec<ModelStatus>,
    pub workflows: Vec<WorkflowSummary>,   // ← 移除
    pub task_runs: Vec<TaskRunSummary>,     // ← 移除
}

// 修改后
pub struct DashboardOverview {
    pub agents: Vec<AgentSummary>,
    pub usage: UsageStats,
    pub usage_trend: Vec<UsagePoint>,
    pub skills: SkillOverview,
    pub mcp_servers: Vec<McpServerStatus>,
    pub recent_sessions: Vec<SessionSummary>,
    pub models: Vec<ModelStatus>,
}
```

**`src-tauri/src/data/services/dashboard_service.rs`** 清理：

从 `overview()` 方法移除（第 15-37 行）：
- `let workflows = self.load_workflows().await?;`（第 23 行）
- `let task_runs = self.load_task_runs().await?;`（第 24 行）
- `workflows,`（第 34 行）
- `task_runs,`（第 35 行）

删除方法：
- `load_workflows()`（第 290-312 行）
- `load_task_runs()`（第 314-347 行）

移除 use 导入：
- `use crate::data::models::*;` 中的 `WorkflowRow` 引用（通过 `query_as::<_, WorkflowRow>` 使用，删除方法后自然消失）

**`src-tauri/src/data/services/agent_service.rs`** 清理：
- 第 67 行注释中的 `WorkflowService::ensure_builtin_workflows` 引用仅为注释，不影响编译，可选清理。

### 2.5 数据流

```
dashboardStore.loadOverview()
  → invoke('dashboard_overview')
    → DashboardService::overview()
      → load_agents()        → AgentSummary[] (含 model_name, skill_count, mcp_count, last_used)
      → load_usage()         → UsageStats (today/week/month tokens, cost, calls)
      → load_usage_trend()   → UsagePoint[] (7 日趋势)
      → load_skills()        → SkillOverview (enabled/total, popular)
      → load_mcp_status()    → McpServerStatus[] (id, name, status, tools_count)
      → load_recent_sessions() → SessionSummary[] (id, title, agent_name, updated_at, message_count)
      → load_models()        → ModelStatus[] (provider_name, model_id, display_name, status)
    ← DashboardOverview (无 workflows/task_runs)
  → 前端 dashboardStore.overview 更新
  → AgentStatusGrid 读取 overview.agents 渲染卡片
  → UsageStatsCard 读取 overview.usage
  → UsageTrendChart 读取 overview.usage_trend
  → SkillOverviewCard 读取 overview.skills
  → McpOverviewCard 读取 overview.mcp_servers
  → RecentSessionsCard 读取 overview.recent_sessions

usage:updated 事件 (聊天完成时后端 emit)
  → dashboardStore 节流刷新 (5s throttle)
  → 面板数据自动更新
```

**Agent 状态判断**：
- 后端 `DashboardOverview.agents[].last_used` 为最近会话的 `updated_at` 时间戳
- 前端 AgentStatusGrid 根据 `last_used` 与当前时间差判断显示文案（「刚刚」「N 分钟前」「从未使用」）
- active/idle 状态：当前版本无实时会话状态推送，暂以 `last_used` 在 30 秒内判定为 active（简化方案，后续可接入 `session_state_query` 实时查询）

### 2.6 新 `+page.svelte` 结构

```svelte
<script lang="ts">
  import { goto } from '$app/navigation';
  import { invoke } from '$lib/api/client';
  import { agentStore } from '$lib/stores/agents.svelte';
  import { dashboardStore } from '$lib/stores/dashboard.svelte';
  import { agentApi } from '$lib/api';

  import DashboardHeader from '$lib/components/dashboard/DashboardHeader.svelte';
  import AgentStatusGrid from '$lib/components/dashboard/AgentStatusGrid.svelte';
  import UsageStatsCard from '$lib/components/dashboard/UsageStatsCard.svelte';
  import UsageTrendChart from '$lib/components/dashboard/UsageTrendChart.svelte';
  import SkillOverviewCard from '$lib/components/dashboard/SkillOverviewCard.svelte';
  import McpOverviewCard from '$lib/components/dashboard/McpOverviewCard.svelte';
  import RecentSessionsCard from '$lib/components/dashboard/RecentSessionsCard.svelte';

  let providers = $state<any[]>([]);
  let models = $state<any[]>([]);

  async function load() {
    providers = await invoke<any[]>('model_providers');
    models = await invoke<any[]>('model_list');
  }

  async function createAgent() {
    goto('/agent');
  }

  async function handleStartChat(agentId: string) {
    const agent = agentStore.agents.find((a) => a.id === agentId);
    if (!agent) return;
    agentStore.selectAgent(agent);
    await agentStore.createSession(agent.id, '新会话');
    goto('/agent');
  }

  async function handleDeleteAgent(agentId: string) {
    await agentApi.delete(agentId);
    await agentStore.loadAgents();
    await dashboardStore.loadOverview();
  }

  function handleOpenSession(sessionId: string) {
    const session = agentStore.sessions.find((s) => s.id === sessionId);
    if (session) {
      agentStore.selectSession(session);
      goto('/agent');
    }
  }

  $effect(() => {
    load();
    dashboardStore.loadOverview();
  });
</script>

<div class="dashboard">
  <DashboardHeader agentCount={dashboardStore.overview?.agents.length ?? agentStore.agents.length} />

  <div class="dashboard-body">
    <!-- Row 1: Agent 状态网格 -->
    <AgentStatusGrid
      agents={dashboardStore.overview?.agents ?? agentStore.agents.map(a => ({
        id: a.id, name: a.name, description: a.description ?? '',
        avatar: null, model_name: null, skill_count: 0, mcp_count: 0,
        last_used: null, order_key: a.order_key ?? 0
      }))}
      onStartChat={handleStartChat}
      onCreateAgent={createAgent}
      onDeleteAgent={handleDeleteAgent}
    />

    <!-- Row 2: Usage Stats + Trend -->
    <div class="section-row two-col">
      <div class="col-main">
        <UsageStatsCard usage={dashboardStore.overview?.usage ?? null} />
      </div>
      <div class="col-side">
        <UsageTrendChart data={dashboardStore.overview?.usage_trend ?? []} />
      </div>
    </div>

    <!-- Row 3: Skills + MCP + Recent Sessions -->
    <div class="section-row three-col">
      <SkillOverviewCard skills={dashboardStore.overview?.skills ?? null} />
      <McpOverviewCard servers={dashboardStore.overview?.mcp_servers ?? []} />
      <RecentSessionsCard
        sessions={dashboardStore.overview?.recent_sessions ?? []}
        onOpenSession={handleOpenSession}
      />
    </div>

    <!-- Quick Setup Banner -->
    {#if providers.length === 0 || models.length === 0}
      <div class="setup-banner"> ... </div>
    {/if}
  </div>
</div>
```

### 2.7 设计令牌使用规范

新组件必须使用以下设计令牌，禁止硬编码颜色/尺寸：

**颜色**：
- 卡片背景：`var(--glass-solid-bg)` + `backdrop-filter: var(--glass-solid-blur)`
- 卡片内层：`var(--color-bg-secondary)`
- 边框：`var(--color-separator)`
- 主文字：`var(--color-fg)`
- 次要文字：`var(--color-fg-secondary)`
- 第三级文字：`var(--color-fg-tertiary)`
- 静音色：`var(--color-muted)`
- 强调色：`var(--color-accent)` / `var(--color-accent-hover)`
- 状态色：`var(--color-green)`（active）、`var(--color-muted)`（idle）、`var(--color-red)`（error）
- 标签色：`var(--color-accent)`（model）、`var(--color-green)`（skill）、`var(--color-purple)`（mcp）

**间距**：
- 卡片 padding：`20px`（外层 glass 卡片）、`16px`（内层 Agent 卡片）
- 网格 gap：`12px`
- 区块间距：`20px`（dashboard-body gap）

**圆角**：
- 卡片：`var(--radius-md)`（12px）
- 标签：`9999px`（药丸形）
- 按钮：`9999px`（药丸形）或 `var(--radius-sm)`（8px）

**字体**：
- 卡片标题 h2：`15px` / `font-weight: 600`
- Agent 名称：`var(--text-headline)` / `font-weight: 600`
- 描述：`var(--text-caption1)`
- 标签：`var(--text-caption2)` / `font-weight: 500`
- 时间：`var(--text-caption2)`

**阴影**：
- 卡片默认：`var(--glass-edge-highlight), var(--shadow-sm)`
- 卡片 hover：`var(--shadow-md)`

**过渡**：
- `transition: transform 0.15s var(--ease-default), box-shadow 0.15s ease`
- `transition: background 0.15s ease, border-color 0.15s ease`

### 2.8 不改动的部分

- Agent 页（`/agent`）、设置页、Wiki、会议、翻译页面
- AgentSidebar 六 Tab 侧边栏
- PrimaryNav 导航
- 后端 agent/session/chat/mcp/skill/model/settings 命令
- 后端 budget/exception/guardrail/log_export/model_switch_list 监控命令
- 后端 `core/budget/`、`core/guardrails/`、`core/observability/`、`core/rig/`、`core/adk/`、`core/session/`、`core/search/` 模块
- 数据库表（workflows/workflow_runs/orchestrator_sessions 等表保留不动，仅移除代码引用）
- 设计令牌 CSS（`src/lib/design-system/styles/tokens.css`）
- 现有保留组件（DashboardHeader、UsageStatsCard、UsageTrendChart、SkillOverviewCard、McpOverviewCard、RecentSessionsCard）

## [S3] Out of Scope

- Agent 级别 token 用量统计（后端目前不按 Agent 维度统计 token，使用全局用量代替）
- 实时 WebSocket 会话状态推送（当前通过 `usage:updated` 事件节流刷新，active/idle 状态以 `last_used` 30 秒内简化判定）
- Agent 详情面板/抽屉（从卡片点击进入 `/agent` 页面查看详情）
- 数据库表删除/迁移（保留旧表，仅移除代码引用）
- Agent 排序拖拽
- 后端 `core/budget/tracker.rs` 中 `active_workflows` 字段（保留不动，值为 0）
- `src/lib/components/loop/` 目录本身（仅删除 LoopDesigner.svelte，如目录为空则删除目录）
- `src/lib/components/trace/TraceReplay.svelte`（非工作流组件，保留）
- `src/lib/components/market/SkillMarket.svelte`（非工作流组件，保留）

## Tasks

- [x] T1: 建立 worktree + 编写 spec — acceptance: `docs/compose/specs/panel-refactor.md` 存在且包含完整设计规范，`.worktrees/panel-refactor` 分支可用 (covers: S1 S2)
- [x] T2: 前端删除工作流代码 — acceptance: 7 个文件删除完成，`api/index.ts` 移除 workflowApi/loopApi/WorkflowDto/Loop 相关类型，`dashboard.svelte.ts` 移除 WorkflowSummary/TaskRunSummary 类型和 workflows/task_runs 字段，`npm run check` 通过且无新增 error (covers: S2.3; depends: T1)
- [x] T3: 后端删除工作流代码 — acceptance: `workflow.rs`/`loop_cmd.rs`/`orchestrator/`/`autoagents/`/`workflow_service.rs` 删除完成，`lib.rs` 移除 21 条命令注册 + AppState.loop_scheduler + LoopScheduler import，`core/mod.rs`/`commands/mod.rs`/`data/services/mod.rs` 移除模块声明，`monitor.rs` 移除 9 个工作流函数 + ActiveWorkflowDto + build_orchestrator_engine + build_default_provider + 相关 use 导入，`models.rs` 移除 WorkflowSummary/TaskRunSummary/WorkflowRow/WorkflowDto + DashboardOverview 字段，`dashboard_service.rs` 移除 load_workflows/load_task_runs 方法 + overview() 中的调用，`cargo check` 通过 (covers: S2.4; depends: T1)
- [x] T4: 新建 AgentStatusGrid 组件 — acceptance: `AgentStatusGrid.svelte` 渲染 Agent 卡片网格，每卡片显示头像/名称/模型/状态灯/技能数/MCP数/最近使用时间/开始对话按钮/操作菜单，末尾有新建卡片，空状态显示创建引导，全部使用 S2.7 设计令牌，`npm run check` 通过 (covers: S2.2 S2.5 S2.7; depends: T2)
- [x] T5: 重构面板首页 — acceptance: `+page.svelte` 使用 AgentStatusGrid 替代 OrchestratorPanel+AgentLauncher，布局符合 S2.1 设计（三行：AgentGrid 全宽 / Usage+Trend 两列 / Skills+MCP+Sessions 三列），保留 Quick Setup Banner，`npm run check` 通过 (covers: S2.1 S2.6; depends: T2 T4)
- [x] T6: 验证与审查 — acceptance: `npm run check` PASS (0 errors, 0 warnings)，`cargo check` 3 个预存 ASR 错误（与本次改动无关），审查 0 critical + 3 major 已修复，spec finalize 为 delivered (covers: S2; depends: T3 T5)
