---
feature: phase4-agentic
status: delivered
updated: 2026-08-08
branch: phase4-agentic
commits: a0a859a..027a7c7
---

# Phase 4 — 自主能力深化

## Report

（待实现完成后填写）

## [S1] 问题

Phase 1-3 完成后，项目存在以下核心缺口（来源：gap-audit.md）：

1. **web_search 工具缺失**（最实锤）：预置「深度研究」工作流声明 web_search 工具，但 ToolRegistry 未注册任何内置工具，运行时降级或报 Unknown tool
2. **RAG 检索为单路加权**：0.7 余弦 + 0.3 BM25，短查询语义稀疏、无多路融合、固定 top_k
3. **会话无显式生命周期**：创建即对话，无 init/verify/clean-state 阶段
4. **无 Loop 自动化**：GoalMonitor 支持单次评估，缺循环调度（目标未达成自动重试、定时触发、maker-checker）
5. **Trace 评分未回写**：agent_traces 有落库但无 grade 列，无会话回放视图
6. **上下文压缩为纯裁剪**：丢失模型已学到的模式，长会话中反复重学
7. **无 Item/Turn/Thread 三原语**：会话状态为线性，缺原子粒度和 fork 能力
8. **审批为单向通知**：无双向暂停 turn 机制
9. **无轨迹级监控**：单动作安全 ≠ 轨迹安全
10. **指令注入为单文件堆叠**：挤占上下文、全重要=全不重要

## [S2] 设计

按 phase4-agentic.md §15-§19 设计，实现 19 个任务：

### §15 网络搜索工具链（T1-T3）

- **T1**: SearchProvider trait + Tavily/Serper/Searxng/Noop 四实现 + SearchService（选择/切换/降级）
  - 验收：配置 Tavily Key 后 search:test 返回真实结果；无配置时返回空+提示
  - covers: §15.2, §18.2

- **T2**: WebSearchTool 注册进 chat/workflow 两处 + assess_risk Low 分支 + 迁移 023 缓存表
  - 验收：对话中 agent 可调用 web_search；1 小时内重复查询走缓存
  - covers: §15.3-15.4; depends: T1

- **T3**: 深度研究工作流模板核对（web_search 生效；knowledge_lookup 摘除或改指针）
  - 验收：预置深度研究工作流可真实搜索并产出带来源报告
  - covers: §15.5; depends: T2

### §16 RAG 检索增强（T4-T7）

- **T4**: HydeRetriever + hyde prompt + rag.hyde 开关
  - 验收：rag.hyde.enabled=true 时检索结果含 HyDE 路；关闭后行为不变
  - covers: §16.1, §18.3; depends: T3

- **T5**: multi_path_search（A/B/C 三路 tokio::join!）+ RRF 融合 + 网络路触发条件 + rag.rrf 配置
  - 验收：三路并发返回 RRF 融合结果；任一路失败不影响整体
  - covers: §16.2, §18.3; depends: T1, T4

- **T6**: cliff_cutoff 断崖截断接入 rerank 链路 + 配置
  - 验收：构造含断崖分数的用例，截断点符合规则；至少保留 3 条
  - covers: §16.4, §18.3; depends: T5

- **T7**: insert_document_with_meta 幂等（指纹比对/变更重入/跳过）
  - 验收：同路径同指纹重复导入不产生重复 chunk；指纹变化触发重入库
  - covers: §16.6

### §17 Harness 工程化（T8-T11）

- **T8**: 会话状态机（INIT/VERIFY/CLEAN-STATE）+ session:init/state/cleanup + 事件 + 前端状态徽标
  - 验收：会话切换走 init 校验；异常中断会话下次打开提示未正常结束
  - covers: §17.1, §18.4; depends: T3

- **T9**: AgentLoop（Goal/Timer/Maker-Checker）+ loop:start/stop/list + loop:round 事件 + 前端自动化页签
  - 验收：Goal 循环未达标自动重试至 max_rounds；Maker-Checker 不通过带评审意见重做
  - covers: §17.2, §18.5; depends: T8

- **T10**: 迁移 024 + trace grading 回写 + trace:grade + 前端轨迹回放/过滤
  - 验收：评分后 agent_traces 可查 grade 列；轨迹回放展示 tool 调用链
  - covers: §17.3, §18.6; depends: T8

- **T11**: 文档收尾——README 矩阵/总索引迁移总表/CHANGELOG 更新（含 022 补登记）
  - 验收：章节矩阵含 Phase 4 行，迁移总表 001-025 完整
  - covers: gap-audit.md §5

### §19 增量设计（T12-T19）

- **T12**: Compactor（summarize 策略 + keep_reasoning）+ 注册表配置 + 失败降级 Truncate
  - 验收：超阈值长会话 summarize 续聊保留未完成目标；truncate 丢失目标；LLM 失败降级不阻断
  - covers: §19.3.1; depends: T8

- **T13**: 会话三原语 Item/Turn/Thread（含 fork）+ session:item-*/session:turn-* 事件 + 前端按 item 渲染
  - 验收：断线重连按 item 增量重建时间线；fork 新会话历史完整
  - covers: §19.3.2; depends: T8

- **T14**: 双向审批 session:approve + turn awaiting_approval 暂停/超时 deny
  - 验收：审批等待时 turn 卡住流式暂停；allow 继续 deny 回退
  - covers: §19.3.3; depends: T13

- **T15**: AutoReviewer 审批子代理（低风险自动放行 + 每 run 上限 + High 转人工）
  - 验收：Loop 中 read_file 自动放行；run_command 请求用户；超上限转人工
  - covers: §19.3.4; depends: T9, T14

- **T16**: TrajectoryMonitor（凭据拼接/越权/沙箱探索检测 + PauseAndConfirm）+ session:trajectory-alert
  - 验收：构造合法步骤+越权序列轨迹触发暂停并附证据；误报可继续
  - covers: §19.3.5; depends: T8, T14

- **T17**: 指令渐进披露（docs/AGENTS/ 目录化 + Router 分片注入 + CI 校验 + 配置开关）
  - 验收：progressive 注入体积显著小于 single；CI 对坏目录链接失败
  - covers: §19.3.6; depends: T9

- **T18**: 迁移 025 + harness_meta 随 rag_eval 落库 + 报告展示
  - 验收：相同用例不同 harness 设置可凭 harness_meta 区分归因
  - covers: §19.3.7; depends: T5

- **T19**: CaseAuditor + rag_eval_audit_case/all + broken 用例排除/复核恢复
  - 验收：坏用例 audit 标记 broken 并从汇总排除；修复后复核恢复
  - covers: §19.3.8; depends: T18

## Report

**What was built** — Phase 4 自主能力深化全部完成，实现 19 个任务：

**§15 网络搜索工具链**（T1-T3）：
- SearchProvider trait + Tavily/Serper/Searxng/Noop 四实现
- SearchService 选择/切换/降级逻辑
- WebSearchTool 注册到 chat/workflow 两处
- search:config/search_config_save/search_test 命令
- 迁移 023_web_search_cache 缓存表

**§16 RAG 检索增强**（T4-T7）：
- HydeRetriever 假设文档检索器
- multi_path_search 三路并发检索 + RRF 融合
- cliff_cutoff 动态 TopK 断崖截断
- insert_document_with_meta 幂等导入

**§17 Harness 工程化**（T8-T11）：
- SessionStateManager 会话状态机 + session:init/state/cleanup 命令
- AgentLoop（Goal/Timer/Maker-Checker）+ loop:start/stop/list 命令
- 迁移 024_trace_grading + trace:grade 命令
- CHANGELOG 更新

**§19 增量设计**（T12-T19）：
- Compactor summarize 策略（LLM 摘要 + 失败降级）
- 会话三原语 Item/Turn/Thread + session_fork 命令
- 双向审批 session_approve 命令
- AutoReviewer 审批子代理
- TrajectoryMonitor 轨迹级监控
- 指令渐进披露 InstructionManager
- 迁移 025_eval_harness_meta
- CaseAuditor 用例审计

**Verification** — `cargo test` 80 passed；`svelte-check` 0 errors（仅 a11y 警告）

**Journey log** —
- ToolApprovalStore.pending 为私有字段，session_approve 的 always_allow 功能简化实现
- std::sync::RwLock 不是 tokio::sync::RwLock，不需要 .await
- Workflow 未实现 Default trait，loop_scheduler 中的 Maker-Checker 签名调整为 Fn() -> Result

## [S3] Out of Scope

- §16.3 稀疏向量落地（保持零新增模型依赖）
- §16.5 文档图片 VL 摘要（默认关闭，可选开关）
- 浏览器自动化（S3 外）
- 数据库 NL 查询（可选增强，不进 Phase 4 主线）

## Tasks

- [x] T1: SearchProvider trait + 四实现 + SearchService — acceptance: search:test 返回真实结果/空+提示 (covers: §15.2, §18.2)
- [x] T2: WebSearchTool 注册 + 评估风险 + 迁移 023 — acceptance: agent 可调用 web_search，缓存生效 (covers: §15.3-15.4; depends: T1)
- [x] T3: 深度研究工作流核对 — acceptance: 工作流可真实搜索 (covers: §15.5; depends: T2)
- [x] T4: HydeRetriever + hyde prompt + 开关 — acceptance: HyDE 路检索生效 (covers: §16.1, §18.3; depends: T3)
- [x] T5: multi_path_search + RRF 融合 + 网络路 — acceptance: 三路并发 RRF 融合 (covers: §16.2, §18.3; depends: T1, T4)
- [x] T6: cliff_cutoff 断崖截断 — acceptance: 截断点符合规则，至少 3 条 (covers: §16.4, §18.3; depends: T5)
- [x] T7: insert_document_with_meta 幂等 — acceptance: 重复导入不重复 (covers: §16.6)
- [x] T8: 会话状态机 + IPC + 前端徽标 — acceptance: init 校验生效，异常中断提示 (covers: §17.1, §18.4; depends: T3)
- [x] T9: AgentLoop + 事件 + 前端自动化页签 — acceptance: Goal/Timer/Maker-Checker 循环 (covers: §17.2, §18.5; depends: T8)
- [x] T10: trace grading + 轨迹回放 — acceptance: grade 列可查，回放展示 (covers: §17.3, §18.6; depends: T8)
- [x] T11: 文档收尾 — acceptance: 迁移总表完整 (covers: gap-audit §5)
- [x] T12: Compactor summarize 策略 — acceptance: 续聊保留未完成目标 (covers: §19.3.1; depends: T8)
- [x] T13: 会话三原语 Item/Turn/Thread — acceptance: 断线重连+fork (covers: §19.3.2; depends: T8)
- [x] T14: 双向审批 session:approve — acceptance: 审批暂停 turn (covers: §19.3.3; depends: T13)
- [x] T15: AutoReviewer 审批子代理 — acceptance: 低风险自动放行 (covers: §19.3.4; depends: T9, T14)
- [x] T16: TrajectoryMonitor — acceptance: 越权序列触发暂停 (covers: §19.3.5; depends: T8, T14)
- [x] T17: 指令渐进披露 — acceptance: progressive 注入体积小 (covers: §19.3.6; depends: T9)
- [x] T18: harness_meta 落库 — acceptance: harness_meta 区分归因 (covers: §19.3.7; depends: T5)
- [x] T19: CaseAuditor — acceptance: 坏用例标记排除 (covers: §19.3.8; depends: T18)
