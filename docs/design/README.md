# Prism Agent R — 设计文档

跨平台（Windows/macOS/Linux）桌面 AI Agent 平台设计文档。Tauri 2.x + Rust 后端 + Svelte 5 前端。

**总索引**：[`docs/compose/specs/prism-agent-r.md`](../compose/specs/prism-agent-r.md)（S0 设计模式参考 / S1 问题 / 架构 / 选型 / MVP / Tasks / 完成报告）

---

## 📍 章节 → 文件 → 阶段 矩阵

按章节编号定位内容所在文件与开发阶段。**新任务定位：先查此表，再读对应文件。**

| § | 内容 | 文件 | 阶段 |
|---|------|------|------|
| S0 | 设计模式参考（Agentic Patterns / Cherry Studio / Compose-Next） | 索引 | — |
| 15 | 网络搜索工具链（SearchProvider / web_search / 缓存 023） | phase4 | 🟪 |
| 16 | RAG 检索增强（HyDE / RRF 多路融合 / 断崖截断 / 幂等导入） | phase4 | 🟪 |
| 17 | Harness 工程化（会话生命周期 / Loop / Trace Grading 024） | phase4 | 🟪 |
| 18 | Phase 4 前端 UI 设计（排版布局参考 Cherry Studio） | phase4 | 🟪 |
| 19 | Agent 设计参考（Anthropic & OpenAI 2026 推荐 + 增量设计 8 项） | phase4 | 🟪 |
| 20-21 | Phase 4 迁移与命令补记（023-025）/ 任务清单（T1-T19） | phase4 | 🟪 |
| S1 | 问题定义 | 索引 | — |
| S2 / 1-2 | 架构总览 / 技术选型 | 索引 | — |
| 3 | Rust 后端三层架构（ADK/Rig/AutoAgents） | phase1 | 🟦 |
| 4 | 完整目录结构 | phase1 | 🟦 |
| 5.1-5.6 | 数据库 Schema / DDL / sqlx | phase1 | 🟦 |
| 5.7.1-5.7.3, 5.7.6-5.7.8 | 数据存储跨阶段基础（PRAGMA/消息FTS/分页/保留/索引/边界） | phase1 | 🟦 |
| 5.7.4 | 会话标题搜索（迁移 012） | phase2 | 🟧 |
| 5.7.5 | 翻译历史搜索（迁移 013） | phase3 | 🟩 |
| 6 | MCP 协议 | phase1 | 🟦(+🟧) |
| 7 | 流式响应 | phase1 | 🟦 |
| 8 | Tauri IPC 命令/事件 | phase1 | 🟦(+🟧🟩) |
| 9.1-9.8 | 前端基础（设计系统/组件/对话） | phase1 | 🟦 |
| 9.9 | 主页面板 | phase2 | 🟧 |
| 9.10 | Agent 侧边栏（六 Tab） | phase2 | 🟧 |
| 10.1-10.3 | Wiki / RAG / 会议 | phase3 | 🟩 |
| 10.4 | Skill 技能系统 | phase1 | 🟦(+🟧🟩) |
| 10.5 | 翻译 / OCR | phase3 | 🟩 |
| 10.6 | 多 Agent 工作流引擎 + 模板 | phase1 | 🟦(+🟧) |
| 10.7 | 记忆系统 | phase1 | 🟦(+🟧🟩) |
| 10.8 | 文件与附件 | phase1 | 🟦 |
| 10.9 | 反思模式 | phase3 | 🟩 |
| 10.10 | 人机协同（工具审批） | phase2 | 🟧 |
| 10.11 | 目标设定与监控 | phase3 | 🟩 |
| 10.12 | 安全护栏 | phase3 | 🟩 |
| 10.13 | 评估与监控 | phase3 | 🟩 |
| 11 | 错误处理与日志 | phase1 | 🟦 |
| 11A | 无障碍设计 | phase3 | 🟩 |
| 12 | 安全设计 | phase1 | 🟦(+🟩) |
| 13 | 性能设计 | phase1（基线）/ phase3（§13.1 压缩） | 🟦🟩 |
| 14 | 旧版 prism-agent 经验与规避（51 条） | phase1 | 🟦 |

> **图例**：🟦 Phase 1（Agent 核心闭环）· 🟧 Phase 2（面板）· 🟩 Phase 3（扩展）· 🟪 Phase 4（自主能力深化）· (+🟧🟩) = 章节含后续阶段子节

---

## 📖 推荐阅读顺序

| 场景 | 读什么 |
|------|--------|
| **新对话 / 新 agent 起步** | 总索引（S0/S1/§1/§2 + MVP 清单 + Tasks）→ 按任务查上表 → 读对应 phase 文件 |
| **做 Phase 1 任务**（对话闭环） | `phase1-core.md` 全量 |
| **做 Phase 2 任务**（面板/侧边栏/审批） | `phase2-panel.md` + 依赖基础（§5/§8/§10.6/§10.7 回查 phase1） |
| **做 Phase 3 任务**（扩展功能） | `phase3-extend.md` + 依赖基础（§3/§5/§7/§8 回查 phase1） |
| **做 Phase 4 任务**（搜索/检索增强/Harness） | `phase4-agentic.md` + 差距审计 `gap-audit.md` + 依赖基础（§10.2 回查 phase3、§10.6 回查 phase1） |
| **数据库 / 迁移相关** | 迁移总表（见总索引）+ phase1 §5（完整 Schema）+ 各阶段 FTS 补充 |
| **排查旧版教训** | phase1 §14（51 条规避，跨阶段对照） |

### 🎯 常见任务速查

| 任务 | 读哪个文件（章节） |
|------|-------------------|
| 做消息全文搜索 | phase1 §5.7.2（messages_fts）+ phase2 §5.7.4（会话标题） |
| 新增 MCP 传输 | phase1 §6（McpTransport Trait/stdio/http + SSE） |
| 加一个 IPC 命令 | phase1 §8.2（命令总表）+ 对应域服务（§5/§10） |
| 改 Agent 对话流 | phase1 §7（流式）+ §8（chat 域）+ §9.5-9.8（前端） |
| 做多 Agent 工作流 | phase1 §10.6（引擎）+ phase2 §9.9.1（任务设计区） |
| 做记忆/checkpoint | phase1 §10.7（完整）+ phase3 §13.1（压缩） |
| 做技能/市场 | phase1 §10.4（主体）+ phase2 §10.4.1-10.4.4（市场搜索） |
| 做会议/ASR | phase3 §10.3（8 后端）+ phase1 §5.4（003 迁移） |
| 做翻译/OCR | phase3 §10.5 + phase1 §5.5（translate_history） |
| 做 Wiki/RAG | phase3 §10.1-10.2 + phase1 §5.3（002 迁移） |
| 做工具审批（HITL） | phase2 §10.10 + phase1 §8.3（tool:approval 事件） |
| 加安全护栏 | phase3 §10.12 + phase1 §12（安全设计） |
| 做上下文压缩 | phase3 §13.1（TokenBudget 统一配置） |
| 做网络搜索 / web_search 工具 | phase4 §15（SearchProvider/缓存/降级） |
| 做 RAG 检索增强（HyDE/RRF/断崖截断） | phase4 §16 + phase3 §10.2（基础） |
| 做会话生命周期 / Loop 自动化 | phase4 §17.1-17.2 + phase1 §10.6（引擎）+ phase2 §10.10（审批） |
| 做轨迹评分 / 回放 | phase4 §17.3 + phase3 §10.13（AgentJudge/trace） |
| 查历史错误教训 | phase1 §14（51 条规避） |

---

## ⚠️ 关键设计约束（跨文件通用）

1. **迁移编号必须递增**（001-016），禁止在已应用迁移上追加（§14.3 #28）。
2. **数据存储是横切关注点**：建表即建索引/PRAGMA，新增表须同步 §5.7.7 关键索引。
3. **每个跨文件引用的章节只在一处有完整内容**，其余放「见 xxx.md §N」指针。
4. **IPC 命令唯一权威在 §8.2**（phase1），其他文件的命令表为摘要，签名以 §8.2 为准。

---

*编辑历史：文档由单文件 6253 行按阶段拆分为多文件（2026-08-05）。详见 [CHANGELOG.md](./CHANGELOG.md)。*
