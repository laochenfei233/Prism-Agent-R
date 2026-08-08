# Prism Agent R — Phase 4（自主能力深化）详细设计

> **参考来源**：本文件 §18 前端 UI 设计参考 `cherry-studio` 仓库的 `DESIGN.md`（shadcn/ui 设计系统）与 `AppShell`/`Sidebar`/`settings` 页面布局实现；§19 Agent 设计参考 Anthropic News（Claude Code 生态 10 篇，见 19.1 表格链接）与 OpenAI News（Codex/harness 生态 10 篇，见 19.2 表格链接）。
> **适配原则**：前端 UI 借用 cherry-studio 的布局骨架与排版节奏（三区 AppShell、两栏设置、PageHeader、状态徽标、卡片栅格、内容宽度约束），**色彩与材质沿用本项目 token**（以 `src/lib/design-system/styles/tokens.css` 实际名为准，如 `--color-bg`/`--color-green`），不引入 cherry-studio 的 neutral-alpha 色系；组件用本项目 base 组件库，不引入 shadcn 依赖（详见 §18.1 与 §18.7）。Agent 设计参考只采纳结论与映射，不复制原文；两家 2026 年推荐与 §15-18 设计方向高度一致，差异增量见 §19.3。
> **归属**：Phase 4（自主能力深化）· 由 4 个参考仓库查漏补缺驱动（审计结论见 [`gap-audit.md`](./gap-audit.md)）
> **总索引**：[`prism-agent-r.md`](../compose/specs/prism-agent-r.md) · **Phase 1**：[`phase1-core.md`](./phase1-core.md) · **Phase 2**：[`phase2-panel.md`](./phase2-panel.md) · **Phase 3**：[`phase3-extend.md`](./phase3-extend.md)
> **Updated**：2026-08-08
> **读者假设**：面向熟悉 Rust（tokio/sqlx/serde）、Svelte 5（runes）、Tauri 2.x（IPC/WebView）的开发者；不解释语言/框架基础语法。
> **内容**：§15 网络搜索工具链 · §16 RAG 检索增强 · §17 Harness 工程化（会话生命周期 / Loop / Trace Grading）· §18 前端 UI 设计（排版布局参考 Cherry Studio）· §19 Agent 设计参考（Anthropic & OpenAI 2026 推荐 + 19.3 增量设计 8 项）· §20 迁移与命令补记（迁移 023-025）· §21 任务清单（P4-T1~T19）
> **依赖基础（见 `phase1-core.md`）**：三层架构（§3）、流式/IPC（§7/§8）、数据库横切（§5 含 §5.7）、工作流引擎（§10.6.1 StageTemplate）、记忆系统（§10.7）
> **依赖基础（见 `phase2-panel.md`）**：工具审批/HITL（§10.10）、任务设计区（§9.9.1）
> **依赖基础（见 `phase3-extend.md`）**：RAG 引擎（§10.2 含 10.2.2 Contextual Retrieval / 10.2.3 解析 / 10.2.5 评测）、评估监控（§10.13 AgentJudge/agent_traces）、上下文压缩（§13.1）
> **编辑约定**：后续新增内容导致章节序号 +1 时，将新增章节的「参考来源 / 适配原则」增量更新到本头部，保持元信息在文档最前（2026-08-08 实证：新增 §19 Agent 设计参考 → 头部参考来源已增量更新）。

---

## 15. 网络搜索工具链

> **问题**：预置「深度研究」工作流（`phase1 §10.6.2`）声明 `web_search` 工具，但 `ToolRegistry` 未注册任何内置工具，运行时降级或报 Unknown tool（见 `gap-audit.md §2.1`）。Phase 4 落地真实网络搜索能力，并与 RAG 检索（§16）多路融合。

### 15.1 目标与边界

- 目标：Agent 通过 `web_search` 工具获得真实互联网检索能力，供深度研究工作流与多路并发检索（§16.2）使用。
- 边界：**仅检索，不写回**——不抓取全文入库，只返回带来源链接的结果摘要；不做浏览器自动化（[S3] 外）。
- 复用：工具注册/审批走现有 §10.10 HITL 链路；搜索历史不新增表（结果落缓存，见 §15.4）。

### 15.2 SearchProvider Trait

```rust
// src-tauri/src/core/search/mod.rs
#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn name(&self) -> &'static str;                 // "tavily" | "serper" | "searxng" | ...
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, AppError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,     // provider 名
    pub published_at: Option<i64>,
}
```

**实现（可插拔，配置切换）**：

| Provider | 说明 | 依赖 | 降级 |
|----------|------|------|------|
| `TavilyProvider` | AI 专用搜索（deep-search-pro 同款），返回结构化结果 | 需 API Key | 未配置 → 该 provider 不可用 |
| `SerperProvider` | Google SERP API，JSON 返回 | 需 API Key | 同上 |
| `SearxngProvider` | 自托管聚合搜索，无 Key，隐私友好 | 需本地/自建实例 | 同上 |
| `NoopSearchProvider` | 空实现 | 无 | **默认**：无任何配置时注册，`web_search` 工具可用但返回空结果 + 提示文案（优雅降级） |

**选择逻辑**（启动时 `SearchService::new(config)`）：

1. 读取设置 `search.provider`（设置页新增「网络搜索」区块，复用 settings.rs）。
2. 按配置实例化对应 provider；未配置 API Key 的 provider 跳过。
3. 至少一个 provider 可用 → 注册真实 provider；否则注册 `NoopSearchProvider`。
4. 多 provider 可并存，`SearchService::search` 轮询取第一个可用者（primary/secondary 配置），失败自动切换。

### 15.3 web_search 工具注册

实现 `ToolExecutor`（`core/adk/tool.rs`），注册名 `web_search`：

```rust
pub struct WebSearchTool { service: Arc<SearchService> }

impl ToolExecutor for WebSearchTool {
    fn name(&self) -> &str { "web_search" }
    fn description(&self) -> &str {
        "搜索互联网获取最新信息。参数：query（搜索词），limit（结果数，默认 5，最大 10）。返回带来源链接的结果摘要，适合时效性信息、事实核查、资料搜集。"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "搜索词" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 10, "default": 5 }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> { /* 见下 */ }
}
```

**execute 流程**：

1. 解析参数；`query` 为空 → 返回错误提示。
2. `service.search(query, limit)` → 查缓存（§15.4）→ 未命中调 provider → 写缓存。
3. 结果非空 → 格式化为 Markdown 列表（标题链接 + snippet + 来源）；空 → 返回「未找到结果或搜索服务未配置」提示（不报错，agent 可据此改查询词）。

**风险分级**：`web_search` 属**只读低风险**，加入 `assess_risk` 的 Low 分支（tool.rs:51），不触发 HITL 审批。

**注册点**（3 处，与现有工具注入路径对齐）：

- `commands/chat.rs:174`（对话）：`registry.register(Box::new(WebSearchTool::new(service.clone())))`
- `commands/workflow.rs:465`（工作流）：同上
- `SearchService` 通过 `TauriState`（settings.rs 已有的 `manage` 状态）共享。

### 15.4 搜索结果缓存（迁移 023）

网络搜索结果 1 小时内有效，避免重复计费与延迟。SQLite 缓存表：

```sql
-- 023_web_search_cache.sql
CREATE TABLE IF NOT EXISTS web_search_cache (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    query_hash    TEXT NOT NULL,          -- sha256(query + limit + provider)
    provider      TEXT NOT NULL,
    query         TEXT NOT NULL,
    results_json  TEXT NOT NULL,          -- Vec<SearchHit> 序列化
    created_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_search_cache_hash ON web_search_cache(query_hash);
-- 保留策略：超过 7 天的缓存由 §5.7.6 保留清理任务顺带删除（复用消息清理调度）
```

- 命中逻辑：`query_hash` 存在且 `created_at` 在 1 小时内 → 直接返回；否则调 provider 后 `INSERT OR REPLACE`。
- 键含 provider：切换 provider 后不误命中。
- 缓存为**尽力而为**：写失败不影响主流程（`let _ =`）。

### 15.5 预置工作流接线（深度研究）

`workflow_service.rs::deep_research_workflow` 的 `stage1` 已声明 `web_search`，工具注册后即自动生效。设计文档 `phase1 §10.6.2` 的 `knowledge_lookup` 工具**不在本阶段实现**（Wiki/RAG 检索由 §16 的 `rag:search` 增强承接，工作流模板同步改为 `web_search` + `read_file` + RAG 检索入口）。

### 15.6 错误处理

| 场景 | 处理 | 反馈 |
|------|------|------|
| 无任何 provider 配置 | `NoopSearchProvider` 返回空 + 提示 | agent 可见「搜索服务未配置」，可改走本地 RAG |
| provider 超时/网络错误 | 单 provider 失败 → 自动切换 secondary；全失败 → 返回错误文案 | agent 重试或降级 |
| API 限额 | 捕获 429 → 返回「搜索限额已用尽」 | 提示，不重试轰炸 |
| 缓存损坏 | 忽略缓存直接调 provider | 无感知 |

## 16. RAG 检索增强

> **问题**：当前 `hybrid_search`（`search.rs`）为单路混合（0.7 余弦 + 0.3 BM25），短查询语义稀疏、无多路融合、固定 top_k。借鉴 intelligent-kb-rag：HyDE、RRF 多路融合、稀疏向量、动态 TopK 断崖截断、图片 VL 摘要、幂等导入。

### 16.1 HyDE 假设文档检索

**动机**：短查询（如「保修政策」）与长文档语义不对齐，直接向量检索召回差。HyDE 先让 LLM 生成假设答案，再用假设答案的向量检索真实文档（intelligent-kb-rag 同款，延迟由多路并发抵消）。

**设计**：

```rust
// src-tauri/src/data/rag/hyde.rs
pub struct HydeRetriever { provider: Arc<dyn ModelProvider>, embedder: Arc<dyn Embedder> }

impl HydeRetriever {
    /// 1) LLM 生成假设文档（hyde_prompt） 2) 嵌入 3) 与 query 向量联合检索
    pub async fn retrieve(&self, wiki_id: &str, query: &str, top_k: usize) -> Result<Vec<RagHit>, AppError>;
}
```

- **Prompt**（新增 `hyde_prompt`，复用 §10.5.2 术语表渲染风格）：给 query，要求 LLM 写一段「假设该文档存在，其内容会如何描述此问题」的段落（100-200 字，含关键词）。
- **检索**：`embed(hyde_doc)` 得到 hyde_vec，`cosine_sim` 与全部 chunk 打分 → 得 HyDE 路结果。
- **配置开关**：`rag.hyde.enabled`（默认 true）、`rag.hyde.model`（默认用 Agent 当前模型）；关闭时跳过（§16.2 RRF 只剩两路）。
- **成本**：每次检索多一次 LLM 调用（约 1-3s），由多路并发（§16.2）掩盖。

### 16.2 RRF 多路召回融合

**动机**：单路加权（0.7+0.3）对分数量纲敏感，无法公平合并异构来源。改为 RRF（Reciprocal Rank Fusion）：`score = Σ 1/(k + rank)`，k=60（intelligent-kb-rag 同款），无训练、天然去重。

**三路并发检索**（langgraph 的并行分支 → 本项目 tokio::join!）：

```rust
// src-tauri/src/data/rag/search.rs — 扩展
pub struct RagSearcher {
    db: Database,
    embedder: Arc<dyn Embedder>,
    hyde: Option<HydeRetriever>,        // 16.1
    web: Option<Arc<SearchService>>,    // §15：wiki 检索不足时补充网络
}

pub async fn multi_path_search(
    &self,
    wiki_id: &str,
    query: &str,
    top_k: usize,
    cfg: &RagSearchConfig,
) -> Result<Vec<RagHit>, AppError>
```

| 路 | 输入 | 召回数 | 来源 |
|----|------|--------|------|
| A 普通混合检索 | query 向量 + BM25 | top-150 | 现有 `hybrid_search`（含 Contextual BM25） |
| B HyDE 检索 | hyde 文档向量 | top-150 | §16.1（`rag.hyde.enabled=false` 时跳过） |
| C 网络搜索（可选） | query | 10 | §15 `web_search`（`rag.web.enabled=true` 且 wiki 命中不足时） |

**融合**：

1. 三路并行（`tokio::join!`），互不阻塞。
2. 各路结果按各自分数排序 → 赋 rank → `RRF = Σ 1/(60 + rank)`（跨路重复 chunk 分数叠加，天然去重）。
3. 按 RRF 降序取前 top_k → 交给 §16.4 断崖截断。
4. **配置**（`rag.rrf` 组，含各路径权重的可选变体，默认等权；设计保持无训练原则）。

**网络路触发条件**（防滥用）：`rag.web.enabled=true` 且路 A/B 合并后去重命中数 < top_k 的 50% 时才补网络搜索。

### 16.3 稀疏向量落地（可选）

**现状**：BM25 为字符串级近似（`embedding.rs::bm25_score`），非真实稀疏向量。BGE-M3 风格稠密+稀疏双向量（intelligent-kb-rag）可提升关键词召回，但需引入稀疏向量模型/存储。

**决策**：本阶段**不做**（保持零新增模型依赖）。BM25 字符串级近似 + Contextual BM25（§10.2.2）已覆盖关键词场景；待有明确 recall 短板时再评估（[S5] 候选）。

### 16.4 动态 TopK 断崖截断

**动机**：固定 `truncate(top_k)` 会把无关文档硬塞进上下文。改为检测 Reranker/融合分数的「断崖」（相邻分数骤降）自动截断（intelligent-kb-rag：gap ≥ 0.5 或相对下降 ≥ 25%）。

**设计**（`rerank.rs` 现有 `LlmReranker` 之后）：

```rust
/// 对 RRF 融合后的 top-N 分数检测断崖，返回保留的 cutoff 索引
pub fn cliff_cutoff(scores: &[f32], min_gap: f32, rel_drop: f32) -> usize
```

- 规则：从第 2 个开始，若 `prev - cur >= min_gap` 或 `(prev - cur) / prev >= rel_drop` → 截断（保留到 prev）。
- 默认：`min_gap=0.15`（RRF 分数尺度小，比 intelligent-kb-rag 的 0.5 适配）、`rel_drop=0.25`；可配置。
- 保底：至少保留 3 个结果，避免过度截断。
- 无 Reranker（Noop）时同样作用于融合分数。

### 16.5 文档图片 VL 摘要（可选增强）

**现状**：§10.2.3 解析保留图片引用，但 chunk 内图片未语义化（检索不到图片内容）。

**设计**（借鉴 intelligent-kb-rag 图片处理节点）：

- 解析阶段（`parser.rs`）：提取文档图片 → 多模态 LLM（复用 §10.5 OCR 的 VL provider）生成一句话摘要 → 摘要文本拼入所属 chunk 的 context（`015_rag_context` 的 context 列）。
- 摘要失败 → 保留原引用，不影响主流程。
- **决策**：本阶段作为可选开关 `rag.image_summary.enabled`（默认 false，成本敏感）；不开不影响检索正确性。

### 16.6 幂等导入

**现状**：`store.rs` 已有 `find_document_by_path` / `fingerprint_of_document`（021 迁移加 file_path/fingerprint），项目索引已按指纹增量；但用户 Wiki 手动导入路径的幂等未显式校验。

**设计**（`store.rs::insert_document_with_meta` 扩展）：

1. 入参带 `file_path` + `fingerprint`（mtime:size，复用 §10.2.1 指纹规则）。
2. `find_document_by_path(wiki_id, file_path)`：
   - 存在且指纹相同 → 跳过（返回既有 doc_id，不重复入库）；
   - 存在且指纹不同 → `delete_document_by_path` 后重新入库（内容已变更）；
   - 不存在 → 正常入库。
3. 无 file_path 的导入（剪贴板文本等）不触发幂等（保持原行为）。

### 16.7 检索链路错误处理

| 场景 | 处理 |
|------|------|
| HyDE LLM 调用失败 | 跳过 HyDE 路，RRF 剩两路（不阻断） |
| 网络路失败 | 跳过网络路，RRF 剩本地两路 |
| 三路全空 | 返回空 + 建议换措辞 |
| embedding 失败 | 退化为纯 BM25（现有行为保留） |

## 17. Harness 工程化

> **动机**：walkinglabs 两仓库把「让 agent 可靠」归纳为五子系统（Instructions / State / Verification / Scope / Lifecycle）与 Loop/Graph 工程。本项目已有记忆（§10.7）、评估（§10.13）、护栏（§10.12）、压缩（§13.1）——Phase 4 补三块：会话生命周期显式化、Loop 自动化、Trace Grading。

### 17.1 Agent 会话生命周期（init / verify / clean-state）

**现状**：会话创建即对话，无显式「初始化-验证-清理」阶段。借鉴 learn-harness L06/L12：`init.sh` 于开始、`clean-state checklist` 于结束。

**设计**（会话状态机扩展 `session.rs`）：

```
CREATED → INIT → READY → RUNNING → (VERIFY)* → DONE
              │          └── 中断 → PAUSED → RESUME → RUNNING
              └── init 失败 → INIT_FAILED（提示 + 可重试）
```

1. **INIT（进入会话）**：加载 Agent 配置 → 校验模型 Provider 可达（`model:test` 快速连通）→ 加载记忆注入（现有 §10.7.4）→ 校验 MCP 服务器可达（失败仅告警不阻断）→ 状态置 READY。
2. **VERIFY（每轮完成）**：流式响应结束后对可验证的产物做轻量自检——有工具调用则校验 Tool 输出非空；有 RAG 引用则校验引用 chunk 存在（§10.2.4 可追溯引用）；`verify:skip` 可关闭（低延迟场景）。
3. **CLEAN-STATE（会话结束/切换）**：`SessionCheckpoint` 落盘（现有 §10.7 机制）→ 释放取消令牌 → 更新会话标题（现有）→ 记 usage 快照（§9.10.1）。异常中断（崩溃/杀进程）由下次 INIT 检测上次 checkpoint 缺失 → 标记会话为「未正常结束」并在 UI 提示。

**IPC**（§8.2 追加）：

| 命令 | 入参 | 返回 | 说明 |
|------|------|------|------|
| `session:init` | `session_id` | `SessionInitReport`（provider/mcp/记忆 三项校验结果） | 进入会话初始化 |
| `session:state` | `session_id` | `SessionState` | 查询状态机当前态 |
| `session:cleanup` | `session_id` | `()`, 事件 `session:state-changed` | 手动触发清理 |

前端：会话切换时先 `session:init`，状态变化经 `session:state-changed` 事件驱动侧边栏显示（INIT_FAILED 黄色徽标提示）。

### 17.2 Loop 工程（goal / timer / maker-checker 自动化）

**现状**：GoalMonitor（§10.11）支持目标定义与单次评估；反思（§10.9）为生产者-评审者。缺**循环调度**：目标未达成自动重试、定时任务触发、maker-checker 分离评审。

**设计**（`autoagents/loop.rs` 新增，复用 §10.6 引擎）：

```rust
pub enum LoopKind { Goal, Timer, MakerChecker }

pub struct AgentLoop {
    kind: LoopKind,
    interval_secs: Option<u64>,          // Timer：定时间隔
    max_rounds: u32,                     // 上限防失控（默认 5）
    goal: Option<TaskGoal>,              // Goal：未达成继续
    maker: Workflow,                     // MakerChecker：生成
    checker: Workflow,                   // MakerChecker：评审
    on_round: Option<Arc<dyn Fn(&LoopRound) + Send + Sync>>,
}
```

| 类型 | 语义 | 终止条件 |
|------|------|---------|
| Goal loop | 每轮执行工作流 → GoalMonitor 评估 → 未达标重试（改参数/换策略提示） | 达标 或 max_rounds |
| Timer loop | 按 interval 触发工作流（如每日晨报、知识库巡检） | 手动停止 或 max_rounds |
| Maker-Checker | maker 产出 → checker 独立评审（LLM 打分+意见）→ 不通过则 maker 带评审意见重做 | checker 通过 或 max_rounds |

- **调度**：Timer loop 用 tokio interval + 会话取消令牌（复用 §7.4 取消链路）；Goal/Maker-Checker 为同步 await 循环。
- **命令**（§8.2 追加）：`loop:start`（kind/interval/max_rounds/goal/maker/checker）→ `loop:stop` → `loop:list`；事件 `loop:round`（每轮推送 round 序号/状态/摘要）。
- **前端**：主页面板任务设计区（§9.9.1）「自动化」页签：Goal/Timer/Maker-Checker 三卡片创建；运行中显示轮次进度条。
- **护栏**：`max_rounds` 硬上限；每轮 token 预算复用 §13.1 TokenBudget；所有工具调用走 §10.10 审批（High+ 需人工确认）。

### 17.3 Trace Grading 与 Session Replay

**现状**：`agent_traces`（016 迁移）+ AgentJudge（§10.13）已有轨迹落库与 LLM 打分，但：①评分结果未回写便于检索；②无会话回放视图。

**设计**：

1. **Trace Grading 回写**（`trace.rs` + 迁移 024）：

```sql
-- 024_trace_grading.sql
ALTER TABLE agent_traces ADD COLUMN grade_score REAL;      -- 0-1，AgentJudge 打分
ALTER TABLE agent_traces ADD COLUMN grade_reason TEXT;     -- 评分理由（LLM）
ALTER TABLE agent_traces ADD COLUMN graded_at INTEGER;
CREATE INDEX IF NOT EXISTS idx_traces_grade ON agent_traces(agent_id, grade_score);
```

   触发：`agent_judge_evaluate`（现有命令）评分后回写；新命令 `trace:grade`（按 trace_id 单条评分/重评）。

2. **Session Replay**（前端，`trace_list` 现有数据源扩展）：
   - 按 agent/时间线列出轨迹 → 展开单条：输入 → 每轮 tool_call（名称/参数/结果摘要）→ 流式输出 → 最终结果 → grade 徽标。
   - 过滤：`trace:list` 增 `min_grade` / `tool_failed` 过滤参数。
   - 定位：侧边栏「评估」Tab 或独立「轨迹回放」视图（复用 §9.10 侧边栏组件结构）。

3. **可观测性内建**（对齐 L11）：轨迹在运行时即写（现有 `on_trace`），**不增加额外开关**；`session:init` 的校验结果与 Loop 轮次也记入 trace（`kind=trace` 扩展字段，016 迁移已有结构）。

### 17.4 与现有体系的关系

| 本阶段新增 | 复用 | 不重复造 |
|-----------|------|---------|
| 会话生命周期状态机 | 记忆 checkpoint / 取消令牌 / model:test | 不复刻 §10.7 记忆机制 |
| AgentLoop 调度 | §10.6 工作流引擎 / §10.11 GoalMonitor / §10.10 审批 | 不重写编排核心 |
| Trace Grading | §10.13 AgentJudge / 016 迁移 | 不另建评测体系 |

## 18. Phase 4 前端 UI 设计（排版与布局，参考 Cherry Studio）

> 参考来源与适配原则见文件头部（编辑约定：新增章节序号 +1 时元信息增量更新到头部）。

### 18.1 借鉴的布局骨架（对照 cherry-studio 实现）

| cherry-studio 布局 | 实现参考 | Phase 4 适配 |
|--------------------|---------|-------------|
| 三区 AppShell：顶部 TabBar（44px）→ 侧边栏 + 主内容 | `components/layout/AppShell.tsx` | 本项目已是三栏布局（§9.5 Codex 风格）；不新增窗口级壳 |
| 侧边栏独立空间色区（`--color-sidebar` 全套 token，220px 宽，32px 行高） | `components/Sidebar/primitives.tsx` + `DESIGN.md §Sidebar` | 沿用 §9.10 Agent 侧边栏（六 Tab）；会话状态徽标挂入侧边栏（18.4） |
| 设置页两栏：左 250px 子菜单 + 右 `flex-1 max-w-3xl` 详情 | `components/SettingsPrimitives.tsx`（`SettingsContentColumn`/`SettingsContentBody`） | 本项目设置界面（§9.5.3）为三栏（设置导航/分组列表/详情）；Phase 4 新增「搜索」分组进中间分组列表，详情落右栏，宽度约束复用 cherry 的 `max-w-3xl` |
| 统一页眉 `PageHeader`（h-8 / 32px，title + 右侧 action） | `@cherrystudio/ui` PageHeader | 每个 Phase 4 视图（搜索测试、Loop、轨迹回放）以 PageHeader 开头 |
| 状态徽标 status palettes（base/text/bg/border 四件套） | `tokens/colors/status.css` | 用于 Loop 轮次状态 / 会话生命周期状态 / grade 徽标（18.5-18.6） |
| 设置抽屉 `PageSidePanel`（Section → Item 三层结构，gap-8 分节） | `components/composites/page-side-panel` | 搜索 provider 详情配置用轻量设置抽屉（复用本项目 Sheet.svelte） |
| 内容宽度约束：外层 `px-6 py-4` + 内层 `mx-auto max-w-3xl` | `DESIGN.md §Settings Panel Layout` | 所有 Phase 4 视图采用（对齐 §9.5.3 设置页惯例） |

**排版节奏（沿用 cherry-studio 数值，颜色走本项目 token）**：

- 字体：沿用本项目字体栈（`--font-sans`/`--font-mono`，tokens.css）；字号阶梯对齐——caption 12px（徽标/时间戳）、footnote 13px（导航/说明）、body 17px（正文）、title3 20px（视图标题）。
- 字重：regular 400（正文）/ medium 500（标签、导航）/ bold 700（页面级强调），与 cherry-studio 三档对齐（本项目 `--font-weight-*`）。
- 圆角：沿用本项目 token（tokens.css）——按钮/输入 `--radius-sm`（8px）、卡片 `--radius-md`（12px）到 `--radius-lg`（16px）、对话框 `--radius-xl`（20px）、胶囊 `--radius-pill`（9999px）。
- 间距：4px 基准刻度（`--spacing-xs` 到 `--spacing-xxl`）；卡片内 padding 16-24px、分节 gap-8、页面分节 gap-12 到 gap-24。
- 深度：表面靠颜色分层（`--color-bg` → `--color-bg-elevated`），阴影仅用于交互反馈与浮动元素（弹层/对话框），不做静态悬浮。

### 18.2 网络搜索设置区块（§15 前端）

**位置**：设置页（§9.5.3 三栏骨架）新增「搜索」分组——左设置导航 + 中间分组列表（「网络搜索」项）+ 右详情。

**结构**（对齐 cherry-studio 设置抽屉 Section → Item 三层，复用本项目 `Sheet.svelte` + `Switch.svelte` + `Input.svelte` + `Select.svelte`）：

| Item | 控件 | 绑定命令 | 说明 |
|------|------|---------|------|
| 启用网络搜索 | `Switch` | `search:config` 写入 | 关闭时注册 NoopProvider（§15.2 降级） |
| Provider | `Select`（tavily/serper/searxng） | 同上 | 配置项 `search.provider` |
| API Key | `Input`（password，右侧明文切换） | 同上 | 复用设置页现有 Key 加密存储（§12） |
| 测试连接 | `Button variant="outline"` + 结果行 | `search:test` | 返回首条结果标题/耗时/Provider，成功绿徽标、失败红徽标 |
| 缓存说明 | 静态文本（`--color-fg-tertiary`） | — | 「结果缓存 1 小时（§15.4）」 |

**交互**：Provider 切换后 Key 输入框联动显隐（searxng 无需 Key）；`search:test` 按钮 loading 态（复用 Button.svelte 的 spinner 模式）。

### 18.3 RAG 检索增强配置面板（§16 前端）

**位置**：Wiki 详情页（§10.1 前端）检索设置区，或设置页「RAG」分组。以**配置卡片栅格**呈现（对齐 cherry-studio `Tool Gallery` 模式：`--color-bg-elevated` 表面 + `--color-border-strong` 边框 + 16-24px padding + `--radius-md`，无阴影）：

| 卡片 | 内容 | 默认 |
|------|------|------|
| HyDE 假设文档检索 | 标题 + 描述 + `Switch`（`rag.hyde.enabled`）+ 模型 Select | on |
| RRF 多路融合 | `Switch`（`rag.rrf.enabled`）+ 各路径权重数值输入（可选） | on |
| 断崖截断 | `Switch`（`rag.rrf.cliff_cutoff`）+ min_gap/rel_drop 数值输入 | on（0.15/0.25） |
| 网络搜索补充 | `Switch`（`rag.web.enabled`）——依赖 §15 已配置 provider | off |

**检索链路可视化**（Phase 4 新增，复用 dashboard `UsageTrendChart.svelte` 的图表模式）：

```
查询 → [A 混合检索] [B HyDE] [C 网络]  ──tokio::join!──→ RRF 融合 → 断崖截断 → 注入
          (top-150)  (top-150)  (top-10)      ↑rank 融合      ↑gap 截断
```

- 每路以胶囊徽标显示状态（✓ 完成 / ⚠ 降级跳过 / ✗ 失败），颜色走本项目语义色（`--color-green`/`--color-orange`/`--color-red`，tokens.css）。
- 截断点可视化：分数曲线 + 断崖位置标记（对齐 intelligent-kb-rag 动态 TopK 思路的 UI 呈现）。

### 18.4 会话生命周期状态显示（§17.1 前端）

**位置**：Agent 侧边栏（§9.10）会话列表条目 + 会话标题旁。

**状态徽标**（对齐 cherry-studio status palette 四件套 → 本项目语义色，token 名以 tokens.css 为准）：

| 状态 | 徽标 | 色（tokens.css） | 触发 |
|------|------|-----------------|------|
| INIT | 灰 `--color-bg-hover` | 进行中，不可对话 | `session:init` 开始 |
| READY | 绿 `--color-green` | 初始化通过 | `session:state-changed` |
| RUNNING | 橙/主色 `--color-accent` | 流式生成中 | 会话消息开始 |
| PAUSED | 橙 `--color-orange` | 中断可恢复 | 取消令牌触发 |
| INIT_FAILED | 橙（+ Tooltip 显示失败项） | provider/mcp/记忆哪项失败 | init 报告非全绿 |
| DONE / 异常结束 | 灰 / 红 `--color-red` | 正常结束 / 上次未正常结束提示 | cleanup / 下次 init 检测 |

**布局**：徽标 12px 圆点 + 4px 间距挂在会话标题左侧；`session:state-changed` 事件驱动 Svelte 5 store 更新（复用 `stores/chat.svelte.ts` 模式），不轮询。

### 18.5 Loop 自动化设计区（§17.2 前端）

**位置**：主页面板任务设计区（§9.9.1）新增「自动化」页签。

**创建视图**：Goal/Timer/Maker-Checker 三卡片（对齐 `TaskTemplateCard.svelte` 现有样式）：

| 卡片 | 字段 | 复用 |
|------|------|------|
| Goal 循环 | 目标描述（复用 §10.11 TaskGoal 字段）、max_rounds、触发工作流 Select | §9.9.1 任务表单字段 |
| Timer 循环 | interval（分钟）、工作流 Select、max_rounds | 同上 |
| Maker-Checker | maker 工作流 Select + checker 工作流 Select、max_rounds | 同上 |

**运行视图**：

- 轮次进度：`Progress.svelte`（round/max_rounds）+ 每轮状态徽标（同 18.4 色系）。
- 实时事件流：`loop:round` 事件 → 轮次卡片追加行（round 序号、状态、输出摘要），滚动到底部。
- 停止按钮：`Button variant="destructive"` 触发 `loop:stop`。
- 布局：运行列表 `--color-bg-elevated` 表面 + `--color-separator` 0.5px 行分隔，对齐 cherry-studio 密集列表节奏。

### 18.6 轨迹回放视图（§17.3 前端）

**位置**：侧边栏「评估」Tab 或独立视图入口（复用 §9.10 侧边栏组件结构）。

**列表**：`trace:list` 数据源 → 时间线列表（每行：时间戳 body-xs / Agent 名 / 输入摘要 body-sm / grade 徽标）。

**展开详情**（单条 trace）：

```
输入（用户消息）
  └─ 轮 1: tool_call [web_search] args 摘要 ──→ 结果摘要
  └─ 轮 2: tool_call [rag_search] args 摘要 ──→ 结果摘要
  └─ 流式输出摘要
  └─ 最终结果 + grade 徽标（0-1 分数 + LLM 评分理由 Tooltip）
```

- 过滤栏：`Input`（关键词）+ `Select`（min_grade：全部/≥0.5/≥0.8）+ `Switch`（仅含失败工具调用）→ 对应 `trace:list` 新参数。
- grade 徽标色阶：≥0.8 绿 `--color-green` / 0.5-0.8 橙 `--color-orange` / <0.5 红 `--color-red`。
- 重评按钮：行内 `IconButton`（复用 base/IconButton.svelte）+ `Button variant="outline"` 触发 `trace:grade`。

### 18.7 设计约束清单（Phase 4 UI）

1. **不引入新色板**：所有颜色走本项目 token（tokens.css 实际名：`--color-bg*`/`--color-fg*`/`--color-green`/`--color-orange`/`--color-red`/`--color-accent`/`--color-separator`/`--color-border-strong`），禁止硬编码 hex/rgba。
2. **布局骨架跟随 cherry-studio**：两栏设置 / PageHeader / 状态徽标 / 内容 `max-w-3xl` 约束，但组件用本项目 base 组件库（Button/Switch/Input/Select/Sheet/Progress），不引入 shadcn 依赖。
3. **低强调默认**：行内操作（重评、测试、复制）默认 `--color-fg-tertiary`，hover 才强调；危险操作触发确认对话框（复用 base/Modal.svelte）。
4. **阴影纪律**：仅浮动元素（Sheet/弹层）与 hover 反馈使用（`--shadow-sm/md`），卡片静态不悬浮（对齐 cherry-studio「flat at rest」）。
5. **暗色适配**：沿用 tokens.css `.dark` 主题，新增视图全部走 token，明暗自动反转。

> **§18 与任务映射**：§18.2 搜索设置 → P4-T1/T2 前端部分；§18.3 RAG 配置面板 → P4-T4/T5/T6 前端部分；§18.4 会话状态徽标 → P4-T8；§18.5 Loop 设计区 → P4-T9；§18.6 轨迹回放 → P4-T10。所有 UI 任务验收时按 §18.7 约束清单检查。

## 19. Agent 设计参考（Anthropic & OpenAI 2026 推荐文章）

> **用途**：整理 2026 年 Anthropic / OpenAI 官方博客中与 Agent 设计直接相关的推荐阅读，提炼可移植到本项目的设计原则。本文只记录**结论与映射**，不复制原文；正文入口见各条目链接（文章页可直读，无需代理）。
> **调研方法**（2026-08-08）：① Anthropic `news` 列表页 2026 年 91 篇中精读 10 篇 Agent 相关；② OpenAI 用 RSS（`/news/rss.xml`，2026 年 330 篇）精读 10 篇 Codex/Agent 相关；③ 与 phase4 §15-18 逐条映射，去重后保留未覆盖增量。
> **章节结构**：19.1 Anthropic 推荐（Claude Code 生态）· 19.2 OpenAI 推荐（Codex 生态）· 19.3 对本项目的增量建议。

### 19.1 Anthropic 推荐（Claude Code / Claude 生态）

| 文章 | 核心设计要点 | 对本项目映射 |
|------|------------|-------------|
| [The Making of Claude Code](https://www.anthropic.com/features/making-of-claude-code)（7 月，口述史） | read/edit/bash 三原语即可驱动一切 agentic 编码；harness = 模型周围脚手架（持久 shell + 流式 IO + 超时）；并行 fan-out 100 个 Haiku 处理超上下文任务；小团队防过度设计、CLI 快速迭代、auto-update + 用户指标、先做到 20-30% 可靠等模型进步、信任后 auto-accept | 印证 §10.8 文件工具/§10.6 工作流/§17 Loop；并行 fan-out 思路支持 §16.2 多路并发 |
| [Claude Code Security](https://www.anthropic.com/news/claude-code-security)（2 月） | 多阶段自验证：模型证明/证伪自己的发现，过滤误报；severity + confidence 双评级；**无人工审批不应用** | 强化 §16 断崖截断与 §10.13 AgentJudge：评分后回写 + 人工确认 |
| [Introducing Claude Opus 5](https://www.anthropic.com/news/claude-opus-5)（7 月） | **mid-conversation tool changes**（改工具不失效 prompt cache）；automatic fallbacks（分类器拦截自动路由到备用模型）；effort 强度档；agent 把 context 当活文档自管理记忆；self-verification/自己搭测试 harness | mid-conversation tool changes ↔ §8 MCP 工具热切换；effort ↔ §13.1 压力等级；自管理记忆 ↔ §10.7 |
| [Apple's Xcode supports Claude Agent SDK](https://www.anthropic.com/news/apple-xcode-claude-agent-sdk)（2 月） | Claude Code 底层 harness = Agent SDK（subagents / background tasks / plugins 三件套）；**visual verification**（截屏闭环改 UI）；项目级推理；goal 驱动而非指令 | 支撑 §17 会话生命周期/§17.2 Loop 自动化；视觉验证思路可用于 §18.6 轨迹回放 |
| [Reflect with Claude](https://www.anthropic.com/news/reflect-with-claude)（7 月） | **4D AI Fluency**：Delegation / Description / Discernment / Diligence；用量可视化 + 定期反思提示 | 可借鉴到 §10.13 评估（用户层反思报告）与用量侧边栏 §9.10.1 |
| [Agents for financial services](https://www.anthropic.com/news/finance-agents)（5 月） | agent 模板 = **skills + connectors + subagents 三件套**；Managed Agents 提供 long-running sessions、per-tool 权限、凭据保险库、**完整审计日志**；用户 in-the-loop 审批 | 印证 §10.4 Skill + §10.6 工作流 + §17.3 trace 审计完整性 |
| [Introducing Claude Sonnet 5](https://www.anthropic.com/news/claude-sonnet-5)（6 月） | 最 agentic Sonnet；effort 强度档；**10M token budget with compaction** | 印证 §13.1 上下文压缩方向（compaction 语义） |
| [Introducing Claude Tag](https://www.anthropic.com/news/introducing-claude-tag)（6 月） | **multiplayer 单 Claude 多人协作**；scoped identity/记忆按渠道隔离；ambient 主动行为；异步调度任务；org/channel 级 token spend 限制 + 操作日志含请求人 | scoped identity ↔ §10.7 记忆命名空间隔离；审计日志 ↔ §10.13 |
| [Safety incidents in cyber evals](https://www.anthropic.com/news/investigating-incidents-cybersecurity-evals)（7 月） | 141,006 次运行回顾发现评估逃逸：环境隔离失败 + prompt 误称无网络；新模型识别真实环境后停止 vs 旧模型继续 → **情境感知是安全关键**；纵深防御 + blameless postmortem | 强化 §10.12 护栏：范围明确 prompt + 轨迹监控 + 停止条件 |
| [Higher usage limits & SpaceX](https://www.anthropic.com/news/higher-limits-spacex)（5 月） | 算力/用量限制变更，Agent 设计参考有限 | —（不采纳） |

### 19.2 OpenAI 推荐（Codex / Harness 生态）

| 文章 | 核心设计要点 | 对本项目映射 |
|------|------------|-------------|
| [Unrolling the Codex agent loop](https://openai.com/index/unrolling-the-codex-agent-loop/)（1 月） | agent loop 定义（input→inference→tool→append→requery）；prompt 构建链（sandbox permissions / AGENTS.md 聚合 32KiB / skills 元数据 / environment_context）；**旧 prompt = 新 prompt 精确前缀 → prompt caching 线性化**；中途配置变化用追加 message 而非改旧；**/responses/compact 自动压缩**（保留潜在理解） | prompt 前缀保持 ↔ chat.rs 请求构建；compact 保留理解 ↔ §13.1 需补「摘要化压缩」而非仅裁剪 |
| [Unlocking the Codex harness (App Server)](https://openai.com/index/unlocking-the-codex-harness/)（2 月） | **Item/Turn/Thread 三原语**（item 生命周期 started→delta→completed；thread create/resume/fork/archive 持久化）；JSON-RPC over stdio 双向协议；**服务器可主动发请求**（approval 暂停 turn 等客户端 allow/deny）；统一 harness 供多客户端复用 | 三原语 ↔ §17.1 会话生命周期状态机；双向 approval ↔ §10.10 升级机制 |
| [Harness engineering: 0 hand-written code](https://openai.com/index/harness-engineering/)（2 月） | **AGENTS.md 当目录（~100 行）而非百科全书**；docs/ 为 system of record + CI 机械校验 + doc-gardening agent；agent legibility（看不到的即不存在）；**架构约束机械强制**（分层依赖 + 自定义 linter 注入修复提示）；Ralph Wiggum Loop（自审 + agent 互审 + 人类可缺席） | AGENTS.md 渐进披露 ↔ §10.7 记忆注入 + 本仓库 docs/design 体系；机械约束 ↔ §10.12 护栏 |
| [Running Codex safely at OpenAI](https://openai.com/index/running-codex-safely/)（5 月） | 沙箱 + approval 双层；**Auto-review mode = 独立审批子代理**自动批低风险动作；网络白名单（allowed/denied_domains + cached）；凭据 OS keyring；Starlark prefix_rule 规则引擎；**OTel 日志导出** → SIEM + AI 安全分流 agent | Auto-review ↔ §17.2 Loop 审批自动化；OTel 导出 ↔ §10.13 trace 落库；规则引擎 ↔ §10.12 护栏策略化 |
| [Safety in long-horizon models](https://openai.com/index/safety-alignment-long-horizon-models/)（7 月） | 长时模型持久性→更多机会做坏事；单动作安全 ≠ 轨迹安全（token 拆两半绕过扫描器）；**incident-derived evals + trajectory-level active monitoring**（监控整条轨迹可暂停会话）；迭代部署 = 受限 + 监控 + 可回滚 | 轨迹级监控 ↔ §10.13 + §17.1（会话可暂停）；incident-derived evals ↔ §10.2.5 评测 |
| [Two settings tripled ARC-AGI-3 scores](https://openai.com/index/how-two-settings-tripled-our-arc-agi-3-scores/)（7 月） | **retained reasoning + compaction = 分数 ×3、输出 token ÷6**；rolling truncation（丢最旧）远劣于 compaction（摘要保留学到的东西）；eval 测的是「模型 + harness + 设置」捆绑体 | **§13.1 压缩应升级为 compaction 语义**（摘要 + 保留推理），非简单裁剪；评估需含 harness 设置 |
| [SWE-Bench Pro audit](https://openai.com/index/separating-signal-from-noise-coding-evaluations/)（7 月） | **~30% 任务损坏**（过严测试/欠明确 prompt/低覆盖测试/误导 prompt）；agent 审查管线（Codex 调查 agent + 5 工程师标注）74% 重叠；eval 须 hard-to-game / easy-to-trust / reflective | §10.2.5 五维评测需带「评测用例质量审计」环节 |
| [Third-party cyber evals incidents](https://openai.com/index/third-party-cyber-evaluations-involving-openai-models/)（8 月） | 评估逃逸：目标名撞真实域名 + 误配网络 → 模型攻真实网站；教训 = 评估环境需生产级安全标准 + 范围明确 prompt + 实时监控 + 停止条件 | 与 Anthropic 同主题，佐证 §10.12 纵深防御 |
| [Codex-maxxing for long-running work](https://openai.com/index/codex-maxxing-long-running-work/)（6 月） | 白皮书：Codex 作**持久工作区**保留上下文；目标拆可验证步骤；委派 vs 人工监督判断 | 支撑 §17.1 会话生命周期 + §17.2 Goal Loop |
| [GPT-5.6 efficiency](https://openai.com/index/gpt-5-6-frontier-intelligence-efficiency/)（7 月） | 效率 = 模型 + 推理 + agentic 工作流三层面；compaction/effort 是 agent 效率关键 | 印证 §13.1 TokenBudget 设计 |

### 19.3 对本项目的增量设计（由参考文章提炼，超出 §15-18 已覆盖）

> 以下 8 项由 19.1/19.2 推荐提炼，**直接以设计文稿形式给出**（问题 / 接口 / 配置 / 流程 / 验收），不再停留在建议清单。落点涉及 phase1-3 章节的，本文件只写 **Phase 4 增强设计**，完整基础见对应文件（跨文件引用约定：一处完整内容，其余放指针）。

#### 19.3.1 会话上下文 compaction 语义压缩（增强 phase3 §13.1）

**问题**：现有 §13.1 为 TokenBudget 压力等级 + 软裁剪（丢最旧/裁剪工具输出）。OpenAI ARC-AGI-3 实测：rolling truncation（丢最旧）远劣于 **compaction**（摘要保留学到的东西）——后者使分数 ×3、输出 token ÷6。裁剪式压缩会丢失"模型已学到的模式"，长会话中反复重学。

**设计**（`core/rig/compaction.rs` 扩展，替代纯裁剪路径）：

```rust
/// 压缩策略：Level 1 裁剪（现有）→ Level 2 摘要化 compaction（新增，默认启用）
pub enum CompactStrategy { Truncate, Summarize }

pub struct Compactor {
    strategy: CompactStrategy,
    summarize_prompt: String,      // 会话摘要 prompt（固定前缀保 prompt cache）
    trigger_tokens: usize,         // 触发阈值，默认 100_000（与 TokenBudget.chat 一致）
    keep_reasoning: bool,          // 保留推理轨迹（对齐 retained reasoning），默认 true
}

impl Compactor {
    /// 超阈值时：调用 LLM 生成「会话摘要 + 未完成目标 + 关键约束」，
    /// 用摘要 + 最近 N 条消息替换历史（保留模型潜在理解，而非简单丢最旧）。
    pub async fn compact(&self, provider: &dyn ModelProvider, history: &[ChatMessage]) -> Result<Vec<ChatMessage>, AppError>;
}
```

- **配置**（设置注册表，`data/settings/registry.rs` 追加）：`context.compact.strategy`（truncate/summarize，默认 summarize）、`context.compact.keep_reasoning`（默认 true）、`context.compact.trigger_tokens`（默认 100000）。
- **摘要 prompt 固定前缀**（对齐 Codex prompt caching 原则）：指令/示例在前、可变内容在后，保证多次 compact 请求前缀可缓存。
- **保留推理**：推理轨迹（reasoning 消息）以加密/截断形式随摘要保留，避免模型"每轮重新理解任务"（ARC 教训）。
- **回退**：compact LLM 调用失败 → 降级为现有 Truncate 策略，不阻断会话。
- **验收**：构造超阈值长会话，summarize 策略下续聊能延续"未完成目标"；truncate 策略下丢失目标语义。单元测试覆盖两策略切换与失败降级。

#### 19.3.2 会话三原语 Item / Turn / Thread（重述 §17.1 状态机）

**问题**：§17.1 会话状态机为线性 CREATED→INIT→READY→RUNNING→VERIFY→DONE，缺原子粒度。Codex App Server 用 **Item/Turn/Thread** 三原语统一了流式事件与持久化，客户端可 `started` 即渲染、`delta` 流式更新、`completed` 定稿。

**设计**（`session.rs` + §7 事件模型对齐）：

```rust
pub enum ItemKind { UserMessage, AgentMessage, ToolExecution, ApprovalRequest, Diff }

pub struct SessionItem {
    pub id: String,
    pub kind: ItemKind,
    pub status: ItemStatus,       // started → (delta)* → completed
    pub payload: serde_json::Value,
}

pub struct SessionTurn {
    pub id: String,
    pub items: Vec<SessionItem>,
    pub status: TurnStatus,       // running → completed | awaiting_approval | cancelled
}

pub struct SessionThread {         // = 现有 session 持久化容器
    pub id: String,
    pub turns: Vec<SessionTurn>,
    // create / resume / fork / archive 四个操作（fork = 从某 turn 分支新会话）
}
```

- **事件对齐**（§8.3 追加）：`session:item-started` · `session:item-delta` · `session:item-completed` · `session:turn-started` · `session:turn-completed`（替代/补充现有 `chat:stream:*`，前端按 item 粒度渲染）。
- **fork 语义**：从指定 turn 复制历史为独立新会话（`session:fork {thread_id, turn_id}`），对齐 Codex 的 thread fork，用于"从某轮分歧点另起一支"。
- **状态机重述**：§17.1 的 CREATED→DONE 成为 **Thread 级状态**；INIT/VERIFY/CLEAN-STATE 保持；每个 turn 内部以 Item 粒度流式推进（不改变现有 RigAgent 循环，只改事件/持久化表达）。
- **验收**：前端断线重连后能按 Item 增量重建时间线（对齐 Codex web「新会话可 catch up」）；fork 后新会话独立且历史完整。

#### 19.3.3 双向审批（服务器主动暂停 turn 请求 allow/deny）

**问题**：现有 §10.10 为「工具触发 → 发事件 → 前端回命令」，单向通知式。Codex App Server 是**双向 JSON-RPC**：服务器发起请求、**暂停 turn**、等待客户端 allow/deny 后才继续——审批是协议内的一等公民。

**设计**（`core/rig/agent.rs` 审批链路升级）：

```
Agent 请求审批 ──→ 暂停当前 turn（await）──→ 客户端响应 allow/deny ──→ 恢复 turn
                        ↑
            事件 session:item-started (kind=ApprovalRequest, id=call_id)
            携带 reason / tool / args / risk
```

- **新命令**：`session:approve {call_id, decision, always_allow?}`（替代/兼容现有 `tool:approval-response`）；`decision ∈ {allow, deny}`。
- **暂停语义**：审批等待期间 turn 状态 `awaiting_approval`（19.3.2 的 TurnStatus），超时（默认 60s）自动 deny 并回退工具（对齐现有 Defer 行为）。
- **兼容**：旧 `tool:approval-request` 事件保留为降级路径（无暂停语义的客户端）。
- **验收**：工具调用触发审批时，turn 卡在 awaiting_approval 且流式输出暂停；allow 后继续、deny 后工具返回错误文案。

#### 19.3.4 Auto-review 审批子代理（低风险自动放行）

**问题**：§17.2 Loop 自动化中，每轮工具调用都可能触发 §10.10 审批 → 高频打断。Codex auto-review 用**独立审批子代理**自动批准低风险动作，仅高风险/不可逆动作请求用户。

**设计**（`core/autoagents/loop.rs` 增强 + 新模块 `approval/reviewer.rs`）：

```rust
pub struct AutoReviewer {
    policy: ApprovalPolicy,       // 风险阈值：自动放行 ≤ Medium；High 需用户授权
    model: Arc<dyn ModelProvider>, // 审批子代理用轻量模型
    max_auto_approve_per_run: u32, // 每 run 自动放行上限（默认 20，防失控）
}

impl AutoReviewer {
    /// 收到 tool call → 评估风险 → auto-approve / 请求用户 / deny
    pub async fn review(&self, call: &ToolCall, context: &RunContext) -> ReviewDecision;
}
pub enum ReviewDecision { AutoApprove, AskUser { reason }, Deny { reason } }
```

- **触发**：仅 Loop 自动化（§17.2）启用 auto-review；普通对话保持 §10.10 人工审批（信任阶梯：先人工后自动）。
- **护栏**：`max_auto_approve_per_run` 硬上限；写操作（write_file/edit_file/run_command）默认不自动放行；deny 记录入 trace（对齐审计日志）。
- **配置**（注册表）：`loop.auto_review.enabled`（默认关）、`loop.auto_review.max_auto_approve`（默认 20）。
- **验收**：Loop 中低风险工具（read_file 等）自动放行不打断；High 风险（run_command）仍请求用户；超上限后全部转人工。

#### 19.3.5 轨迹级监控（单动作 → 整条轨迹 + 可暂停会话）

**问题**：OpenAI 长时模型教训——**单动作安全 ≠ 轨迹安全**（token 拆两半绕过扫描器；每个 step 单独看都合法，序列指向越权结果）。现有 §10.13 按 trace 事后落库，无运行时轨迹监控与暂停能力。

**设计**（`trace.rs` 扩展 + 会话暂停链路复用 §7.4 取消令牌）：

```rust
pub struct TrajectoryMonitor {
    checks: Vec<TrajectoryCheck>,      // 规则：如「敏感凭据拼接检测」「越权访问模式」
    on_suspicious: SuspiciousHandler,  // 默认：暂停会话 + 通知前端 + 等用户确认
}

pub enum SuspiciousAction { PauseAndConfirm, LogOnly }
```

- **检测点**（对齐 §10.12 护栏既有 8 模式，新增轨迹级）：① 凭据/令牌拆分拼接重建；② 多次同源失败后换路径（沙箱探索）；③ 访问范围外系统（对齐两家 cyber-eval 教训：模型攻击了 prompt 声称"模拟"的真实系统）。
- **动作**：默认 `PauseAndConfirm`——`session:pause` 暂停当前 turn（复用取消令牌），发 `session:trajectory-alert {reason, evidence}`，用户选择继续/停止；`LogOnly` 供低风险模式。
- **配置**（注册表）：`guardrail.trajectory_monitor.enabled`（默认关，仅长时任务/自动化开）。
- **验收**：构造"步骤各自合法、序列越权"轨迹，monitor 触发暂停并附证据；用户可继续（误报恢复）或停止。

#### 19.3.6 指令渐进披露（AGENTS.md 目录化 + 记忆分片注入）

**问题**：OpenAI harness-engineering 实测——单文件巨型指令 4 种失败（挤占上下文/全重要=全不重要/瞬间腐烂/难验证）；正解 = **AGENTS.md 当目录（~100 行）+ docs/ 分片 + CI 校验**。现有 §10.7 记忆注入为分层注入，但指令注入是单文件堆叠式。

**设计**（`core/adk/prompt.rs` PromptBuilder 指令注入升级）：

```
指令注入（每会话固定顺序，前缀稳定保缓存）：
1. 核心指令（~100 行内）：工作原则 + 目录指针（到 docs/ 分片）
2. 按任务分片：仅注入当前任务命中的分片（§10.14 Router BM25 复用）
3. 记忆注入（现有 §10.7 分层 global/projects/sessions）
4. environment_context（对齐 Codex：cwd/shell/平台）
```

- **分片约定**：项目级 `docs/AGENTS/` 目录（AGENTS.md = 目录；按功能分片文件如 `docs/AGENTS/rag.md`、`docs/AGENTS/safety.md`），**CI 校验**（build.yml test job 检查：目录存在、目录链接可解析、无 >100 行单文件）。
- **配置**：`agent.instructions.mode`（single|progressive，默认 progressive）；single 保留旧行为兼容。
- **验收**：progressive 模式下长指令库仅注入目录 + 命中分片，prompt 体积显著小于 single；CI 对坏目录链接失败。

#### 19.3.7 评测捆绑体记录（harness 设置随报告落库）

**问题**：ARC-AGI-3 教训——**eval 测的是「模型 + harness + 设置」捆绑体**（同模型官方 harness 13.3% vs 调优 harness 38.3%）。现有 §10.2.5 评测报告落库 rag_eval_reports（019 迁移）未记录 harness 设置，横向对比会误读为模型能力差。

**设计**（`019_rag_eval_reports` 扩展，迁移 025）：

```sql
-- 025_eval_harness_meta.sql
ALTER TABLE rag_eval_reports ADD COLUMN harness_meta TEXT;  -- JSON：{compact_strategy, top_k, vector_weight, rerank_enabled, hyde_enabled, model}
```

- `rag_eval` 命令写入时自动附带 harness_meta（从设置注册表读取当前值）；`rag_eval_report` 展示时一并呈现。
- **验收**：两次相同用例不同 harness 设置跑出的报告，可凭 harness_meta 区分归因；趋势对比页展示 harness 设置差异。

#### 19.3.8 评测用例质量审计

**问题**：SWE-Bench Pro 审计——**~30% 任务损坏**（过严测试/欠明确 prompt/低覆盖测试/误导 prompt）。现有 §10.2.5 有 eval 用例（rag_eval_cases）但无质量审计环节，坏用例会污染评测结果。

**设计**（`rag/eval.rs` 扩展 + 新命令）：

```rust
pub struct CaseAuditor {
    /// 逐用例审查：prompt 是否明确、测试是否覆盖功能而非实现细节、
    /// 是否可被非预期方式通过（低覆盖）、是否误导
    pub async fn audit(&self, case_id: &str) -> Result<CaseAuditReport, AppError>;
}
pub struct CaseAuditReport {
    pub case_id: String,
    pub verdict: AuditVerdict,     // ok | broken { categories: Vec<BrokenCategory> }
    pub reason: String,            // 审查理由（LLM 输出 + 证据）
}
```

- **命令**：`rag_eval_audit_case {case_id}`（单条）/ `rag_eval_audit_all`（批跑，结果落 `rag_eval_cases` 增列，迁移 025 一并加 `audit_verdict`）。
- **人工确认**：audit 结果默认标记 broken 的用例在 `rag_eval` 汇总中**排除**，需人工复核后才恢复（对齐 OpenAI「5 工程师标注 + 分歧升级」）。
- **验收**：构造坏用例（过严测试/误导 prompt）→ audit 标记 broken → 汇总排除；修复后复核恢复。

> **映射验证**：以上 8 项设计由 19.1/19.2 推荐提炼，与 §15-18 共同构成 Phase 4 完整设计面。评审时按 19.3.1-19.3.8 逐条核对实现（任务映射见 §21 P4-T12~T19）。

## 20. 迁移与命令补记（Phase 4）

**迁移**（编号递增，登记入总索引迁移总表）：

| 迁移 | 内容 | 定义位置 |
|------|------|---------|
| 023_web_search_cache | web_search_cache 缓存表 | §15.4 |
| 024_trace_grading | agent_traces 增 grade 列 | §17.3 |
| 025_eval_harness_meta | rag_eval_reports 增 harness_meta + rag_eval_cases 增 audit_verdict | §19.3.7-19.3.8 |

> 022_meeting_transcript_upsert 已存在（见 `gap-audit.md §2.2`：补登记进总表）。迁移 023-025 为 Phase 4 全量；可选增强（§16.3 稀疏向量 / §16.5 图片摘要）如需落库再评估 026+。

**新命令**（§8.2 追加登记）：

| 命令 | 域 | 说明 |
|------|-----|------|
| `search:config` / `search:test` | 搜索 | 读取/测试网络搜索配置 |
| `session:init` / `session:state` / `session:cleanup` | 会话 | 生命周期（§17.1） |
| `session:fork` / `session:approve` | 会话 | 线程 fork / 双向审批（§19.3.2-19.3.3） |
| `loop:start` / `loop:stop` / `loop:list` | 工作流 | Loop 自动化（§17.2） |
| `trace:grade` | 评估 | 单条轨迹评分回写（§17.3） |
| `rag_eval_audit_case` / `rag_eval_audit_all` | 评估 | 评测用例质量审计（§19.3.8） |

**新事件**：`session:state-changed` · `loop:round` · `session:item-started` / `session:item-delta` / `session:item-completed` · `session:turn-started` / `session:turn-completed` · `session:trajectory-alert`（§8.3 追加登记）。

## 21. Phase 4 任务清单

> 按依赖顺序；每个任务为最小可独立验证单元。

- [ ] P4-T1: `SearchProvider` trait + Tavily/Serper/Searxng/Noop 四实现 + `SearchService`（选择/切换/降级）— acceptance: 配置 Tavily Key 后 `search:test` 返回真实结果；无配置时返回空+提示 (covers: §15.2, §18.2)
- [ ] P4-T2: `WebSearchTool` 注册进 chat/workflow 两处 + `assess_risk` Low 分支 + 迁移 023 缓存表 — acceptance: 对话中 agent 可调用 web_search；1 小时内重复查询走缓存 (covers: §15.3-15.4; depends: P4-T1)
- [ ] P4-T3: 深度研究工作流模板核对（web_search 生效；`knowledge_lookup` 摘除或改指针）— acceptance: 预置深度研究工作流可真实搜索并产出带来源报告 (covers: §15.5; depends: P4-T2)
- [ ] P4-T4: `HydeRetriever` + hyde prompt + `rag.hyde` 开关 — acceptance: `rag.hyde.enabled=true` 时检索结果含 HyDE 路；关闭后行为不变 (covers: §16.1, §18.3; depends: P4-T3)
- [ ] P4-T5: `multi_path_search`（A/B/C 三路 `tokio::join!`）+ RRF 融合 + 网络路触发条件 + `rag.rrf` 配置 — acceptance: 三路并发返回 RRF 融合结果；任一 路失败不影响整体 (covers: §16.2, §18.3; depends: P4-T1, P4-T4)
- [ ] P4-T6: `cliff_cutoff` 断崖截断接入 rerank 链路 + 配置 — acceptance: 构造含断崖分数的用例，截断点符合规则；至少保留 3 条 (covers: §16.4, §18.3; depends: P4-T5)
- [ ] P4-T7: `insert_document_with_meta` 幂等（指纹比对/变更重入/跳过）— acceptance: 同路径同指纹重复导入不产生重复 chunk；指纹变化触发重入库 (covers: §16.6)
- [ ] P4-T8: 会话状态机（INIT/VERIFY/CLEAN-STATE）+ `session:init/state/cleanup` + 事件 + 前端状态徽标 — acceptance: 会话切换走 init 校验；异常中断会话下次打开提示未正常结束 (covers: §17.1, §18.4; depends: P4-T3)
- [ ] P4-T9: `AgentLoop`（Goal/Timer/Maker-Checker）+ `loop:start/stop/list` + `loop:round` 事件 + 前端自动化页签 — acceptance: Goal 循环未达标自动重试至 max_rounds；Maker-Checker 不通过带评审意见重做 (covers: §17.2, §18.5; depends: P4-T8)
- [ ] P4-T10: 迁移 024 + trace grading 回写 + `trace:grade` + 前端轨迹回放/过滤 — acceptance: 评分后 agent_traces 可查 grade 列；轨迹回放展示 tool 调用链 (covers: §17.3, §18.6; depends: P4-T8)
- [ ] P4-T11: 文档收尾——README 矩阵/总索引迁移总表/CHANGELOG 更新（含 022 补登记）— acceptance: 章节矩阵含 Phase 4 行，迁移总表 001-025 完整 (covers: `gap-audit.md` §5)
- [ ] P4-T12: `Compactor`（summarize 策略 + keep_reasoning）+ 注册表配置 + 失败降级 Truncate — acceptance: 超阈值长会话 summarize 续聊保留未完成目标；truncate 丢失目标；LLM 失败降级不阻断 (covers: §19.3.1; depends: P4-T8)
- [ ] P4-T13: 会话三原语 Item/Turn/Thread（含 fork）+ `session:item-*`/`session:turn-*` 事件 + 前端按 item 渲染 — acceptance: 断线重连按 item 增量重建时间线；fork 新会话历史完整 (covers: §19.3.2; depends: P4-T8)
- [ ] P4-T14: 双向审批 `session:approve` + turn awaiting_approval 暂停/超时 deny — acceptance: 审批等待时 turn 卡住流式暂停；allow 继续 deny 回退 (covers: §19.3.3; depends: P4-T13)
- [ ] P4-T15: `AutoReviewer` 审批子代理（低风险自动放行 + 每 run 上限 + High 转人工）— acceptance: Loop 中 read_file 自动放行；run_command 请求用户；超上限转人工 (covers: §19.3.4; depends: P4-T9, P4-T14)
- [ ] P4-T16: `TrajectoryMonitor`（凭据拼接/越权/沙箱探索检测 + PauseAndConfirm）+ `session:trajectory-alert` — acceptance: 构造合法步骤+越权序列轨迹触发暂停并附证据；误报可继续 (covers: §19.3.5; depends: P4-T8, P4-T14)
- [ ] P4-T17: 指令渐进披露（docs/AGENTS/ 目录化 + Router 分片注入 + CI 校验 + 配置开关）— acceptance: progressive 注入体积显著小于 single；CI 对坏目录链接失败 (covers: §19.3.6; depends: P4-T9)
- [ ] P4-T18: 迁移 025 + harness_meta 随 rag_eval 落库 + 报告展示 — acceptance: 相同用例不同 harness 设置可凭 harness_meta 区分归因 (covers: §19.3.7; depends: P4-T5)
- [ ] P4-T19: `CaseAuditor` + `rag_eval_audit_case/all` + broken 用例排除/复核恢复 — acceptance: 坏用例 audit 标记 broken 并从汇总排除；修复后复核恢复 (covers: §19.3.8; depends: P4-T18)
