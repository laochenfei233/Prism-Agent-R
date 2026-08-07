---
feature: settings-center
status: designed
updated: 2026-08-07
branch: feat/settings-center
---

# 设置中心重构 — 全量可配置项整理与重新设计

## Report

## [S1] Problem

设置页（`src/routes/settings/+page.svelte`）目前只覆盖 7 组功能（Provider / 模型 / Agent / MCP / 技能 / Market / 记忆索引），而项目的可配置项远不止这些：

- **硬编码默认值无 UI**：反思（ReflectionConfig）、护栏（GuardrailPipeline max_chars=100_000）、Token 预算（chat.rs `with_token_budget(100_000)`）、RAG 分块（chunk_size=1000/overlap=200）、RAG 检索 top_k=5、混合检索权重（0.7/0.3）、trace 保留（1000 条）、Agent 默认温度/最大 token（0.7/8192）等均写死在代码里，用户无法调整。
- **仅有 IPC 命令无 UI**：RAG 嵌入/contextual/rerank 配置、翻译专用模型、项目索引开关、工作区绑定、ASR 配置、TTS 语言/语速等命令已存在但设置页不展示。
- **preferences 表零散读写**：各服务各自实现 get/set SQL（workspace.rs / rag_service.rs / project_index.rs / tts_service.rs / translate_service.rs 各有重复代码），无统一入口、无类型化读取、无默认值回退封装。
- **部分配置实际未被消费**：`TokenBudget` 结构体定义了 14 个字段但 chat.rs 只用了单一数值 `with_token_budget(100_000)`；`ReflectionConfig` 未在 chat.rs 启用（`with_reflection` 从未调用）。

目标：**整理全部可配置设置项**，后端统一到 preferences 存储（注册表驱动 + 通用读写命令 + 类型化读取），前端设置页按「模型服务 / Agent / 记忆 / 工具 / RAG / 会议 / 安全 / 高级」八组重新设计，全覆盖展示与修改。

## [S2] Design

### S2.1 后端：设置注册表 + 通用读写命令

新增 `src-tauri/src/commands/settings.rs` 通用命令（lib.rs 注册）：

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `settings_get_all` | — | `Vec<SettingSpecDto>` | 返回全部已注册设置项：key / label / group / kind / default / value（当前值）/ description / options / min / max / step |
| `settings_set` | `{ key, value }`（value 为 JSON） | `SettingSpecDto` | 写 preferences（类型校验 + 更新 updated_at），返回更新后的项 |

新增 `src-tauri/src/data/settings/registry.rs`：静态注册表 `pub fn specs() -> Vec<SettingSpec>`，定义每项：

```rust
pub struct SettingSpec {
    pub key: &'static str,        // preferences 键，如 "rag.chunk_size"
    pub label: &'static str,      // 中文标签
    pub group: SettingGroup,      // ModelService | Agent | Memory | Tools | Rag | Meeting | Security | Advanced
    pub kind: SettingKind,        // Bool | Int | Float | String | Select
    pub default: SettingValue,    // 默认值（JSON）
    pub description: &'static str,
    pub options: Option<Vec<&'static str>>, // Select 选项
    pub min: Option<f64>, pub max: Option<f64>, pub step: Option<f64>,
}
```

新增 `src-tauri/src/data/settings/prefs.rs` 类型化读取辅助（消除各服务重复 SQL）：

```rust
pub async fn get_str(db, key, default) -> String
pub async fn get_bool(db, key, default) -> bool
pub async fn get_i64(db, key, default) -> i64
pub async fn get_f64(db, key, default) -> f64
pub async fn set(db, key, value) -> Result<(), AppError>   // INSERT OR REPLACE
pub async fn remove(db, key) -> Result<(), AppError>       // DELETE
```

### S2.2 后端：注册表清单（全量可配置项）

| key | 分组 | kind | 默认 | 说明 |
|-----|------|------|------|------|
| `agent.default.temperature` | Agent | Float (0–2, step 0.1) | 0.7 | 新建 Agent 默认温度 |
| `agent.default.max_tokens` | Agent | Int (256–128000) | 8192 | 新建 Agent 默认最大输出 token |
| `reflection.enabled` | Agent | Bool | false | 反思循环总开关（启用后 chat.rs 接线 `with_reflection`） |
| `reflection.max_iterations` | Agent | Int (1–10) | 3 | 反思最大轮数 |
| `goal.achieved_threshold` | Agent | Float (0–1) | 0.8 | 目标达成阈值 |
| `rag.chunk_size` | Rag | Int (200–2000, step 100) | 1000 | RAG 分块大小（字符） |
| `rag.chunk_overlap` | Rag | Int (0–500, step 50) | 200 | RAG 分块重叠 |
| `rag.top_k` | Rag | Int (1–20) | 5 | 检索返回条数默认值 |
| `rag.vector_weight` | Rag | Float (0–1, step 0.1) | 0.7 | 混合检索向量权重（BM25 = 1 − 该项） |
| `guardrail.max_chars` | Security | Int (1000–1000000) | 100000 | 输入长度限制阈值 |
| `guardrail.injection_enabled` | Security | Bool | true | 注入检测开关 |
| `token_budget.chat` | Advanced | Int (10000–500000) | 100000 | 对话 Token 预算（工具输出裁剪阈值） |
| `trace.retain` | Advanced | Int (100–10000) | 1000 | Agent 轨迹保留条数 |
| `meeting.audio_buffer_secs` | Advanced | Int (10–120) | 30 | 会议音频缓冲保留时长（秒） |
| `tts.lang` | Meeting | String | zh-CN | TTS 播报语言 |
| `tts.rate` | Meeting | Float (0.5–2, step 0.1) | 1.0 | TTS 播报语速 |
| `translate.model_id` | ModelService | String（空=默认模型） | "" | 翻译专用模型 ID（前端调 translate_model_config，不入注册表） |
| `workspace.current_dir` | Advanced | String | （进程目录） | 当前工作区目录（前端调 workspace_get/set，含 recent_dirs 维护，不入注册表） |
| `project_index.enabled` | Advanced | Bool | true | 项目自动索引开关（前端调 project_index_toggle，不入注册表） |

已有专有命令的设置项（`rag.embedding.*`、`rag.contextual`、`rag.rerank`）保留专用命令不迁移，注册表仅登记展示项由前端调用既有命令（见 T2.3）。

### S2.3 后端：硬编码消费点接线（读 preferences 回退默认）

- `chat.rs:205` 护栏：`GuardrailPipeline::default_input()` → 从 prefs 读 `guardrail.max_chars` / `guardrail.injection_enabled` 构建（注入检测可关、长度阈值可配）。
- `chat.rs:212` Token 预算：`with_token_budget(100_000)` → 读 `token_budget.chat`。
- `chat.rs` 反思：`reflection.enabled` 为 true 时 `with_reflection(ReflectionConfig::from_prefs(...))`，否则不接（保持现状默认不启用）。
- `rag_service.rs` / `contextualize.rs` 分块：`chunk_text(text, chunk_size, overlap)` 调用处 → 读 `rag.chunk_size` / `rag.chunk_overlap`。
- `rag.rs:32` 与 eval：`top_k.unwrap_or(5)` → 读 `rag.top_k`。
- `search.rs:75` 混合权重：硬编码 0.7/0.3 → 读 `rag.vector_weight`（BM25 = 1 − w）。
- `trace_service.rs:63` 保留条数 1000 → 读 `trace.retain`。
- `agent.rs agent_create` 默认 temperature/max_tokens → 读 `agent.default.temperature` / `agent.default.max_tokens`。
- `tts_service.rs voices_status` 已读 `tts.lang`/`tts.rate`（保持），`settings_set` 写入后即生效。
- `translate_service.rs` 已读 `translate.model_id`（保持），经通用命令写入。

原则：**读 preferences 无记录时回退现有硬编码默认值**，不改动行为；每个消费点最多加 1 次类型化读取。

### S2.4 前端：设置页按八组重新设计

`src/routes/settings/+page.svelte` 重构：

- **布局**：左侧分组导航（八组 + 图标），右侧内容区；移动端降级为分组 tab 或折叠。复用 base 组件（Tabs / Switch / Input / Select / Slider / Button）。
- **注册表驱动渲染**：`settings_get_all()` 一次拉取全部项，按 `group` 分组渲染；kind → 组件映射：Bool→Switch、Int→Input(number)/Slider、Float→Slider、String→Input、Select→Select。保存调用 `settings_set(key, value)`。
- **保留并归组现有管理块**：
  - 模型服务：Provider 增删/改 Key、模型添加/默认标记（现状逻辑保留）
  - 工具：MCP 服务器增删测、技能安装/卸载、Skill Market
  - 记忆：重建索引按钮 + 说明
  - 会议：ASR 配置管理（asr_list_configs / save / delete / 测试）
  - 高级：翻译模型选择（translate_model_config）、RAG 嵌入配置（rag_embedding_config 模式/模型/维度）、RAG contextual/rerank 开关、项目索引开关（project_index_toggle）
- **即时反馈**：保存后 toast 提示；数值项范围校验（min/max/step 由注册表下发）。
- **样式**：沿用现有 Apple Design 令牌（`--color-*`），分组卡片 + 分隔线，不复刻新设计语言。

### S2.5 不做的事（Out of Scope）

- **不展开 TokenBudget 14 字段**：结构体未被消费，仅保留 `token_budget.chat` 单值。
- **不迁移 providers/mcp/asr 等既有表结构**：仅统一 preferences 键值读写。
- **不新建迁移**：preferences 表已存在（005），新增键无 schema 变更。
- **不做云端同步/多配置档位**：单份全局设置。
- **不迁移 `rag.embedding.*` / `rag.contextual` / `rag.rerank` 专有命令**：保留原命令，前端透传。

## [S3] Out of Scope

- TokenBudget 结构体其余 13 字段（未消费，不建 UI）
- 每 Agent 级设置（temperature/disabled_tools 等属 agent_update 既有能力，不进全局设置页）
- preferences 存量数据迁移（无 schema 变更）
- 多配置档位 / 配置导入导出

## Tasks

- [ ] T1: 后端注册表 + prefs 类型化读取 + `settings_get_all`/`settings_set` 命令并注册 — acceptance: `cargo test` 通过，新增 registry/prefs 单测；`cargo check` 零警告 (covers: S2.1)
- [ ] T2: 硬编码消费点接线（护栏/Token 预算/反思/RAG 分块/top_k/混合权重/trace 保留/Agent 默认值）— acceptance: 各消费点读 preferences 回退默认；`cargo test` 通过 (covers: S2.2, S2.3; depends: T1)
- [ ] T3: 前端设置页八组重构 + 注册表驱动渲染 + 既有管理块归组 — acceptance: `svelte-check` 无新增错误；设置页可读全部注册项、修改后刷新仍在 (covers: S2.4; depends: T1, T2)
- [ ] T4: 验证与评审 — acceptance: `npm run check` 0 error；`cargo test` 全绿；评审通过 (covers: S2.1-S2.4; depends: T3)
