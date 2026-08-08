# Prism Agent R — Phase 4 查漏补缺审计报告

> **用途**：记录对 `docs/design`（phase1-3）与整个项目代码的差距审计结论——文档-代码偏差、参考仓库可借鉴项、Phase 4 设计范围。
> **审计日期**：2026-08-08
> **参考仓库**：[waseens/deep-search-pro](https://github.com/waseens/deep-search-pro) · [waseens/intelligent-kb-rag](https://github.com/waseens/intelligent-kb-rag) · [walkinglabs/awesome-harness-engineering](https://github.com/walkinglabs/awesome-harness-engineering) · [walkinglabs/learn-harness-engineering](https://github.com/walkinglabs/learn-harness-engineering)
> **配套文档**：[`phase4-agentic.md`](./phase4-agentic.md)（Phase 4 设计提案，只含设计）

---

## 1. 审计方法

1. 浅克隆 4 个参考仓库，通读 README 与关键实现（deep-search-pro 的 orchestrator/工具/路径安全；intelligent-kb-rag 的 RAG 流程/HyDE/RRF/双向量/幂等导入；awesome-harness 的分类清单；learn-harness 的 14 讲 8 项目课程）。
2. 对照 `docs/design/` 三份 phase 文档（章节→文件→阶段矩阵、各功能详细设计、Phase 3 实现状态附录）。
3. 核对 `src-tauri/src/` 实际代码：迁移文件、IPC 命令、ToolRegistry、RAG 检索链路、工作流引擎。

## 2. 文档-代码偏差（设计写了，代码没有 / 不一致）

### 2.1 🔴 `web_search` / `knowledge_lookup` 工具缺失（最实锤）

- **文档声明**：`phase1-core.md` §10.6.2 预置「深度研究」工作流 `stage1` 的 `tools: ["web_search", "knowledge_lookup", "read_file"]`；`workflow_service.rs:171` 的 `deep_research_workflow` 同样声明 `tools: vec!["web_search", "read_file"]`。
- **代码事实**：
  - `core/adk/tool.rs` `ToolRegistry::default()` 为空实现（line 176-179），不注册任何内置工具。
  - `commands/chat.rs:174` 只注册该 Agent 绑定的 MCP 工具。
  - `commands/workflow.rs:465` 同样只建空 `ToolRegistry::new()`。
  - `web_search` 全代码库仅 `workflow_service.rs:171` 一处字符串引用，无实现。
  - `assess_risk`（tool.rs:53）把 `http_request` 列为 High 风险，但也没有对应工具实现。
- **后果**：预置「深度研究」工作流运行时，`researcher` 阶段声明的 `web_search` 工具不可用——agent 只能降级用 `read_file` 或直接报 `Unknown tool`。设计承诺的「搜索资料」能力实际是空的。
- **Phase 4 对应**：§15 网络搜索工具链（见设计提案）。

### 2.2 🟠 迁移总表缺 022

- 迁移文件实际存在 `022_meeting_transcript_upsert.sql`，phase3 完成报告与 CHANGELOG 均提及 015-022。
- 但总索引 `prism-agent-r.md` 迁移总表只登记到 `021_project_index`（缺 022 行）。
- **修正**：迁移总表补 022 行；Phase 4 新增迁移从 023 起编号。

### 2.3 🟢 其余核对通过项（无需修正）

- §10.2.1 项目级自动索引 / §10.2.2 Contextual Retrieval / §10.2.3 文档解析 / §10.2.4 可追溯引用 / §10.2.5 多维评测——代码均有实现（`rag/` 模块 10 个文件，`015_rag_context.sql`/`021_project_index.sql` 等迁移在位）。
- §10.13 评估监控（agent_traces / AgentJudge / agent_stats）——`trace.rs`/`agent_eval.rs` 在位。
- §10.7 记忆系统三层结构——`memory.rs` 在位。
- §10.3 会议系统、§10.5 翻译/OCR、§10.9 反思、§10.11 目标、§10.12 护栏——命令文件齐全。

## 3. 参考仓库 → 本项目映射与差距

### 3.1 waseens/deep-search-pro（多智能体协作，<1000 行）

| 借鉴点 | 本项目现状 | 差距 / Phase 4 |
|--------|-----------|----------------|
| Orchestrator 主从调度（主 agent 调度 3 子 agent） | 已有 §10.6 AutoAgents 工作流引擎（Actor/Coordinator） | 无差距；可借鉴「子 agent = 轻量配置字典」的简洁度 |
| 网络搜索工具（Tavily） | **无**（仅工作流字符串引用） | 🔴 §15 网络搜索工具链 |
| WebSocket 实时进度推送 | 已有 §7 流式事件 + `chat:stream:*` | 无差距 |
| 优雅降级（无服务自动跳过） | 部分（RAG/会议有降级；web_search 无） | §15 纳入降级设计 |
| ContextVar 协程级隔离 | 桌面单进程，会话以 session_id 隔离 | 无差距（非 Web 多租户） |
| 路径安全 12 场景 | 已有 §10.8 文件安全 | 无差距 |
| 数据库 NL 查询（agent 写 SQL） | **无** | 可选增强，[S5] 级别，不进 Phase 4 主线 |

### 3.2 waseens/intelligent-kb-rag（RAG 全工程实践）

| 借鉴点 | 本项目现状 | 差距 / Phase 4 |
|--------|-----------|----------------|
| HyDE 假设文档嵌入（短查询语义稀疏） | **无**（`search.rs` 直接用 query 向量） | 🔴 §16.1 |
| RRF 多路召回融合（普通向量 + HyDE + 网络搜索） | **无**（单路 0.7cos + 0.3bm25 加权） | 🔴 §16.2（含多路并发检索） |
| BGE-M3 稠密+稀疏双向量 | 仅稠密余弦 + 字符串级 BM25 近似 | 🟠 §16.3（可选：稀疏向量落地） |
| 动态 TopK 断崖截断 | **无**（固定 `truncate(top_k)`） | 🔴 §16.4 |
| MinerU 高精度 PDF→MD（保留表格/图片结构） | 有 pdf-extract 文本层 + pdfium 视觉层 | 🟠 §16.5（可选：接入 MinerU 类解析） |
| 文档图片 VL 摘要（图→摘要→入库） | **无**（chunk 内图片引用未语义化） | 🟠 §16.5 |
| 商品名/实体归一化（LLM 提取→向量匹配） | 无（Wiki 有 entities/ 页面，非检索链路） | 🟢 可选，领域相关 |
| 幂等导入（重复导入清理旧数据） | 有转写幂等（022 迁移）；**RAG 导入幂等未验证** | 🟠 §16.6 |
| 任务进度追踪（各节点状态推送） | 已有 rag:progress 事件 | 无差距 |
| 三路并发（LangGraph 条件路由） | 工作流引擎为顺序 stage 链 | §16.2 引入多路并发检索 |

### 3.3 walkinglabs/awesome-harness-engineering（harness 精选清单）

| 借鉴点 | 本项目现状 | 差距 / Phase 4 |
|--------|-----------|----------------|
| Harness 五子系统（Instructions/State/Verification/Scope/Lifecycle） | 记忆系统 §10.7 + 评估 §10.13 + 护栏 §10.12 已覆盖大部分 | 🟠 §17 补「会话生命周期」显式化 |
| 上下文工程 / 记忆预算 | 已有 §13.1 TokenBudget + 压力等级 | 无差距 |
| Evals（trace grading、确定性 verifier） | 已有 §10.13 AgentJudge + eval_gate CI 门槛 | 🟠 §17.3 trace grading 深化 |
| 可观测性（session replay、成本追踪） | 已有 trace_list / agent_stats | 🟠 §17.3 session replay |
| Loop 工程（goal/timer/maker-checker loop） | 已有 GoalMonitor（§10.11）+ 反思（§10.9 生产者-评审者） | 🟠 §17.2 loop 自动化 |
| 沙箱 / 安全自主 | 已有 §10.12 护栏 + §10.10 HITL 审批 | 无差距 |
| Graph 工程（节点/边/共享状态/路由） | 工作流为顺序 DAG（depends_on） | 🟢 可选：并行分支/人工审批节点可视化 |

### 3.4 walkinglabs/learn-harness-engineering（课程，capstone 即桌面知识库应用）

| 借鉴点 | 本项目现状 | 差距 / Phase 4 |
|--------|-----------|----------------|
| 课程 capstone = Electron 知识库桌面应用（导入→索引→带引用的 Q&A） | 本项目 §10.1 Wiki + §10.2 RAG + §10.2.4 可追溯引用 已是同类产品 | 无差距（本项目更完整） |
| Agent 会话生命周期（init → select → execute → wrap up → 清理） | 记忆系统有 checkpoint；无显式 init/verify 阶段 | 🟠 §17.1 |
| feature_list.json 作为 harness 原语 | 目标系统 §10.11 TaskGoal | 🟢 可选对齐 |
| 多会话连续性（progress log） | 已有 §10.7 记忆 + 会话持久化 | 无差距 |
| 可观测性内建 harness（L11） | 已有 §10.13 | 无差距 |
| 评估驱动改进（ablation study） | 已有 rag_eval + eval_gate | 🟢 可选扩展 |

## 4. 结论：Phase 4 范围（三项，用户已确认）

1. **§15 网络搜索工具链**（deep-search-pro 启发，补齐文档-代码偏差 2.1）：`SearchProvider` trait（Tavily/Serper/本地等）+ `web_search` 工具注册 + 优雅降级 + 缓存；接入预置「深度研究」工作流。
2. **§16 RAG 检索增强**（intelligent-kb-rag 启发）：HyDE、RRF 多路融合（含网络搜索多路并发）、稀疏向量落地、动态 TopK 断崖截断、图片 VL 摘要、幂等导入。
3. **§17 Harness 工程化**（walkinglabs 两仓库启发）：Agent 会话生命周期显式化、loop 工程（goal/timer/maker-checker 自动化）、trace grading 与 session replay、可观测性内建。

## 5. 顺带修正项

- 总索引 `prism-agent-r.md`：迁移总表补 `022_meeting_transcript_upsert`；Phase 4 迁移 023+ 预留登记。
- `docs/design/README.md`：章节→文件→阶段矩阵补 Phase 4 行；推荐阅读顺序与速查表补 phase4 条目。
- `docs/design/CHANGELOG.md`：登记本次审计与 Phase 4 文档拆分。
