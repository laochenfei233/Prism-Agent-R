# Prism Agent R — 设计文档变更日志

记录设计文档的重大结构变更，避免后续 agent 重复报告已修复的问题。

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
