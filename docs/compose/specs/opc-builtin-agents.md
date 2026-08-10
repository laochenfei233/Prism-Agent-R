---
feature: opc-builtin-agents
status: delivered
updated: 2026-08-10
branch: feat/opc-builtin-agents
commits: 324e8d4..<head-sha>
---

# 内置 OPC Agent

## Report

**What was built** — 在 `AgentService` 中新增 `ensure_builtin_agents()`：懒加载插入 8 个内置 OPC Agent（短视频脚本师、文案优化师、品牌定位顾问、竞品分析师、合同审查员、法务顾问、税务计算器、周报月报生成器），按名称去重幂等，不覆盖同名用户 Agent；幂等 ALTER 为 `agents` 表补 `source` 列（默认 `'user'`，内置记录 `'builtin'`）。`agent_list` 与 `dashboard_overview` 命令在返回前触发种子，空库首次打开即见 8 个内置 Agent。前端删除硬编码创建默认「助手」的逻辑（`+page.svelte` 的 `createAgent` 改为跳转 `/agent`，`settings/+page.svelte` 删除按钮、函数、`.btn-green` CSS 与未用 import）。

**Verification** — `cargo check` PASS；`cargo test --lib` 127 passed / 0 failed（含新增 2 个 agent_service 测试：seeds 8、幂等且保留用户 Agent）；`npm run check`（svelte-check）0 errors / 0 warnings。已知 PRE-EXISTING：`src/core/adk/instructions.rs:190` unused import `std::fs`（非本次改动）。

**Journey log** —
- 种子触发点最初只考虑 `agent_list`，审查 dashboard 数据流时发现 dashboard 首页首次加载走 `dashboard_overview` 而非 `agent_list`，需双触发点才能保证首页立即可见，已补。
- 参考 prism-agent 的 builtinAgentsSeeder 是「表非空即跳过」全量播种；本项目按名称去重更符合「已有库也能补齐」的需求，且不覆盖用户同名 Agent。
- Reviewer 提示并发首调种子存在 SELECT→INSERT 竞态，与既有 `workflow_service` 模式一致，严重度低，未引入事务。

## [S1] Problem

当前项目首次使用时仅由前端创建单个默认「助手」（`+page.svelte:27`、`settings/+page.svelte:309`），没有覆盖一人公司（OPC）常见工作场景的预设 Agent。参考项目 prism-agent 内置 35 个 OPC Agent（内容/品牌/法务/财务/HR/研究/开发），本项目缺失该开箱即用能力。

## [S2] Design

### 内置 Agent 列表（8 个，参考 prism-agent builtinAgentsSeeder）

| 名称 | 描述 | system_prompt 要点 |
|------|------|-------------------|
| 短视频脚本师 | 专精于口播、剧情、Vlog 等类型的短视频脚本创作 | 熟悉抖音/B站/小红书调性；黄金3秒钩子；节奏控制；输出格式：标题\|时长\|画面\|口播\|BGM\|注意 |
| 文案优化师 | 精通社交媒体文案优化，提升点击率和互动率 | 优化标题/简介/评论；平台风格适配；SEO 关键词；输出：原文\|优化后\|改动说明 |
| 品牌定位顾问 | 梳理品牌核心价值、差异化定位，输出品牌手册 | STP 框架与品牌金字塔；竞品策略分析；输出品牌手册 |
| 竞品分析师 | 系统分析竞品内容策略、受众画像、变现模式 | 直接/间接竞品识别；SWOT 对比；差异化机会 |
| 合同审查员 | 审查合同条款，标注风险点和不合理条款 | 识别风险条款；标注模糊表达；修改建议与谈判话术；关注知识产权/竞业/保密 |
| 法务顾问 | 解答著作权、肖像权、商标、合同等法律知识 | 知识产权、合同法/劳动法/广告法；风险防范建议；通俗解释 |
| 税务计算器 | 估算个税、增值税、企业所得税 | 解释税种规则；按收入估算税额；小微税收优惠；纳税主体对比 |
| 周报月报生成器 | 基于工作记录自动生成周报/月报 | 提取关键工作项；结构化汇报；量化成果；适配汇报对象 |

- 字段映射：`name` / `description` / `system_prompt` / `avatar`（本项目 `AgentCard` 将 avatar 作为图片 URL，内置 Agent 的 avatar 置 NULL，前端回退显示名称首字符）。
- `order_key` 递增（1..8），保证 dashboard 排序稳定。

### 懒加载种子（后端）

- `AgentService::ensure_builtin_agents()`：仿 `WorkflowService::ensure_builtin_workflows` 模式，但按**名称去重**（非"表为空才插入"）——已有库（含默认「助手」）也能补齐 8 个内置 Agent，幂等。
- 幂等 ALTER 增加 `agents.source` 列（默认 `'user'`），内置记录写入 `source='builtin'`，对齐 workflows 表的既有惯例；用户删除内置 Agent 后下次 `agent_list` 会按名称重新补齐（与参考项目 builtinAgentsSeeder 行为一致）。
- 触发点：`agent_list` 与 `dashboard_overview` 命令均调用 `ensure_builtin_agents()`（dashboard 首次加载走 overview 而非 list，双触发点保证首页立即可见）。

### 前端删除默认创建逻辑

- `+page.svelte`：删除 `createAgent` 函数（硬编码创建「助手」）；`AgentLauncher` 的「新建」按钮改指向 `/agent` 页（用户手动输入名称创建自定义 Agent）。
- `settings/+page.svelte`：删除 `createAgent` 函数与「创建默认 Agent」按钮、`.btn-green` CSS、未用 `agentApi` import。
- `agent/+page.svelte`：保留输入名称手动创建（用户自定义 Agent 的正常入口，非默认逻辑）。

## [S3] Out of Scope

- 不移植全部 35 个 OPC Agent（本次仅 8 个核心角色）。
- 不接入 autoagents/工作流引擎的 Agent 角色绑定（`WorkflowStageV2.agent_id`），仅提供可对话的预设 Agent。
- 不新增前端内置 Agent 管理 UI（删除/编辑沿用现有 Agent 列表）。

## Tasks
- [x] T1: AgentService 增加 `ensure_builtin_agents()`（8 个内置 OPC Agent，名称去重幂等插入，source 列幂等 ALTER） — 验收：空库/已有库调用后 agents 表含 8 个内置记录且二次调用不重复 (covers: S2)
- [x] T2: `agent_list` 与 `dashboard_overview` 命令接入 `ensure_builtin_agents()` — 验收：首次调用返回 8 个内置 Agent (covers: S2)
- [x] T3: 前端删除默认「助手」创建逻辑（`+page.svelte`、`settings/+page.svelte`） — 验收：两处不再硬编码创建「助手」，AgentLauncher 新建按钮跳转 /agent (covers: S2)
- [x] T4: 验证 — 验收：`cargo check` 与 `svelte-check`/前端构建通过 (covers: S2)
