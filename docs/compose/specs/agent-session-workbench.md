---
feature: agent-session-workbench
status: delivered
updated: 2026-08-28
branch: master
commits: pending
---

# Agent Session Workbench

## Report

已按批准设计完成实施。首页替换为 Agent Workspace Grid；Agent 页改为可折叠 Agent → 会话树；新建和选择会话统一执行生命周期初始化；可见卡片懒加载最近 trace。未引入 3D 可视化或新的后端聚合接口。
验证：`npm run check` 通过（0 errors，1 个既有 a11y warning）；`npm test` 通过（3 files / 12 tests）；`npm run build` 通过（既有 a11y 与 chunk-size warnings）。

## [S1] Problem

当前首页是一个纵向仪表盘：Kanban、用量、趋势、Skills、MCP 和最近会话分散在不同卡片里。用户无法在一个主视图内快速看到“每个 Agent 正在做什么、进展到哪里、最近模型调用是否正常”。

同时，Agent 页把 Agent 列表和会话列表拆成两个纵向区块，会话数量多时必须滚动到底部；多 Agent 会话无法保持稳定的空间分组。

## [S2] Design

### 设计方向

1. **首页：Agent Workspace Grid**
   - 以 Agent 为单位展示卡片，每个 Agent 固定占据一张卡，不按会话拆成多张卡。
   - 默认九宫格视觉；Agent 数量增加时扩展为 3×4、3×5 等网格，而不是纵向滚动列表。
   - 替换现有首页 Dashboard：Kanban、用量统计、趋势图、Skills、MCP 总览和最近会话卡片都不再作为首页面板展示。
   - 点击卡片进入对应 Agent 会话页面。

2. **Agent 页：单侧栏可折叠树**
   - 保留已确认的 Agent → 会话树导航。
   - Agent 是一级节点，会话是嵌套二级节点。
   - 从工作区卡片进入后，当前 Agent 分组展开并高亮当前会话。

### 首页布局

```text
┌ Agent Workspace ──────────────────────────────────────────────┐
│ Title: Workspace        [All Ready] [Running 2] [New Agent]  │
├──────────────────────────────────────────────────────────────┤
│ ┌ Agent A ─────────┐ ┌ Agent B ─────────┐ ┌ Agent C ───────┐ │
│ │ Running           │ │ Done              │ │ Idle            │ │
│ │ 任务 2/5          │ │ 最近会话 B1       │ │ 暂无进行中任务   │ │
│ │ Call: Running     │ │ Call: Success     │ │ Call: None      │ │
│ └──────────────────┘ └──────────────────┘ └────────────────┘ │
│ ┌ Agent D ─────────┐ ┌ Add Agent ───────┐ ┌ Empty Cell ────┐ │
│ │ Failed            │ │                   │ │                 │ │
│ └──────────────────┘ └──────────────────┘ └────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### 网格规则

- Agent 数量小于等于 9 时使用 3 列布局，并保留至少 3 行的九宫格骨架；空白格不显示边框，只在仍有可创建空间时提供 `Add Agent` 占位。
- Agent 数量 10–12 时使用 3×4 布局。
- Agent 数量 13–15 时使用 3×5 布局。
- Agent 数量超过 15 时保持 5 列，行数按 `ceil(count / 5)` 增长。
- 卡片按 Agent 的 `order_key` 稳定排序；不因为 Running / Failed 状态自动跳位，避免用户失去空间记忆。
- 窄屏响应式降级：宽度不足时先降为 2 列，再降为 1 列；状态信息保持完整。

### Agent 卡片内容

每张卡片使用固定三段结构，保证不同状态下行高可预测：

1. **身份区**
   - Agent 头像、名称、当前模型名。
   - 生命周期状态徽章：`Idle`、`Ready`、`Running`、`Paused`、`Done`、`Failed`、`Uninitialized`。

2. **进度区**
   - 优先显示 Agent 当前任务：`owner === agent.id` 且 `status === 'doing'` 的第一个任务标题。
   - 显示任务进度：`done / total`；没有任务时显示 `No tasks`。
   - 如果最近 trace 正在执行，显示当前步骤摘要：步骤类型或工具名；没有步骤数据时显示 `Working...`。
   - 不虚构百分比；只有任务数量可以形成明确的 `done / total`。

3. **调用区**
   - 查询该 Agent 最近会话的最近一条 trace。
   - 显示最近调用结果、耗时、prompt/completion token 和成本。
   - 有未完成 trace 时显示 `Calling...`；失败时显示 `Failed` 和短错误摘要。
   - 没有会话或没有 trace 时显示 `No recent calls`，不标记为失败。

### 数据来源与刷新

不新增后端协议，全部使用现有数据：

1. `agentStore.loadAgents()` 提供完整 Agent 列表；工作区必须展示所有 Agent，而不是只展示有 system prompt 的 Agent。
2. `dashboard_kanban` 提供 Agent 最近会话、消息数和 `SessionLifecycle`。前端用 `agent_id` 与 Agent 列表合并；Kanban 缺失的 Agent 生成 `Idle / Uninitialized` 卡。
3. `dashboard_tasks` 按 `owner` 分组到 Agent 卡片，用于 `done / total` 进度和当前任务标题。
4. `traceApi.list(sessionId, 1)` 查询最近会话的最近模型调用；只为可见卡片查询并缓存结果。
5. `session:state-changed` 刷新 Kanban 生命周期。
6. `chat:stream:done`、`chat:stream:error`、`usage:updated` 刷新对应 Agent 的调用摘要。
7. Provider / Model 未配置时，在页头显示紧凑警告和 `Settings` 入口；不恢复独立快速配置 Banner。

### 点击行为

- 点击已有最近会话的卡片：
  1. 根据 `agent_id` 找到 `AgentDto` 并选中。
  2. 根据 `session_id` 从全量会话索引中找到 `SessionDto` 并选中。
  3. 跳转 `/agent`。
  4. Agent 页加载历史并执行现有 `sessionLifecycleApi.init(sessionId)`。
- 点击没有会话的卡片：选中 Agent，创建新会话，跳转 `/agent` 并统一完成初始化。
- 点击 `Add Agent` 占位：进入 Agent 页并打开现有新建 Agent 表单。
- 卡片必须使用明确的 clickable 样式、hover 状态和键盘可达性。

### Agent 页会话树

- `AgentStore` 增加 `sessionsByAgent: Record<string, SessionDto[]>`，通过现有 `sessionApi.list()` 加载全量会话并按 `agent_id` 分组。
- 页面内维护 `expandedAgents: Record<string, boolean>`：
  - 默认展开 `currentAgent`；没有 `currentAgent` 时展开第一个 Agent。
  - 点击 Agent 名称：选中该 Agent 并展开分组。
  - 点击 chevron：只切换展开/折叠，不改变当前 Agent。
  - 点击 `+`：选中 Agent、展开分组并创建新会话。
- 会话排序：`pinned === true` 优先，其余按 `updated_at` 降序。
- 新建和选择会话都走同一流程：创建/选中、插入分组索引、加载历史、初始化生命周期、更新当前状态。

## [S3] Scope

### In scope

- 将首页 Dashboard 替换为 Agent Workspace Grid。
- 实现以 Agent 为单位的九宫格 / 3×N 状态卡片。
- 展示生命周期、任务进度、当前步骤摘要和最近模型调用状态。
- 卡片点击进入正确 Agent 会话页。
- `AgentStore` 增加全量会话索引和分组数据。
- Agent 页侧栏改为可折叠 Agent / 会话树。
- 统一新建会话与选择会话的生命周期初始化。

### Out of scope

- 新增后端 API、DTO、SQL 或数据库迁移。
- 3D 工作区、办公室、角色和空闲动画。
- Agent 编辑、设置页或模型选择逻辑重构。
- 会话重命名、删除、批量移动、归档。
- 跨设备同步展开状态。
- 完整 Trace 回放、评分和日志查看。
- 保留原首页面板作为二级页面。

## Tasks

- [x] T1: 为 `AgentStore` 增加 `loadAllSessions()` 与 `sessionsByAgent`，新建/删除会话时同步分组索引。
- [x] T2: 新增 Agent Workspace Grid 页面，按 Agent 数量应用 3×3、3×4、3×5 和响应式网格。
- [x] T3: 合并 Agent、Kanban、Task 数据，生成生命周期与任务进度摘要。
- [x] T4: 按最近会话懒加载最近 trace，展示模型调用状态、耗时、token 和失败摘要。
- [x] T5: 实现卡片点击进入既有会话或创建新会话，并处理 `Add Agent` 占位。
- [x] T6: 将首页替换为工作区网格，移除旧 Dashboard 卡片的首页渲染。
- [x] T7: Agent 页移除独立会话区块，实现可折叠 Agent / 会话树。
- [x] T8: 统一新建和选择会话的生命周期初始化。
- [x] T9: 补齐无 Agent、无会话、无任务、无 trace、未配置 Provider/Model 和窄屏状态。
- [x] T10: 运行 Svelte check、单元测试和相关构建验证。

## Acceptance Criteria

1. 首页只展示 Agent Workspace Grid 和必要页头，不再渲染原 Dashboard 的 Kanban、用量、趋势、Skills、MCP、最近会话面板。
2. 每个已创建 Agent 都有一张工作区卡片，包括没有最近会话或 system prompt 的 Agent。
3. Agent 数量不超过 9 时呈现九宫格；10–12 张卡使用 3×4；13–15 张卡使用 3×5；更多 Agent 时按 5 列追加行。
4. 卡片显示生命周期状态；有任务时显示 `done / total` 进度和当前 doing 任务；没有任务时显示空态。
5. 有最近模型调用时显示成功/失败、耗时和 token；没有调用时显示 `No recent calls` 且不误报失败。
6. 点击有会话的卡片后，Agent 页打开正确的 Agent 和最近会话，并完成生命周期初始化。
7. 点击无会话的卡片后，Agent 页创建并打开新会话，同样完成生命周期初始化。
8. Agent 页任一 Agent 展开后，会话固定显示在该 Agent 下方；当前会话正确高亮。
9. 点击 Agent 名称不会误创建会话；点击 chevron 只折叠；点击 `+` 才创建新会话。
10. 会话状态、消息完成或调用失败后，对应工作区卡片能够刷新。
11. 窄屏下网格降级为 2 列或 1 列，卡片文本不溢出，Agent 页会话树仍可滚动。
12. `npm run check`、现有单元测试和可行构建验证无新增失败。

## Alternatives Considered

### 保留 Dashboard 并增加工作区入口

会继续保留两套首页心智模型，用户仍要先理解 Kanban 和统计卡片。与“替换面板”的要求不一致。未采用。

### 按会话生成网格卡片

会话数量会快速撑大网格，Agent 数量多时反而找不到目标。工作区应以 Agent 为稳定单位，会话留在 Agent 页分组中。未采用。

### Marvis 式可视化工作区

办公室、房间、角色和动画能表达执行氛围，但当前需求是可读的进度和调用状态，不适合增加渲染和资产成本。未采用。

### 后端新增工作区聚合接口

现有 Agent 列表、Kanban、任务和 trace 数据已经覆盖本次展示。新增聚合接口会扩大变更范围。未采用。
