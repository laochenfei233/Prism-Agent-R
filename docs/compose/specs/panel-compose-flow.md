---
feature: panel-compose-flow
status: designed
updated: 2026-08-09
branch: feat/panel-compose-flow
commits: <base-sha>..<head-sha> # filled at delivery
---

# 面板自主编排流（Panel Compose Flow）

## Report

## [S1] Problem

面板首页（`src/routes/+page.svelte`）目前以手动任务设计器（`TaskDesigner`）为最前区块，用户无法从「描述需求」直接进入自动化流程。后端 `OrchestratorEngine`（§27 Spec→Plan→Execute→Review）与前端 `OrchestratorPanel`（需求输入 → SPEC → 执行 → 审查）均已实现，但：

1. `OrchestratorPanel` 未被任何页面引用（孤儿组件），用户无法触达自主编排能力；
2. 引擎只发会话级粗粒度事件（`executing` / `execution_completed` 等），`OrchestratorEvent.data` 恒为 `None`，没有按任务粒度的状态（哪个子 Agent 在跑、哪个完成/失败），执行阶段无法展示任务级进度；
3. 执行 Tab 仅展示计划分组与事件流文本，缺少直观的子任务状态卡片。

目标体验：面板先让用户输入需求；需求较大时，面板展示整体进度、每个子任务的细节状态，并由多个子 Agent（并行分组）协作完成。

## [S2] Design

### 2.1 事件契约（后端新增任务级事件）

在 `src-tauri/src/core/orchestrator/engine.rs` 增加任务级事件，沿用现有 `OrchestratorEvent { event_type, message, timestamp, data }` 结构（`data: Option<serde_json::Value>` 已存在，目前恒为 `None`）。

- **`task_started`**：任务开始前发出。
  - message: `开始执行任务 {task_id}`
  - data: `{ "task_id": "T1", "role": "assistant", "model_id": "gpt-4o", "group_id": "<group-uuid>" }`
- **`task_finished`**：任务结束后发出（成功与失败都发）。
  - message: `任务 {task_id} 完成` 或 `任务 {task_id} 失败`
  - data: `{ "task_id": "T1", "status": "completed" | "failed", "duration_ms": 1234, "tokens_used": 567, "output_summary": "<前 200 字符>", "error": "<错误信息，可选>" }`

实现方式：

- 新增私有方法 `emit_task_event(&self, event_type: &str, message: &str, data: serde_json::Value)`，内部构造 `OrchestratorEvent` 并调用现有 `on_event` 回调（与 `emit_event` 同路径，事件经 `orchestrator:event` IPC 推送到前端）。
- 在 `execute_task`（`engine.rs`）开头发 `task_started`、结束时发 `task_finished`。`execute_task` 已持有 `task: &PlannedTask`（含 `spec_task_id`、`agent_config.role`、`agent_config.model_id`）与计算结果，无需改签名。
- `output_summary` 取 `response.text` 前 200 字符（`chars().take(200)`），完整输出仍存于 `TaskResult.output` 供审查使用。
- 不修改会话级事件、不改 `TaskStatus` 的 serde 序列化、不改 `OrchestratorSession` 持久化结构（`task_results` 不持久化是既有已知限制，见 S3）。

### 2.2 面板首页布局（前端）

`src/routes/+page.svelte`：

- 顶部第一区块改为自主编排入口：`<OrchestratorPanel />`，置于固定高度容器（约 560px，内部自带滚动），作为「先输入需求」的主入口。
- 原第一区块 `TaskDesigner` 下移为第二区块（保留手动任务设计能力，不删除）。
- 其余区块（Agent Launcher / UsageTrend / Stats / Skills / MCP / Recent Sessions / Setup Banner）顺序不变。

### 2.3 子任务状态卡片（前端执行 Tab）

`src/lib/components/dashboard/OrchestratorPanel.svelte` 执行 Tab（`activeTab === 'execution'`）：

- **任务列表**：由 `session.plan.groups` 展开为卡片列表，按组标注 `并行` / `顺序`；每卡片显示：
  - 任务 id（`spec_task_id`）
  - 所属子 Agent 角色（`agent_config.role`）与模型（`agent_config.model_id`）
  - 状态徽标与文案：`待运行`（slate）、`运行中`（blue）、`完成`（emerald）、`失败`（red）
  - 完成/失败后显示：耗时（`duration_ms`）、token（`tokens_used`）、输出摘要（截断）、错误信息（失败时）
- **状态来源**：组件内从 `orchestratorStore.events` 派生 —— 以 `task_started` / `task_finished` 事件的 `data.task_id` 为键，后到的事件覆盖先到者（`task_finished` 覆盖 `task_started`）。未出现在任何事件中的任务显示为 `待运行`。
- **总进度**：执行区块顶部进度条，`完成数 / 总任务数`（总任务数取 `plan.total_tasks` 或展开组内任务数）。
- **数据流**：`events.data` 类型为 `any`，store 无需改动；派生逻辑全部在组件内 `$derived`。

### 2.4 不改动的部分

- 引擎执行/审查/修复/预算逻辑、`orchestrator_start/resume/pause/stop/list` 命令签名与返回类型。
- 会话级事件文本与顺序。
- `MonitorPanel`、Agent 页、其余页面。

## [S3] Out of Scope

- 恢复（resume）会话后的任务状态重建：`task_results` 与 `history` 不持久化，resume 后任务卡片只能显示 `待运行`，直至新一轮事件到达。
- 引擎层面并行度/超时/修复策略的调整；`execute_task` 目前无工具调用，保持现状。
- 任务失败自动重试与重试计数展示。
- 多轮循环（cycle）历史在卡片上的区分展示。
- 监控面板（`MonitorPanel`）的接线与完善。

## Tasks

- [ ] T1: 编写本 spec 并建立 worktree — acceptance: `docs/compose/specs/panel-compose-flow.md` 存在，`.worktrees/panel-compose-flow` 分支可用（covers: S1 S2）
- [ ] T2: 后端任务级事件 — acceptance: `cargo check` 通过；`execute_task` 前后分别发出 `task_started`/`task_finished`，事件 `data` 含 task_id/status/duration_ms/tokens_used/output_summary（covers: S2.1；depends: T1）
- [ ] T3: 面板首页接入 OrchestratorPanel 置顶 — acceptance: `+page.svelte` 顶部渲染需求输入主入口，TaskDesigner 位于其下，`npm run check` 通过（covers: S2.2；depends: T1）
- [ ] T4: 执行 Tab 子任务状态卡片 — acceptance: 卡片由 plan 展开并随事件更新（待运行→运行中→完成/失败），显示耗时/输出摘要/所属 Agent，顶部有总进度条，`npm run check` 通过（covers: S2.3；depends: T2 T3）
- [ ] T5: 验证与审查 — acceptance: 全部验证命令通过（cargo build + npm run check），审查无 critical 发现，spec finalize（covers: S2；depends: T2 T3 T4）
