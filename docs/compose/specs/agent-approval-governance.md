---
feature: agent-approval-governance
status: delivered
updated: 2026-08-30
branch: feat/agent-approval-governance
commits: 未提交（为保留当前工作区的既有 WIP，代码与 spec 一同留在分支工作区）
---

# Agent Approval Governance

## Report

**What was built**

Agent 审批治理已落地：新增持久化审批请求表和作用域化授权规则表；`ToolApprovalStore` 升级为队列化、可超时、可审计的审批管理器；`delegate_to_agent` 增加结构化委派 Gate，展示父/子 Agent、任务摘要与子 Agent 能力计划；`AlwaysApprove` 按 `agent_id + tool_name + normalized_arguments_hash` 隔离，默认最多 10 次；`Critical` 工具强制二次确认且不能 AlwaysApprove；子 Agent 支持按 Agent 配置 MCP Server 数、单 Server 工具数、总 MCP 工具数、Skill 数、总工具数与 allowlist/denylist。

前端审批弹窗改为多请求队列，支持拒绝原因、委派上下文、等待数量与二次确认态；Agent 页新增「子 Agent」面板，可配置能力配额并查看最近审批历史。

**Verification**

- `cargo clippy --all-targets -- -D warnings` — PASS（0 警告）
- `cargo test --all-targets` — PASS（130 unit + 6 integration）
- `npm run check` — PASS（0 错误，1 条既有 a11y 警告）
- `npm test` — PASS（12/12）

**Journey log**

1. 沙箱把 `.git` 设为只读，建分支需提升权限；`npm run check/test` 读取父目录也被沙箱拦截，需提升权限验证。
2. `apply_patch.bat` 在 Windows 下无法正确传多行参数，改用同一底层 `codex.exe --codex-run-as-apply-patch` 打补丁。
3. `end_to_end_capability_loop` 暴露 HEAD 已存在的工具命名不匹配（`write_file/read_file` vs `file_write/file_read`），在风险表补了别名，属审批风险分类范畴。
4. 自审发现 Critical 首次确认会写入 AlwaysApprove 规则、allowlist 可能被总上限截掉，已修复并补测试。
5. 用户确认委派改为一次确认：`delegate_to_agent` 标记为 Low，统一由 `DelegationGate` 审批。
6. 为保留用户既有 WIP，未创建 worktree 也未自动提交代码；实现直接落在 `feat/agent-approval-governance` 分支工作区。

## [S1] Problem

Agent 运行时已经有工具级 HITL，但审批体验和安全边界仍不完整：

1. `delegate_to_agent` 目前只命中未知工具的 High 风险兜底，审批请求缺少父 Agent、子 Agent、子任务和子 Agent 能力计划等上下文，用户难以判断是否应该放行。
2. 审批请求没有持久化。应用重启或前端错过事件后，无法可靠追溯谁在何时批准/拒绝了哪个操作。
3. 多个审批请求并发时，前端只保留最后一个请求，前面的 pending receiver 可能只能等待超时。
4. `AlwaysApprove` 只按全局工具名放行。一个 Agent 批准某个写文件工具后，等于放大了所有 Agent 对同名工具的授权。
5. `RiskLevel::Critical` 只是枚举注释声明需要二次确认，实际执行路径没有二次确认。
6. 子 Agent 已经受全局工具开关和数量限制，但缺少按 Agent 配置的 MCP/Skill/工具配额表，无法精确表达“最多读取 2 个 MCP Server、每个 Server 最多 3 个工具、最多 2 个 Skill”。

## [S2] Design

### 审批模型

引入统一 `ApprovalRequestKind`：

| Kind | 触发点 | 默认策略 |
|------|--------|----------|
| `tool` | High/Critical 工具执行前 | `ask` |
| `delegation` | Orchestrator 调用子 Agent 前 | `ask` |

审批请求统一携带 `request_id`、`kind`、`session_id`、`agent_id`、`parent_agent_id`、`parent_session_id`、`child_agent_id`、`tool_name`、`arguments`、`task_summary`、`capability_summary`、`risk_level`、`reason`、`created_at` 和 `expires_at`。

`delegate_to_agent` 在风险表中标记为 Low，工具级 HITL 不重复拦截；`DelegationGate` 在构建子 Agent、调用模型前运行，是委派的唯一审批点，一次确认即完成。拒绝委派时，Orchestrator 收到明确错误结果，但流程不会被当成应用崩溃。

### 授权作用域

移除“按全局工具名 Always Approve”的行为。`AlwaysApprove` 的实际持久化作用域是 `agent_id + tool_name + normalized_arguments_hash`。

例如批准 Agent A 执行 `file_write({ path: "/project/a.txt", ... })`，只允许 Agent A 后续对完全相同规范化目标重复执行；不会自动放行 Agent B，也不会放行 Agent A 写其他路径。

新增 `agent_approval_rules`：

```sql
CREATE TABLE agent_approval_rules (
    id                     TEXT PRIMARY KEY,
    agent_id               TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    tool_name              TEXT NOT NULL,
    normalized_args_hash   TEXT NOT NULL,
    scope_hash             TEXT NOT NULL,
    max_uses               INTEGER,
    use_count              INTEGER NOT NULL DEFAULT 0,
    expires_at             INTEGER,
    created_at             INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL,
    UNIQUE(agent_id, tool_name, normalized_args_hash)
);
```

匹配规则时原子递增 `use_count`；达到 `max_uses` 或过期后自动失效。v1 的 `AlwaysApprove` 默认 `max_uses = 10`，避免变成永久授权。

### 审批队列与持久化

新增 `agent_approval_requests`：

```sql
CREATE TABLE agent_approval_requests (
    id                  TEXT PRIMARY KEY,
    kind                TEXT NOT NULL,
    status              TEXT NOT NULL,
    session_id          TEXT,
    parent_session_id   TEXT,
    agent_id            TEXT NOT NULL,
    parent_agent_id     TEXT,
    child_agent_id      TEXT,
    tool_name           TEXT,
    arguments           TEXT,
    task_summary        TEXT,
    capability_summary  TEXT,
    risk_level          TEXT NOT NULL,
    reason              TEXT,
    decision            TEXT,
    decision_reason     TEXT,
    decided_by          TEXT,
    decided_at          INTEGER,
    expires_at          INTEGER,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL
);
```

状态流是 `pending -> approved / rejected / deferred / expired / cancelled`。后端先生成并持久化 `pending` 记录，再发送 Tauri 事件。响应命令根据 `request_id` 更新记录并唤醒 waiter。

### 前端审批队列

`ToolApprovalDialog` 改为队列视图：

- 后端事件只负责 append，不覆盖已有请求。
- 弹窗显示当前请求和等待数量。
- 用户逐条决定；拒绝必填原因。
- 委派审批显示父 Agent、子 Agent、任务摘要、子 Agent 能力摘要。
- 工具审批显示工具名、参数、风险等级、Agent 和会话。
- 超时默认从 30 秒提高到 300 秒，可通过 `approval.timeout_seconds` 配置；`0` 表示不自动超时。

### Critical 二次确认

`execute_tool` 对 `Critical` 请求执行两次独立审批：

1. 第一次请求展示完整参数和风险说明。
2. 第一次批准后生成新的 `request_id`，进入第二次确认。
3. 第二次确认只允许 `Approved`；`AlwaysApprove` 在二次确认界面不可用。
4. 两次决定都写入审批审计表。

`Critical` 只用于不可逆或跨边界动作。`delegate_to_agent` 在 v1 中标记为 `Low` 并统一走 `DelegationGate` 一次审批；后续若新增批量删除、外部发送、数据库迁移类工具，才标为 `Critical`。

### 子 Agent 能力配额

保留全局设置 `orchestrator.sub_agent_tools_enabled` 和 `orchestrator.sub_agent_max_tools`。

在目标 Agent 的 `configuration` 中新增可选 `sub_agent_capabilities`：

```json
{
  "sub_agent_capabilities": {
    "max_mcp_servers": 2,
    "max_mcp_tools": 5,
    "mcp_server_tool_limits": {
      "<mcp_server_id>": 3
    },
    "max_skills": 2,
    "max_total_tools": 12,
    "tool_allowlist": [],
    "tool_denylist": []
  }
}
```

规则：

1. 空 `tool_allowlist` 表示不额外收紧；非空时只保留列表中的工具。
2. `tool_denylist` 最后生效，优先级高于 allowlist 和全局数量限制。
3. MCP Server 先按 `max_mcp_servers` 截断，再按每个 Server 的 `mcp_server_tool_limits` 截断，最后按 `max_mcp_tools` 截断。
4. Skill 只作为提示上下文加载，按 `max_skills` 截断；Skill 不能绕过工具配额。
5. `max_total_tools` 覆盖全局 `orchestrator.sub_agent_max_tools`；未配置时继续使用全局值。
6. Agent 设置页显示当前 MCP Server、Skill 和工具数量配额表，保存后下一次委派生效。

### 数据与事件兼容

- 旧 `ToolApprovalRequest` 事件新增字段时全部使用可选字段和 serde 默认值，前端可以兼容旧 payload。
- 旧内存 `always_approve` 集合删除，启动时不迁移旧授权；用户重新批准一次即可形成带作用域的授权。
- 所有审批事件 payload 使用持久化记录 ID，避免前端事件乱序导致错绑。

## [S3] Scope

### In scope

- 统一工具与委派审批模型。
- 审批请求持久化、审计和队列化前端交互。
- 作用域化、可过期、可计数的 Always Approve 规则。
- Critical 工具二次确认。
- 按 Agent 配置子 Agent MCP/Skill/工具配额。
- Agent 设置页的能力配额和审批历史入口。

### Out of scope

- 任务完成结果复审和 Edict 式封驳重做。
- 多层子 Agent 委派；子 Agent 仍然不能再委派。
- 飞书、Telegram 等外部审批通知。
- 跨设备审批同步。
- 对已有 Compose 工作流状态机重写。

## Tasks

- [x] T1: 新增审批请求/规则迁移、模型和查询服务（depends: none）
- [x] T2: 升级 Approval Store 为队列化、作用域化和持久化管理器（depends: T1）
- [x] T3: 扩展工具审批 payload、超时配置和 Critical 二次确认（depends: T2）
- [x] T4: 实现结构化 `DelegationGate` 并接入 `delegate_to_agent`（depends: T2）
- [x] T5: 实现子 Agent MCP/Skill/工具配额解析和注册过滤（depends: T1）
- [x] T6: 改造全局审批弹窗为多请求队列，支持委派上下文展示（depends: T2）
- [x] T7: Agent 设置页增加能力配额表和审批历史入口（depends: T1、T5）
- [x] T8: 增加审批、配额、委派拒绝和并发审批测试（depends: T3、T4、T5、T6）
- [x] T9: 运行 Rust/前端检查、测试和构建（depends: T8）

## [S4] Acceptance

- [x] Orchestrator 委派子 Agent 前出现结构化审批请求，包含父 Agent、子 Agent、任务摘要和子能力摘要。
- [x] 委派只出现一次审批确认，不再叠加工具级 HITL。
- [x] 拒绝委派后 Orchestrator 收到失败工具结果，不会创建子 Agent 运行。
- [x] 并发 3 个审批请求时，前端按队列逐个展示；每个决定唤醒对应后端 waiter。
- [x] Agent A 的 Always Approve 不会放行 Agent B；参数规范化哈希不同也不会复用授权。
- [x] Always Approve 达到使用次数或过期后必须重新审批。
- [x] 所有审批请求和最终决定都能在本地数据库中查到完整审计记录。
- [x] Critical 工具必须完成两次独立确认才执行；第二次确认不能使用 Always Approve。
- [x] 子 Agent 能力配额可限制 MCP Server 数、单个 Server 工具数、总 MCP 工具数、Skill 数和总工具数。
- [x] 关闭子 Agent 工具开关时，上述配额不再生效，子 Agent 回到纯 LLM 推理。
- [x] `npm run check`、`npm test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --lib` 全部通过。
