---
feature: agent-completeness
status: designed
updated: 2026-08-28
branch: feat/agent-completeness
commits:
---

# Agent 完善设计 — 代码审计差距修复

## Report

## [S1] 审计结论

直接读代码审计（不看设计文档），发现以下问题按严重程度排序：

### 🔴 Critical（Agent 无法正常运行/核心功能断裂）

| # | 问题 | 位置 | 影响 |
|---|------|------|------|
| C1 | Compose 引擎 provider 硬编码 `OpenAiProvider` | `core/compose/mod.rs:create_provider()` | Compose 工作流不支持 responses-api / anthropic 协议 |
| C2 | 记忆系统未接入 Agent 循环 | `commands/chat.rs`（无 memory 引用） | RigAgent 没有 memory 字段；MemoryStore trait 存在但从未在 chat 路径使用 |
| C3 | `chat.rs` 工具注册代码块重复（file + task 工具各注册两次） | `commands/chat.rs` ~L250-260 | 重复注册无直接 bug 但说明代码质量问题；BM25 路由会重复索引 |
| C4 | Compose `pause` 将状态设为 `Failed("Paused at: ...")` 而非专用 Paused 状态 | `core/compose/mod.rs:pause()` | 暂停 = 失败，语义混乱；resume 靠字符串前缀匹配恢复 |
| C5 | **Skill 系统完全失效（死代码）** | `core/adk/prompt.rs:PromptBuilder` | `build_system_prompt()` 实现了 skill+memory 注入但从未被调用；`chat.rs` 直接用原始 system_prompt |
| C6 | **Skill 路由索引了但从未注入** | `core/rig/agent.rs:routed_tool_specs()` | `RouteResult.skills` 字段被完全忽略；只用了 tools 字段 |

### 🔴 Critical — Skill 接入断链详细分析

**Skill 系统现状：**

| 组件 | 状态 | 说明 |
|------|------|------|
| `agent_skills` 数据库表 | ✅ 存在 | 有 agent_id/skill_id/is_enabled 字段 |
| `skill_install/uninstall/toggle` API | ✅ 存在 | 前端可正常调用 |
| `PromptBuilder.build_system_prompt()` | ❌ 死代码 | 支持注入 skill 内容 + 项目记忆 + 全局记忆，但全代码库无调用点 |
| `chat.rs` system prompt 构建 | ❌ 绕过 | 直接用 `agent_row.system_prompt`，不查 `agent_skills` 表 |
| `routed_tool_specs()` skill 路由 | ❌ 无效 | `RouteResult { skills, tools }` 中 skills 字段被忽略 |
| `build_router()` 索引构建 | ❌ 不含 skill | 只索引 `ToolRegistry.tool_names()`，不含 skills 表 |

**后果**：用户安装/启用技能后，对实际对话**零影响**。技能市场、安装、开关全部是摆设。

### 🟠 MCP 接入状态

| 组件 | 状态 | 说明 |
|------|------|------|
| `agent_mcp_servers` 表 + 查询 | ✅ 正常 | `chat.rs` 查询并注册工具 |
| `McpRuntime.connect/list_tools/call_tool` | ✅ 正常 | stdio + http 双传输 |
| `McpToolExecutor` | ⚠️ 可用 | schema 兼容（inputSchema）；但结果序列化整个 `McpCallResult` JSON 而非提取文本 |
| `find_tool_server()` fallback | ✅ 正常 | 未注册工具自动查找 MCP 服务器 |
| 图片/Resource 类型返回 | ⚠️ 降级 | `McpContent::Image/Resource` 被序列化为 JSON，纯文本 provider 无法使用 |

### 🟠 High（Agent 功能严重不足）

| # | 问题 | 位置 | 影响 |
|---|------|------|------|
| H1 | 无 `edit_file` 工具（只有 read/write/list） | `core/adk/file_tools.rs` | Agent 无法做精准行级编辑，只能全文件覆写 |
| H2 | 无 `grep`/`glob` 代码搜索工具 | `core/adk/` 无对应文件 | Agent 无法搜索代码库内容 |
| H3 | 无 `knowledge_lookup`/`wiki_search` 工具（只有 `wiki_write`） | `core/adk/wiki_tool.rs` | Agent 可写 Wiki 但无法查询已有知识 |
| H4 | 无 `memory_search`/`memory_save` 工具 | `core/adk/` 无对应文件 | Agent 无法在对话中自主搜索/保存记忆 |
| H5 | `assess_risk` 引用 10 个不存在的工具名 | `core/adk/tool.rs:assess_risk()` | `glob`,`grep`,`lsp:diagnostics`,`edit_file`,`delete_file`,`run_command`,`http_request`,`rm_rf`,`database_drop`,`send_message` 均无实现 |
| H6 | `session_init` 前端从未调用 | `src/routes/agent/+page.svelte:handleSelectSession()` | 会话生命周期状态机是死代码；切换会话只 loadHistory |
| H7 | Compactor（摘要压缩）已实现但从未接入 Agent 循环 | `core/rig/compaction.rs` 存在但 `chat.rs`/`agent.rs` 无引用 | 长会话上下文只能简单截断，无摘要压缩 |

### 🟡 Medium（体验/可观测性缺陷）

| # | 问题 | 位置 | 影响 |
|---|------|------|------|
| M1 | `ToolCallCard` 不显示工具返回结果 | `src/lib/components/chat/ToolCallCard.svelte` | 用户看不到工具输出，只有参数和状态 |
| M2 | 工具调用的中间过程事件（`chat:stream:tool_call`）只在 console.log | `src/lib/stores/chat.svelte.ts:onToolCall` | 流式工具调用对用户不可见 |
| M3 | Agent 的 `disabled_tools` 配置字段从未使用 | `chat.rs` 构建时不过滤 `agent_row.disabled_tools` | 用户无法禁用特定工具 |

## [S2] 设计方案

### 修复 C1 — Compose Provider 统一分发

提取 `create_model_provider()` 公共函数（从 `chat_send` 的 dispatch 逻辑抽出），`chat.rs` 和 `compose/mod.rs` 共用。

```rust
// 新增：src-tauri/src/core/rig/provider/dispatch.rs
pub async fn create_model_provider(
    model_row: &ModelRow,
    provider_row: &ProviderRow,
) -> Result<Arc<dyn ModelProvider>, AppError> {
    // 统一按 provider.kind 分发 → OpenAi / ResponsesApi / Anthropic
}
```

### 修复 C2 — 记忆接入 Agent 循环

1. `RigAgent` 新增 `memory_store: Option<Arc<dyn MemoryStore>>` 字段。
2. `chat_send` 构建时传入 `MemoryService` 实例。
3. Agent `run()` 入口：`build_context(session_id, agent_id)` → 将记忆摘要注入 system prompt 尾部。
4. Agent `run()` 出口（成功完成）：`record(session_id, agent_id, exchange)` 记录本轮对话。
5. 新增 `memory_search` 工具让 Agent 主动查询历史记忆。

### 修复 C3 — 去除重复注册

删除 `chat.rs` 中第二组 file/task 工具注册代码块。

### 修复 C4 — Compose 暂停状态

`ComposeStatus` 枚举新增 `Paused(ComposeStatus)` 变体（包装暂停前状态），`pause()` 设置 `Paused`，`resume()` 从 `Paused(inner)` 恢复。

### 修复 C5-C6 — Skill 接入 Agent

1. `chat.rs` 构建系统提示时调用 `PromptBuilder.build_system_prompt()`（替代直接用 `agent_row.system_prompt`）：
   ```rust
   // 查询该 Agent 启用的 skills
   let enabled_skills: Vec<String> = sqlx::query_scalar(
       "SELECT skill_id FROM agent_skills WHERE agent_id = ? AND is_enabled = 1"
   ).bind(&agent_id).fetch_all(&pool).await?;
   
   let builder = PromptBuilder::new(state.db.clone());
   let system_prompt = builder.build_system_prompt(&agent_dto, &session_id, &enabled_skills).await?;
   ```
2. `routed_tool_specs()` 利用 `RouteResult.skills` 匹配已启用技能，动态注入相关技能内容到本轮上下文。

### 修复 MCP — 结果提取

`McpToolExecutor.execute()` 改为提取 `McpContent::Text` 的 text 字段作为输出（而非序列化整个 JSON）；Image/Resource 转为占位文本提示。

### 修复 H1-H4 — 新增缺失工具

| 工具 | 文件 | 风险级别 |
|------|------|----------|
| `edit_file`（行级编辑：old_string/new_string 替换） | `file_tools.rs` 追加 | Medium |
| `grep`（正则搜索文件内容，复用 `rg` 或 Rust 正则） | 新建 `search_tools.rs` | Low |
| `glob`（文件名模式匹配遍历） | 新建 `search_tools.rs` | Low |
| `wiki_search`（查询知识库） | `wiki_tool.rs` 追加 | Low |
| `memory_search`（搜索历史记忆） | 新建 `memory_tools.rs` | Low |
| `memory_save`（保存记忆条目） | 新建 `memory_tools.rs` | Medium |

### 修复 H5 — 同步 assess_risk

`assess_risk()` 补齐新增工具的风险级别映射：
- `edit_file` → Medium
- `grep`/`glob`/`wiki_search`/`memory_search` → Low
- `memory_save` → Medium

不实现的工具名（`delete_file`,`run_command`,`http_request`,`rm_rf`,`database_drop`,`send_message`）从匹配中移除，落入默认 `High`。

### 修复 H6 — 前端调用 session_init

`handleSelectSession()` 切换会话时调用 `sessionLifecycleApi.init(sessionId)`，初始化失败显示徽标提示。

### 修复 H7 — Compactor 接入 Agent 循环

`RigAgent.run()` 在每次模型调用前检查 `estimate_tokens > trigger_tokens`，触发时调用 `Compactor.compact()` 替换历史。从设置页读取 `compaction.strategy` / `compaction.trigger_tokens`。

### 修复 M1 — ToolCallCard 显示结果

`chat.rs` 工具执行完成后 emit `chat:stream:tool_result` 事件（含 call_id + output 摘要）；前端 ToolCallCard 展开时显示返回结果。

### 修复 M2 — 流式工具调用 UI 可见

`chat.svelte.ts` 的 `onToolCall` 回调不再只 console.log，而是将工具调用卡片插入消息流占位。

### 修复 M3 — disabled_tools 过滤

`chat.rs` 构建 `ToolRegistry` 后，按 `agent_row.disabled_tools`（JSON 数组）过滤已注册工具。

## [S3] Scope

- In scope:
  - C1-C4：Critical 修复
  - H1-H7：High 缺失功能补齐
  - M1-M3：Medium 体验改进
- Out of scope:
  - 多 Agent 并行编排（Phase 5 §27）
  - 沙箱/预算系统（Phase 5 §22-23）
  - 工作流引擎重构（Phase 5 §25）
  - 前端监控面板（Phase 5 §26）

## Tasks

- [ ] T1: 提取公共 `create_model_provider()`，Compose 与 chat 共用（fixes C1）
- [ ] T2: RigAgent 接入 MemoryStore，chat_send 传入 MemoryService（fixes C2）
- [ ] T3: 删除 chat.rs 重复工具注册块（fixes C3）
- [ ] T4: ComposeStatus 增加 Paused 变体（fixes C4）
- [ ] T5: 新增 edit_file / grep / glob 工具（fixes H1-H2）
- [ ] T6: 新增 wiki_search / memory_search / memory_save 工具（fixes H3-H4）
- [ ] T7: assess_risk 同步新增工具 + 清理幽灵工具名（fixes H5）
- [ ] T8: 前端 handleSelectSession 调用 session_init（fixes H6）
- [ ] T9: Compactor 接入 Agent run 循环 + 设置项（fixes H7）
- [ ] T10: ToolCallCard 显示工具返回结果 + 流式工具调用 UI（fixes M1-M2）
- [ ] T11: chat_send 按 disabled_tools 过滤工具注册（fixes M3）
- [ ] T12: cargo clippy + cargo test --lib + npm run check 全部通过
- [ ] T13: chat.rs 接入 PromptBuilder，查询 agent_skills 注入 skill 内容（fixes C5）
- [ ] T14: routed_tool_specs 利用 RouteResult.skills 动态注入技能（fixes C6）
- [ ] T15: McpToolExecutor 提取 text 内容而非序列化整个 JSON（fixes MCP 结果提取）

## [S4] Acceptance

- [ ] Compose 工作流使用 responses-api / anthropic 模型可正常生成
- [ ] Agent 对话自动注入记忆上下文，完成后自动记录
- [ ] Agent 可调用 edit_file / grep / glob / wiki_search / memory_search / memory_save
- [ ] assess_risk 与实际注册工具一一对应
- [ ] 切换会话时前端调用 session_init 并显示初始化状态
- [ ] 长会话触发 Compactor 摘要压缩
- [ ] ToolCallCard 展开后显示工具返回结果
- [ ] 流式工具调用在消息流中实时可见
- [ ] agent.disabled_tools 配置生效
- [ ] cargo clippy -D warnings 零错误
- [ ] cargo test --lib 全部通过
- [ ] 安装并启用的 Skill 在对话中生效（SKILL.md 内容注入 system prompt）
- [ ] agent_skills 表中 is_enabled=1 的技能影响实际对话
- [ ] MCP 工具返回文本内容而非包装 JSON
- [ ] 禁用 Skill 后对话不再注入该技能内容
