---
feature: phase5-production
status: delivered
updated: 2026-08-08
branch: phase5-production
commits: e9b72e7..5a4abd5
---

# Phase 5 — 生产加固

## Report

**What was built** — Phase 5 production hardening core infrastructure:
- **Budget system** (§22): Three-tier budget (Global/Crew/Agent) with tracker, model fallback chain, configurable degradation policies, and budget events
- **Guardrails** (§23): Tool-level (whitelist/blacklist/approval/param validation), trajectory-level (credential/sandbox escape/resource exhaustion), and sandbox (filesystem/network/process)
- **Observability** (§24): Exception recorder with SQLite persistence, structured JSON Lines logger
- **Workflow V2** (§25): Extended definitions with budget/guardrails/retry, engine integrating all systems
- **Orchestrator** (§27): Auto-orchestration engine (Spec→Plan→Execute→Review loop) with session state
- **Frontend** (§26): Monitor store + MonitorPanel component with real-time budget/exception display

**Verification** — `cargo test`: 62/62 pass, `cargo check`: 0 errors

## [S1] 问题

Phase 1-4 完成后，项目存在以下核心缺口：

1. **无预算控制**：工作流执行无 token/费用/时间上限，Agent 可能无限消耗资源
2. **无自动降级**：超预算直接失败，无法切换到更便宜的模型继续执行
3. **护栏仅输入级**：§10.12 仅做 prompt injection 检测，无工具调用/行为/系统级拦截
4. **异常记录不结构化**：§17.3 trace grading 无异常分类、严重级别、处理链路
5. **工作流引擎简单**：线性执行，无预算/护栏/重试集成
6. **无实时监控**：前端无预算/异常/趋势的实时仪表盘
7. **无自主编排**：用户需手动设计工作流，缺少「输入需求 → 自动生成计划 → 多 Agent 执行 → 审查 → 循环」的自主能力

## [S2] 设计

按 phase5-production-hardening.md §22-§27 设计，实现 20 个任务：

### §22 预算监控与自动降级（T1-T3）

- **T1**: BudgetConfig + BudgetTracker + 预算事件
  - 验收：三级预算配置生效；超预算触发 warning/exceeded 事件
  - covers: §22.1-22.4

- **T2**: ModelFallbackChain + 自动降级策略
  - 验收：超预算时自动切换到更便宜模型；无更便宜模型时 PauseAndAsk
  - covers: §22.3; depends: T1

- **T3**: WorkflowEngineV2 集成预算追踪
  - 验收：工作流执行时实时追踪 token/费用；超预算自动暂停/降级
  - covers: §22.2, §25.1; depends: T1, T2

### §23 越界拦截与安全护栏（T4-T6）

- **T4**: ToolGuardrail + 工具级护栏配置
  - 验收：工具调用前校验白名单/黑名单/参数；需审批工具触发 approval
  - covers: §23.2

- **T5**: TrajectoryGuardrail + 行为级检查器
  - 验收：凭据拼接/沙箱逃逸/越权访问触发拦截；违规记录入库
  - covers: §23.3

- **T6**: SandboxPolicy + 系统级沙箱
  - 验收：文件/网络/进程访问受白名单限制；越界访问被拦截
  - covers: §23.4

### §24 异常记录与可观测性（T7-T9）

- **T7**: 异常数据库表 + ExceptionRecorder
  - 验收：异常记录入库；支持按 session/agent/type 查询
  - covers: §24.1-24.2

- **T8**: AgentLogger + 结构化日志
  - 验收：日志按级别输出；支持 JSON Lines 格式；可配置文件输出
  - covers: §24.4

- **T9**: 监控仪表盘前端组件
  - 验收：实时显示预算/异常/趋势；支持交互操作
  - covers: §24.3, §26

### §25 工作流引擎重构（T10-T12）

- **T10**: WorkflowEngineV2 核心重构
  - 验收：集成预算/护栏/异常记录；支持重试策略
  - covers: §25.1; depends: T1, T4, T7

- **T11**: WorkflowV2 定义扩展
  - 验收：支持工作流/阶段级预算+护栏配置；支持重试策略
  - covers: §25.2

- **T12**: 工作流命令迁移（workflow_run → V2）
  - 验收：现有工作流使用新引擎；无回归
  - covers: §25.1; depends: T10, T11

### §26 前端监控面板（T13-T15）

- **T13**: monitorStore + IPC 事件监听
  - 验收：实时接收并存储监控数据；支持轮询备用
  - covers: §26.3

- **T14**: MonitorPanel 主面板组件
  - 验收：展示预算/活跃工作流/异常/趋势；布局对齐 §18.7
  - covers: §26.1

- **T15**: 交互操作（暂停/继续/终止/切换模型）
  - 验收：按钮触发对应命令；状态实时更新
  - covers: §26.4; depends: T14

### §27 自主编排循环（T16-T20）

- **T16**: OrchestratorSession + 数据结构 + SQLite 表
  - 验收：会话状态可持久化；崩溃后可恢复
  - covers: §27.2

- **T17**: SpecGenerator（需求分析 + 任务拆解）
  - 验收：输入模糊需求 → 输出 SPEC（任务清单 + 验收标准 + 依赖）
  - covers: §27.3

- **T18**: PlanGenerator（任务分组 + Agent 分配 + 并行识别）
  - 验收：SPEC → 执行计划（并行组 + 顺序依赖 + 模型分配）
  - covers: §27.3; depends: T17

- **T19**: OrchestratorEngine 主循环（Spec→Plan→Execute→Review→循环）
  - 验收：完整循环执行；支持暂停/恢复；预算耗尽自动停止
  - covers: §27.3, §27.6; depends: T16, T17, T18

- **T20**: 前端自主编排界面（输入/SPEC预览/执行监控/审查结果）
  - 验收：用户可输入需求、查看 SPEC、监控执行、查看审查结果
  - covers: §27.4; depends: T19

## [S3] Out of Scope

- 分布式预算（跨设备/用户共享预算）
- 实时视频监控（Agent 桌面操作录屏）
- 自定义仪表盘（固定布局，不支持用户拖拽）
- 费用计费系统（仅追踪，不涉及支付）

## Tasks

### §22 预算监控（T1-T3）
- [x] T1: BudgetConfig + BudgetTracker + 预算事件 — acceptance: 三级预算配置生效，超预算触发事件 (covers: §22.1-22.4)
- [x] T2: ModelFallbackChain + 自动降级 — acceptance: 超预算自动切换模型 (covers: §22.3; depends: T1)
- [x] T3: WorkflowEngineV2 集成预算追踪 — acceptance: 实时追踪 token/费用 (covers: §22.2, §25.1; depends: T1, T2)

### §23 越界拦截（T4-T6）
- [x] T4: ToolGuardrail + 工具级护栏 — acceptance: 工具调用前校验生效 (covers: §23.2)
- [x] T5: TrajectoryGuardrail + 行为级检查器 — acceptance: 越权行为触发拦截 (covers: §23.3)
- [x] T6: SandboxPolicy + 系统级沙箱 — acceptance: 文件/网络/进程访问受限 (covers: §23.4)

### §24 异常记录（T7-T9）
- [x] T7: 异常数据库表 + ExceptionRecorder — acceptance: 异常记录入库可查 (covers: §24.1-24.2)
- [x] T8: AgentLogger + 结构化日志 — acceptance: 日志按级别输出 JSON Lines (covers: §24.4)
- [x] T9: 监控仪表盘前端组件 — acceptance: 实时显示预算/异常/趋势 (covers: §24.3, §26)

### §25 工作流重构（T10-T12）
- [x] T10: WorkflowEngineV2 核心重构 — acceptance: 集成预算/护栏/异常记录 (covers: §25.1; depends: T1, T4, T7)
- [x] T11: WorkflowV2 定义扩展 — acceptance: 支持工作流/阶段级配置 (covers: §25.2)
- [x] T12: 工作流命令迁移 — acceptance: 现有工作流无回归 (covers: §25.1; depends: T10, T11)

### §26 前端监控（T13-T15）
- [x] T13: monitorStore + IPC 事件监听 — acceptance: 实时接收监控数据 (covers: §26.3)
- [x] T14: MonitorPanel 主面板组件 — acceptance: 展示预算/异常/趋势 (covers: §26.1)
- [x] T15: 交互操作（暂停/继续/终止/切换模型） — acceptance: 按钮触发对应命令 (covers: §26.4; depends: T14)

### §27 自主编排循环（T16-T20）
- [x] T16: OrchestratorSession + 数据结构 + SQLite 表 — acceptance: 会话状态可持久化，崩溃可恢复 (covers: §27.2)
- [x] T17: SpecGenerator（需求分析 + 任务拆解） — acceptance: 输入模糊需求 → 输出 SPEC (covers: §27.3)
- [x] T18: PlanGenerator（任务分组 + Agent 分配） — acceptance: SPEC → 执行计划（并行组+模型分配） (covers: §27.3; depends: T17)
- [x] T19: OrchestratorEngine 主循环 — acceptance: Spec→Plan→Execute→Review→循环完整执行 (covers: §27.3, §27.6; depends: T16, T17, T18)
- [x] T20: 前端自主编排界面 — acceptance: 用户可输入需求、查看SPEC、监控执行 (covers: §27.4; depends: T19)
