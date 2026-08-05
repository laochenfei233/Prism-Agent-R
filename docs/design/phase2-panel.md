# Prism Agent R — Phase 2（面板功能）详细设计

> **归属**：Phase 2（面板功能）· 本文件来自 `prism-agent-r` 设计文档按阶段拆分
> **总索引**：[`prism-agent-r.md`](../compose/specs/prism-agent-r.md) · **Phase 1**：[`phase1-core.md`](./phase1-core.md) · **Phase 3**：[`phase3-extend.md`](./phase3-extend.md)
> **Updated**：2026-08-05
> **内容**：§9.9 主页面板 · §9.10 Agent 侧边栏（六 Tab） · §10.10 人机协同（工具审批）
> **依赖基础（见 `phase1-core.md`）**：设计令牌/组件库（§9.1-9.4）、对话前端（§9.5-9.8）、数据库（§5 含 §5.7 分页/索引）、IPC 命令（§8）、工作流引擎（§10.6）、记忆基础（§10.7）
> **依赖基础（见 `phase3-extend.md`）**：目标监控（§10.11）、反思配置（§10.9）

---

## 9. Svelte 5 前端详细设计（Phase 2 部分）

> 注：§9 章节头与 §9.1-9.8（设计系统/对话前端）见 `phase1-core.md`；本文件为 §9.9-9.10（面板与侧边栏）。

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
- 卡片拖拽到任务画布 → 自动创建该 Agent 的阶段节点（见 9.9.1）
- 排序：最近使用优先；支持拖拽重排（order_key）
- 空态：无 Agent 时显示引导 + 创建按钮 + 预设模板（研究员/写作/翻译等）

**用量统计实现**：从 `messages.usage` 聚合（见 phase1-core.md §5），按 `created_at` 分组；费用估算用 `preferences` 中的单价表（provider/model → 每 1K token 价格）。`usage:updated` 事件在每条消息完成后推送，面板增量刷新。

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
- 预置模板与 §10.6 预置工作流（见 phase1-core.md）一致：深度研究 / 代码审查 / 头脑风暴 / 翻译校对
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

**依赖输出引用**：提示模板支持 `{{stage.id.output}}` 占位符，运行前由 `render_template` 解析（见 phase1-core.md §10.6）。检查器提供"插入变量"按钮 + 语法高亮 + 悬停预览实际值。

**③ 运行模式（TaskRunnerPanel + TaskRunTimeline）**：

- [▶ 启动] → `workflow:run` 或新命令 `task:run`（自定义设计任务）→ 返回 `run_id`
- 运行面板：
  - 顶部：进度条（完成阶段/总阶段）+ 当前阶段名 + 耗时
  - 时间线（垂直）：每个阶段一节点行——状态着色（⬜待运行/🟦运行中/🟩完成/🟥失败/⬛已取消）+ 输出摘要（折叠）
  - 阶段详情展开（TaskStageResult）：最终文本 + 该阶段全部工具调用卡片（名称/参数/耗时/结果）
  - 控制：⏸ 暂停（当前阶段完成后停）、■ 停止（取消 token）、▶ 继续
- [保存结果] → 阶段输出写入会话（生成对话页消息），用户可继续对话式追问
- 运行中任务在 `workflow_runs` 中轮询 + `workflow:stage` 事件实时推进（无需刷新）

> **运行状态存储说明**：`task:run` 将 TaskDefinition 转换为 Workflow 交给 `WorkflowEngine.run()`（§10.6，phase1-core.md），运行状态统一落在 **`workflow_runs` 表**（§5.5，phase1-core.md 迁移 004）——不新建 `task_runs` 表，避免与工作流运行状态双轨。前端 `DashboardOverview.task_runs`（§9.9，本文件 69 行）即查询 `workflow_runs` 的最近记录（`source='task'` 区分来源）。

**任务定义数据结构**（可保存为模板，落库 `workflows` 表，`definition` JSON）：

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct TaskDefinition {
    pub id: String,               // 新建时生成
    pub name: String,
    pub description: String,
    pub inputs: Vec<TaskInput>,   // 运行前用户填写的参数声明
    pub stages: Vec<TaskStageDef>,
    pub goals: Vec<TaskGoal>,     // 目标定义（§10.11，见 phase3-extend.md，可空 = 不启用目标监控）
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
    pub reflection: Option<ReflectionConfig>, // 反思配置（§10.9，见 phase3-extend.md，可空）
    pub model_hint: Option<String>,           // 模型建议（如 "plan"，可空）
    pub output_spec: Option<String>,          // 输出格式约定（可空）
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskInput {
    pub key: String,              // 如 "topic"
    pub label: String,            // 如 "研究主题"
    pub kind: InputKind,          // Text | Textarea | Select | Number
    pub options: Option<Vec<Value>>, // Select 的选项列表（kind=Select 时必填）
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
- 长会话自动截断提示：后端 PromptBuilder 已做滑动窗口（§13.1，见 phase3-extend.md），此处仅展示窗口内有效 tokens

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
| 4 | `{workdir}/.prism/memory.md` | 全量注入（项目记忆，§10.7，见 phase1-core.md） |
| 5 | `{workdir}/README.md` | 不注入，仅展示（避免噪声；可手动"注入此文件"） |

**交互规则**：

- 文件卡片点击 → 内嵌预览（`workspace:read-file`，默认只读渲染 Markdown/纯文本）
- ✏ 编辑 → 切换 Textarea（语法高亮），💾 保存 → `workspace:write-instructions` → 重新注入标记 ✅
- "未注入"文件提供 [注入此文件] 按钮 → 加入本次会话的 PromptBuilder（`session:inject-file` 命令，见 9.10.7 新增命令）
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

**新增命令**（§8.2 命令清单见 phase1-core.md，此处为补充）：

| 命令 | 参数 | 返回 |
|------|------|------|
| `session:inject-file` | `{session_id, path}` | `()` | 将指令文件注入本次会话 |
| `lsp:detect` | `{workdir}` | `Vec<LspCandidate>` | 推断候选 LSP（无进程启动） |
| `fs:watch` | `{workdir, enable}` | `()` | 开启/关闭工作目录变更监听 |

**布局关系**：主页面的 `AgentLauncher` 点击后进入 `/chat/{sessionId}`，对话页默认展开 Agent 侧边栏；`⌘\` 折叠。主页面板与侧边栏共用 `usage`/`mcp` 数据源，一次请求双端复用。侧边栏六 Tab 中"用量/指令"与 PromptBuilder（§10.7，见 phase1-core.md）共享注入状态，"MCP/LSP"与 RigAgent 工具执行共享运行状态。

---

## 10. 特色功能详细设计（Phase 2 部分）

> 注：§10 章节分散在三个文件——§10.1-10.3/10.5/10.9/10.11-10.13 见 `phase3-extend.md`；§10.4/10.6-10.8 见 `phase1-core.md`；本文件为 §10.10。
## 10. 特色功能详细设计（Phase 2 部分）

> 注：§10 章节分散在三个文件——§10.1-10.3/10.5/10.9/10.11-10.13 见 `phase3-extend.md`；§10.4/10.6-10.8 见 `phase1-core.md`；本文件为 §10.10。
> 注：§10 章节分散在三个文件——§10.1-10.3/10.5/10.9/10.11-10.13 见 `phase3-extend.md`；§10.4 主体/§10.6-10.8 见 `phase1-core.md`；本文件为 §10.4.1-10.4.4（市场搜索，自 phase1 移入）与 §10.10。

### 10.4.1 三源市场搜索（Phase 2，自 phase1 §10.4 移入）

> **Skill 技能系统主体（安装/注入/加载）见 phase1-core.md §10.4**；本节为市场搜索详设（T9 补充）。

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
### 10.10 人机协同（Human-in-the-Loop）

**来源**：Agentic Design Patterns Ch.13 — 人类监督、干预与升级。

**设计目标**：关键操作（文件写入、删除、外部 API 调用）需用户确认后执行；Agent 遇到无法处理的情况时自动升级给用户。

#### 10.10.1 工具审批流程

```rust
// 核心数据结构
#[derive(Serialize, Deserialize, Clone)]
pub struct ToolApprovalRequest {
    pub call_id: String,                 // 工具调用 ID
    pub tool_name: String,               // 工具名称
    pub arguments: serde_json::Value,    // 调用参数
    pub agent_id: String,                // 发起的 Agent
    pub risk_level: RiskLevel,           // 风险等级
    pub description: String,             // 人类可读描述
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum RiskLevel {
    Low,       // 自动放行（read/list/glob/grep）
    Medium,    // 静默记录（write_file 到已知目录）
    High,      // 需要审批（delete/edit/外部 API）
    Critical,  // 需要二次确认（rm -rf/数据库操作/发送消息）
}

pub enum ToolApprovalResponse {
    Approved,                   // 批准执行
    Rejected(String),           // 拒绝（附原因）
    AlwaysApprove(String),      // 本次会话始终批准此类工具
    Defer,                      // 延后（Agent 尝试其他方案）
}
```

**审批流程**：

```
Agent 调用工具 → ToolExecutor 检查 RiskLevel
  ├─ Low/Medium → 自动执行，记录日志
  ├─ High → emit('tool:approval-request', request)
  │         → 前端弹出 ToolApprovalDialog
  │         → 用户响应 → emit('tool:approval-response', response)
  │         → 批准 → 执行；拒绝 → 返回拒绝结果给 Agent
  └─ Critical → 二次确认弹窗 + 执行摘要预览
```

**前端 ToolApprovalDialog**：

```
┌─ 工具审批 ─────────────────────────────┐
│ Agent "研究员" 请求调用工具              │
│                                        │
│ 工具: write_file                       │
│ 风险: 🔴 高                            │
│                                        │
│ 参数:                                  │
│   path: src/main.rs                    │
│   content: (234 行代码变更)             │
│                                        │
│ 影响: 将覆盖现有文件                    │
│ ──────────────────────────────────────  │
│ [✅ 批准] [❌ 拒绝] [📋 查看详情]       │
│ [☑ 本次会话始终批准 write_file]         │
└────────────────────────────────────────┘
```

#### 10.10.2 升级机制

Agent 在以下情况自动升级给用户：

| 升级触发条件 | 处理方式 |
|-------------|----------|
| 连续 3 次工具调用失败 | 暂停执行 + 通知用户 + 建议替代方案 |
| 工具审批被拒绝 | Agent 尝试其他方案；若无则请求用户指导 |
| 上下文窗口 > 90% | 提示用户开启新会话或压缩上下文 |
| 工作流阶段失败 | 可选：自动跳过 / 重试 / 暂停等用户决策 |
| 检测到循环行为（重复相同操作 > 5 次） | 自动中断 + 诊断报告 |

---

## 5.7.4 会话标题搜索（数据存储横切设计补充）

> **归属**：Phase 2（会话列表搜索增强）· 数据存储完整设计见 `phase1-core.md` §5.7（跨阶段基础）
> **迁移**：`012_session_fts.sql`（独立迁移；不并入 009——遵循 §14.3 #28「迁移版本号必须 bump，禁止在已应用迁移上追加」）

```sql
-- 会话标题 FTS（轻量级，标题短文本）— 迁移 012_session_fts.sql
CREATE VIRTUAL TABLE sessions_fts USING fts5(
    title,
    session_id UNINDEXED,
    content='sessions',
    content_rowid='rowid',
    tokenize='unicode61'
);

-- 同步触发器（标题变更时同步索引；删除用 'delete' 行模式，同 messages_fts）
CREATE TRIGGER sessions_ai AFTER INSERT ON sessions BEGIN
    INSERT INTO sessions_fts(rowid, title, session_id)
    VALUES (new.rowid, new.title, new.id);
END;

CREATE TRIGGER sessions_au AFTER UPDATE OF title ON sessions BEGIN
    INSERT INTO sessions_fts(sessions_fts, rowid, title, session_id)
    VALUES ('delete', old.rowid, old.title, old.id);
    INSERT INTO sessions_fts(rowid, title, session_id)
    VALUES (new.rowid, new.title, new.id);
END;

-- 搜索：支持标题模糊搜索 + 按时间排序
SELECT s.*, bm25(sessions_fts) as score
FROM sessions_fts f
JOIN sessions s ON s.id = f.session_id
WHERE sessions_fts MATCH ?
ORDER BY score, s.updated_at DESC
LIMIT ?;
```

**要点**：
- 会话列表/侧边栏搜索走 `sessions_fts`（替代 `LIKE` 扫描），命中高亮用 `snippet()`（模式同 `phase1-core.md` §5.7.2）
- 更新触发器（`sessions_au`）在标题自动重命名（§9.10.3 指令相关 + §1 自动重命名，见 prism-agent-r.md）时同步索引

