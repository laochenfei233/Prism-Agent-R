# Prism Agent R — 设计文档变更日志

记录设计文档的重大结构变更，避免后续 agent 重复报告已修复的问题。

## 2026-08-08

### Phase 4 新增（查漏补缺驱动）

学习 4 个参考仓库（deep-search-pro / intelligent-kb-rag / awesome-harness-engineering / learn-harness-engineering）后对 phase1-3 文档与代码做差距审计，新增：

- `docs/design/gap-audit.md` — 差距审计报告（文档-代码偏差 + 参考仓库映射）
- `docs/design/phase4-agentic.md` — Phase 4 设计提案（§15 网络搜索工具链 / §16 RAG 检索增强 / §17 Harness 工程化 / §18 前端 UI 设计 / §19 Agent 设计参考 / §20-21 迁移命令补记 + 任务清单）
- `docs/design/README.md` — 章节矩阵与速查表补 Phase 4 行（图例加 🟪）
- 总索引 `prism-agent-r.md` — 导航表补 phase4 两文件；迁移总表补 022（漏登记）/ 023（web_search_cache）/ 024（trace_grading）；[S5] 补三项 Phase 4 功能

**审计发现的文档-代码偏差**（详见 `gap-audit.md` §2）：
- 🔴 预置「深度研究」工作流声明的 `web_search`/`knowledge_lookup` 工具未实现（ToolRegistry 默认空、chat/workflow 仅注册 MCP 工具）→ phase4 §15 落地
- 🟠 迁移总表漏登记 `022_meeting_transcript_upsert`（文件存在）→ 已补

### 修订（同日）

- `phase4-gap-audit.md` 重命名为 `gap-audit.md`（与 phase 文档命名解耦），10 处引用同步更新
- `phase4-agentic.md` 新增 **§18 前端 UI 设计**（初版编号 §17A，后重编号）：参考 cherry-studio 的 `DESIGN.md` 与 AppShell/Sidebar/settings 布局实现，提炼布局骨架（三区 AppShell、两栏设置、PageHeader、状态徽标、内容 max-w-3xl 约束），逐视图映射 Phase 4 各功能（搜索设置 §18.2 / RAG 配置面板 §18.3 / 会话状态徽标 §18.4 / Loop 自动化设计区 §18.5 / 轨迹回放 §18.6）+ 设计约束清单 §18.7；任务清单 covers 同步补 §18 引用
- §18 评审修复：token 名对齐**实际实现**（tokens.css 的 `--color-bg*`/`--color-green`/`--color-orange`/`--color-red`/`--color-accent` 等，而非 phase1 §9.1 文档语义别名）；设置页布局修正为三栏（§9.5.3 实为三栏）；cherry 实现路径修正（`components/SettingsPrimitives.tsx`）；「TaskInput」误标修正为任务表单字段
- 章节重编号：§17A（UI 设计）→ **§18**，原 §18（迁移命令补记）→ **§19**，原 §19（任务清单）→ **§20**；参考来源/适配原则移至文件头部（编辑约定：后续新增章节序号 +1 时元信息增量更新到头部）
- 新增 **§19 Agent 设计参考**（Anthropic & OpenAI 2026 推荐）：调研 Anthropic News 2026（91 篇中精读 10 篇 Claude Code 生态）与 OpenAI News 2026（RSS 330 篇中精读 10 篇 Codex/harness 生态），19.1/19.2 提炼设计要点 + 本项目映射表；原 §19 迁移→**§20**、原 §20 任务→**§21**；头部参考来源增量更新（编辑约定实证）
- **§19.3 升级为增量设计文稿（用户指示「直接变成设计文稿的内容，而不是纯粹的文章」）**：19.3.1 compaction 语义压缩（§13.1 增强）/ 19.3.2 会话三原语 Item/Turn/Thread + fork / 19.3.3 双向审批 session:approve（暂停 turn）/ 19.3.4 Auto-review 审批子代理 / 19.3.5 轨迹级监控 PauseAndConfirm / 19.3.6 指令渐进披露（docs/AGENTS/ 目录化 + CI 校验）/ 19.3.7 评测捆绑体 harness_meta 落库 / 19.3.8 评测用例质量审计 CaseAuditor —— 每项含问题/接口签名/配置/流程/验收；迁移补 **025_eval_harness_meta**；命令补 session:fork/approve + rag_eval_audit_*；事件补 item/turn/trajectory-alert；任务补 **P4-T12~T19**（covers 19.3.1-19.3.8）；总索引迁移总表与导航、README 矩阵同步

## 2026-08-05

### 文档拆分（单文件 → 4 文件）

原单文件 `docs/compose/specs/prism-agent-r.md`（6253 行）按开发阶段拆分为：

- `docs/compose/specs/prism-agent-r.md` — 总索引（frontmatter / S0 设计模式参考 / S1 问题 / 架构 / 选型 / S3 / S4 错误矩阵 / S5 功能建议 / MVP / Tasks / 完成报告）
- `docs/design/phase1-core.md` — Phase 1（Agent 核心闭环）
- `docs/design/phase2-panel.md` — Phase 2（面板功能）
- `docs/design/phase3-extend.md` — Phase 3（扩展功能）
- `docs/design/README.md` — 章节→文件→阶段矩阵 + 阅读顺序

### 一致性修复轮（子代理审查）

- **§13.1 上下文压缩重复**：phase1/phase3 各有一份 → phase1 改指针，phase3 为唯一归属
- **迁移编号体系**：005 双号 → 重编号 001-013；新增迁移编号总表（索引）
- **迁移「并入 009」矛盾**：sessions_fts→012、translate_fts→013（遵守 §14.3#28 版本号 bump）
- **task_runs 无 DDL**：复用 workflow_runs（补 source 列）
- **IPC 命令双所有权**：§8.2 回填 task:* / session:inject-file / lsp:detect / fs:watch
- **covers 锚点失效**：27 处 `S2-§` → `文件 §N` 格式
- **跨文件引用**：~35 处补「见 xxx.md」
- **PRAGMA 同步**：§5.6 与 §5.7.1 统一为 8 条

### 阶段内容调整

- **消息搜索阶段矛盾修复**：从 🟩 Phase 3 移回 🟦 Phase 1（§5.7.2，对话闭环即用）
- **市场搜索移入 phase2**：§10.4.1-10.4.4 从 phase1 移出（留指针）
- **§10.6/§10.7 子节加阶段徽标**（🟧🟩）

### 新增内容

- **[S4] 各模块错误处理矩阵**：10 个模块的错误→检测→处理→反馈
- **[S5] 功能建议**：14 项后续迭代候选（云端同步/导出/用量预警等）
- **数据存储按阶段拆回**：PRAGMA/索引/分页/保留策略回 phase1（横切关注点）
- **14 项功能建议细化到所属模块**：每项含定位/实现方案/安全考虑/复用关系 + 「可能错误+处理方法」表（§5.8、§1.1、§1.2、§9.1.1、§9.5.1、§9.8.1、§9.8.2、§10.4.5、§10.6.4.1、§10.8.1、§14.5.1、§9.10.1、§10.2.1、§10.3.9）
- **新增迁移 014-016**：014_session_archive / 015_prompt_templates / 016_workflow_versions（已在索引迁移总表登记，编号递增至 016）
