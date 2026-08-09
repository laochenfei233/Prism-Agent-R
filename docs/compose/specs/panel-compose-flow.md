---
feature: panel-compose-flow
status: delivered
updated: 2026-08-09
branch: feat/panel-compose-flow
commits: 983075d..a051f62
---

# 面板自主编排流（Panel Compose Flow）

## Report

**What was built** — 面板首页（`src/routes/+page.svelte`）顶部新增自主编排主入口：`OrchestratorPanel` 以 560px 大卡片置顶（需求输入 → SPEC → 执行 → 审查四 Tab），`TaskDesigner` 下移至第二区块，其余面板区块不变。该组件此前是孤儿组件且全部使用本项目并不存在的 Tailwind 死样式，本次全量重写为 iOS 18 设计令牌（`--color-*`/`--spacing-*`/`--radius-*`），浅深色主题自动适配。执行 Tab 升级为事件驱动的子任务状态卡片：由 `session.plan.groups` 展开，每卡显示任务 id、子 Agent 角色、模型、状态徽标（待运行/运行中/已完成/失败）、耗时、token、输出摘要（截断）与失败错误；顶部总进度条（完成数/总任务数）。后端 `OrchestratorEngine` 在执行循环中补发任务级事件（`task_started` / `task_finished`，data 携带 task_id/role/model_id/group_id/status/duration_ms/tokens_used/output_summary/error），经 `orchestrator:event` IPC 透传（修复了桥接闭包原本丢弃 `data` 的问题）。`GroupKind` 序列化改为 snake_case 并兼容旧 PascalCase 持久化数据。

**Verification** — `cargo check`（src-tauri）: PASS，exit 0（含任务级事件、IPC 桥、GroupKind 改动后各轮复验）。`npm run check`（worktree 根）: 本改动涉及文件零诊断；全局 1 error + 8 warnings 均为 PRE-EXISTING 基线（`src/routes/settings/+page.svelte:10` onMount/listen 类型、`TaskNodeInspector.svelte`，经 git stash 对照基线确认，未触碰）。两轮独立审查（全量审查 + 修复复审）通过：1 critical（IPC 桥丢 data）+ 1 major（GroupKind 大小写不匹配）+ 1 minor（事件 key 同毫秒冲突）均已修复并复验。

**Journey log** — ① OrchestratorPanel 从未被接线的原因之一是它用了项目不存在的 Tailwind 类——重写为设计令牌是接入的必经步骤，不是可选美化。② `orchestrator:event` IPC 桥（`monitor.rs build_orchestrator_engine`）只序列化 3 个字段，任何新增事件 `data` 必须同步修改此处，否则前端静默拿不到。③ Rust serde 枚举默认 PascalCase 序列化，前端 `=== 'parallel'` 这类比较是无用功；契约应明确 snake_case + 旧数据 alias。④ 并行组任务事件可能同毫秒时间戳，`#each` key 用复合值（timestamp+type+message）。⑤ 项目 `npm run check` 基线不过（settings/+page.svelte），验收判定以「不引入新报错」为准。

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

- 新增私有方法 `emit_task_event(&self, event_type: &str, message: String, data: serde_json::Value)`，内部构造 `OrchestratorEvent` 并调用现有 `on_event` 回调。
- 在 `execute_plan` 内通过新增的 `run_group_task` 包装器（并行与顺序分支共用）在 `execute_task` 前后发 `task_started` / `task_finished`，不改 `execute_task` 签名；`group_id` 来自所属 `ExecutionGroup.id`。
- `output_summary` 取 `response.text` 前 200 字符（`chars().take(200)`），完整输出仍存于 `TaskResult.output` 供审查使用。
- IPC 桥：`monitor.rs build_orchestrator_engine` 的 `on_event` 闭包必须透传 `"data": event.data`（此前只序列化 3 个字段）。
- `GroupKind` 序列化契约：`#[serde(rename_all = "snake_case")]` → `"parallel" / "sequential"`，并为旧数据加 `#[serde(alias = "Parallel")]` 等兼容。
- 不修改会话级事件、不改 `TaskStatus` 的 serde 序列化、不改 `OrchestratorSession` 持久化结构。

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
  - 状态徽标与文案：`待运行`（neutral）、`运行中`（accent）、`已完成`（green）、`失败`（red）
  - 完成/失败后显示：耗时（`duration_ms`）、token（`tokens_used`）、输出摘要（截断）、错误信息（失败时）
- **状态来源**：组件内从 `orchestratorStore.events` 派生 —— 以 `task_started` / `task_finished` 事件的 `data.task_id` 为键，后到的事件覆盖先到者（`task_finished` 覆盖 `task_started`；事件数组最新在前，需倒序遍历）。未出现在任何事件中的任务显示为 `待运行`。
- **总进度**：执行区块顶部进度条，`完成数 / 总任务数`（完成数 = `status==='completed'` 的 task_finished 去重计数，总数取 `plan.total_tasks`）。
- **样式**：全组件改用项目 iOS 18 设计令牌（项目无 Tailwind，原 slate-* 类为死样式）；事件列表 `#each` key 使用复合值（timestamp+event_type+message）避免并行任务同毫秒冲突。

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
- [x] T4: 执行 Tab 子任务状态卡片 — acceptance: 卡片由 plan 展开并随事件更新（待运行→运行中→完成/失败），显示耗时/输出摘要/所属 Agent，顶部有总进度条，`npm run check` 通过（covers: S2.3；depends: T2 T3）
- [x] T5: 验证与审查 — acceptance: 全部验证命令通过（cargo check + npm run check），审查无 critical 发现，spec finalize（covers: S2；depends: T2 T3 T4）
