---
feature: settings-model-manage
status: delivered
updated: 2026-08-10
branch: feat/settings-model-manage
commits: 1611eea..<head-sha>
---

# 模型管理区块 Cherry Studio 风格重做

## Report

**What was built** — 设置页「Provider」详情中的模型管理区块按 Cherry Studio 风格重做：模型行新增能力徽标（chat=对话 / embedding=嵌入 / vision=视觉 / asr=语音，颜色走 tokens.css 语义色）、「设默认」与「删除」操作按钮（删除带 confirm 确认）；添加模型区域新增「预置模型库」下拉（按 provider kind 分类的静态常量，选择后填充模型 ID 输入框）。后端新增 `model_delete` 与 `model_set_default` 命令（下沉至 `ModelService::delete` / `ModelService::set_default`，事务内清除同 provider 其他模型的默认标记后置目标为默认，未知 id 报错），并在 lib.rs 注册。

**Verification** — `cargo check` PASS；`cargo test --lib` 130 passed / 0 failed（含 3 个新增 model_service 测试：set_default 清同级置目标且不影响其他 provider、未知 id 报错、delete 移除）；`npm run check`（svelte-check）0 errors / 0 warnings。首轮 reviewer 发现 `model_set_default` 引用 models 表不存在的 `updated_at` 列（critical），已修复并补测试覆盖，复审查确认解决且无新问题。

**Journey log** —
- critical bug 根因：models 表（001_init.sql）无 `updated_at` 列，但首版 UPDATE 引用了它；`cargo check` 无法捕获（sqlx 查询为运行时字符串），靠 reviewer 代码审阅发现。教训：SQL 字符串列名变更需对照迁移 schema。
- 命令层（settings.rs）无测试先例，且 AppState 构造复杂；将逻辑下沉至 `ModelService` 后测试成本大降，与代码库「service 层承载业务逻辑」约定一致。
- 测试需先插入 provider 行以满足 models.provider_id 外键约束（首个版本 2 个测试因此失败）。

## [S1] Problem

设置页「Provider」区块已具备 Cherry Studio 基础布局（左 Provider 列表 + 右详情），但模型管理部分简陋：模型添加只能手输 model ID 或拉取后选择，无预置模型库；模型列表只显示名称 + 默认徽标，无法删除、无法设默认、无能力类型（chat/embedding/vision）标识。

## [S2] Design

### 后端命令（settings.rs 追加，逻辑下沉 ModelService）

| 命令 | 入参 | 行为 |
|------|------|------|
| `model_delete` | `id: String` | 删除 models 表记录（`DELETE FROM models WHERE id = ?`） |
| `model_set_default` | `id: String` | 事务内先将该 provider 下全部模型 `is_default=0`，再将该 id 置 1；未知 id 报错 |

- 注册点：`lib.rs` invoke_handler（对齐 `model_list` / `model_providers` 所在位置）。
- 删除默认模型后，默认模型变为空（`get_default_model` 返回 None），前端列表无默认徽标，属预期行为。
- 注意：models 表无 `updated_at` 列（001_init.sql），UPDATE 不得引用它。

### 前端（settings/+page.svelte providers 区块）

保留现有左 Provider 列表 + 右详情布局，重做「模型」卡片：

1. **添加模型**：表单行保留 Provider 选择 + 模型 ID 输入 + 拉取 + 添加（现有）；新增「预置模型库」下拉（静态常量 `PRESET_MODELS`，按 provider kind 分类，选择后经 `applyPreset` 填充模型 ID 输入框）。
2. **模型列表行**（每行）：
   - 左侧：模型名（display_name || model_id）+ 类型徽标（kind：chat 绿 / embedding 蓝（accent）/ vision 紫 / asr 橙，走 tokens.css 语义色）+ 默认徽标（现有）
   - 右侧：操作按钮组——「设默认」（仅当 `!m.is_default`）、「删除」（`confirm` 确认后调 `model_delete`）
3. 操作后刷新：`models = await invoke('model_list')`。

### 设计约束

- 不引入新色板/依赖，颜色走 tokens.css 既有语义色；组件用现有 base 按钮/徽标样式。
- 预置模型库为前端静态常量（不加后端配置），仅填充输入框，不自动保存。

## [S3] Out of Scope

- 不重做 Provider 列表本身（图标/状态开关/Base URL 编辑留待后续）。
- 不做模型能力自动探测（kind 由添加时选择/默认 'chat'，本阶段不新增 kind 编辑 UI）。
- 不做批量删除/多选。

## Tasks
- [x] T1: 后端 `model_delete` / `model_set_default` 命令（ModelService::delete/set_default）+ lib.rs 注册 — 验收：命令可调用且行为正确（删除后列表消失；设默认后该 provider 仅此模型为默认） (covers: S2)
- [x] T2: 前端模型列表行加能力徽标 + 设默认/删除按钮 — 验收：列表行显示 kind 徽标与操作按钮，设默认/删除后刷新生效 (covers: S2)
- [x] T3: 前端预置模型库下拉（按 kind 分类静态常量） — 验收：选择预置项填充 model ID 输入框 (covers: S2)
- [x] T4: 验证 — 验收：`cargo check` + `npm run check` 通过 (covers: S2)
