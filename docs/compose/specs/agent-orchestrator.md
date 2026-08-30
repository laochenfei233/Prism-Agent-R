---
feature: agent-orchestrator
status: delivered
updated: 2026-08-28
branch: master
commits: pending
---

# Agent Orchestrator（总智能体 / 子 Agent 编排）

## [S1] Problem

用户创建了多个 Agent（翻译、编程、写作等），但不一定清楚每个 Agent 的边界。面对复杂需求时，用户需要手动拆分任务、逐个调用 Agent、复制粘贴中间结果，操作繁琐且容易遗漏。

需要一个**总智能体（Orchestrator）**：
- 用户把需求交给它，不需要知道该找谁。
- 它自动选择合适的 Agent 作为子 Agent 执行子任务。
- 它收集子 Agent 的结果，汇总后输出最终答案。

## [S2] Design

### 核心决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 身份 | **真实 Agent 记录**，标记 `is_orchestrator = true` | 出现在九宫格；有完整会话和生命周期；用户可以直接对话 |
| 创建方式 | 首次启动时自动 seed（如果不存在） | 用户零配置即可使用 |
| 子 Agent 调用机制 | 自定义 Tool `delegate_to_agent` 注册到 ToolRegistry | 复用现有工具调用链，前端已有 tool_call/tool_result UI |

| 委派深度 | 1（orchestrator 可调子 Agent，子 Agent 不能再委派） | 防止无限递归；v1 不需要多层编排 |
| 子 Agent 可见性 | tool_call / tool_result 事件已推送到前端 | 用户能看到"正在调用翻译 Agent…"的过程 |
| 模型 | 使用 orchestrator 自身配置的模型 | 与普通 Agent 一致，用户可自行更换 |

### 编排流程

```text
用户消息（"帮我翻译这段英文并总结要点"）
         │
         ▼
  Orchestrator LLM 思考
         │
         ├─ 决定调用 delegate_to_agent(agent_id="翻译Agent", task="翻译：...")
         │       │
         │       ▼
         │  创建 temp session → 子 Agent 生成翻译 → 返回 text
         │       │
         ├─ 决定调用 delegate_to_agent(agent_id="写作Agent", task="总结要点：...")
         │       │
         │       ▼
         │  创建 temp session → 子 Agent 生成摘要 → 返回 text
         │
         ▼
  Orchestrator 汇总所有子 Agent 结果
         │
         ▼
  输出最终回复给用户
```

### `delegate_to_agent` Tool

```rust
// 参数
{ "agent_id": "xxx", "task": "具体子任务描述" }

// 返回
ToolOutput { content: "子 Agent 的完整回复文本", is_error: false }
```

实现要点：
- 实现 `ToolExecutor` trait（与 `McpToolExecutor`、`WebSearchTool` 同层）。
- `execute()` 内部：
  1. 按 `agent_id` 查询 Agent 和 Provider/Model 配置。
  2. 调用 `sessionApi.create()` 创建临时 session（标题：`[Orchestrator] {task 前 30 字}`）。
  3. 构建 provider + system prompt + **空 ToolRegistry**（子 Agent 不携带 MCP/skill 等工具，保持轻量）。
  4. 直接 `agent.run(GenerationRequest)` 同步等待结果。
  5. 删除临时 session（可选，或保留作为审计记录）。
  6. 返回 `ToolOutput { content: result.text }`。
- Agent 不存在或执行失败：返回 `ToolOutput { content: "Agent not found", is_error: true }`。

### Orchestrator System Prompt（seed 时写入）

```text
你是一个智能任务协调者。你可以将子任务委派给以下专业 Agent：

{逐行列出: - agent_id: name — description}

使用 delegate_to_agent 工具将子任务委派给最合适的 Agent。
收到所有子 Agent 结果后，综合整理并输出最终回复。
如果需求简单到只需要你自己处理，直接回复，不必委派。
```


### 子 Agent 工具能力（v1.1 增强）

子 Agent 不再使用空 ToolRegistry，而是复用 `chat_send` 的完整工具注册逻辑，但受以下全局设置控制：

| 设置 key | 类型 | 默认值 | 说明 |
|----------|------|--------|------|
| `orchestrator.sub_agent_tools_enabled` | bool | `true` | 总开关：关闭后子 Agent 退回纯 LLM 推理 |
| `orchestrator.sub_agent_max_tools` | i64 | `0`（不限） | 注册工具数量上限，超出部分截断 |

规则：
- 总开关关闭时子 Agent 使用空 registry，退回纯 LLM 推理；`max_tools = 0` 表示不限制数量。
- 子 Agent 只能使用其自身绑定的 MCP 服务器和启用的 Skill；`disabled_tools` 仍在最后生效。
- 注册顺序与 `chat_send` 一致：MCP、搜索、wiki、记忆、文件、代码搜索、任务；不注册
  `delegate_to_agent`，子 Agent 不能继续委派。
- 超过上限时按注册顺序保留前 N 个工具，再应用 `disabled_tools`。
- 子 Agent 不开启 MCP fallback，避免模型绕过注册表上限或禁用名单。

### Workspace 集成

- Orchestrator 出现在九宫格第一张卡（`order_key = -1`），卡片带 `Orchestrator` 徽章。
- 用户像使用普通 Agent 一样与它对话，不需要额外的路由输入框。
- 如果用户知道该找谁，仍然直接点对应 Agent 卡片——Orchestrator 是可选项，不是强制入口。

### Seed 逻辑

在应用启动时（Tauri `setup`）：
1. 查询是否存在 `is_orchestrator = 1` 的 Agent。
2. 不存在则创建：`name = "Orchestrator"`, `description = "智能任务协调者——把需求交给它，它会调用合适的 Agent 完成任务"`, `is_orchestrator = 1`, `order_key = -1`。
3. `model_id` 默认为空，用户首次使用时通过 Agent 页选择模型（与普通 Agent 一致）。

## [S3] Scope

### In scope
- 数据库迁移：`agents` 表新增 `is_orchestrator` 列（默认 0）。
- 新增 `DelegateToAgentTool`（实现 `ToolExecutor`）。
- `chat_send` 中检测 orchestrator 并注册该 tool。
- 应用启动 seed orchestrator Agent。
- 前端 AgentDto 类型新增 `is_orchestrator` 字段。
- 工作区卡片显示 Orchestrator 徽章。

### Out of scope
- 多层委派（子 Agent 不能再委派）。
- 并行委派（v1 顺序执行）。

- Orchestrator 的设置页特殊配置。
- 修改现有 Agent 的编辑/删除逻辑。

## Tasks

- [x] T1: 数据库迁移：`agents` 表新增 `is_orchestrator INTEGER DEFAULT 0`（depends: none）
- [x] T2: 更新 `AgentRow` / `AgentDto` Rust + TS 类型（depends: T1）
- [x] T3: 新增 `DelegateToAgentTool` 实现 `ToolExecutor`（depends: T2）
- [x] T4: `chat_send` 中 orchestrator 注册 `delegate_to_agent`（depends: T3）
- [x] T5: Tauri setup seed orchestrator Agent（depends: T2）
- [x] T6: 前端工作区卡片 Orchestrator 徽章（depends: T2）
- [x] T7: 运行 check / test / build 验证（depends: T6）

- [x] T8: `delegate.rs` 支持读取设置并构建完整 ToolRegistry（depends: none）
- [x] T9: 设置页新增 Orchestrator 子 Agent 工具开关和数量上限（depends: T8）
- [x] T10: 运行 check / test / cargo test 验证（depends: T9）

## [S4] Acceptance

- [x] 首次启动后，九宫格出现带 Orchestrator 徽章的卡片。
- [x] 给 Orchestrator 发"帮我翻译这段英文：Hello World"后，能看到 tool_call 事件（delegate_to_agent），最终输出翻译结果。
- [x] 给 Orchestrator 发"写一个 Python 快排并解释"后，它能委派给编程类 Agent 并汇总结果。
- [x] 给 Orchestrator 发"你好"后，它直接回复，不委派。
- [x] 子 Agent 执行过程中前端显示 tool_call / tool_result（复用现有 UI）。
- [x] 普通 Agent 没有 delegate_to_agent 工具可用。
- [x] `npm run check` 0 errors；`npm test` 通过；`cargo test --lib` 通过。
- [x] `orchestrator.sub_agent_tools_enabled = false` 时，子 Agent 无工具可用。
- [x] `orchestrator.sub_agent_max_tools = 3` 时，子 Agent 最多注册 3 个工具。
- [x] 设置页可以修改上述两项配置并即时生效。
