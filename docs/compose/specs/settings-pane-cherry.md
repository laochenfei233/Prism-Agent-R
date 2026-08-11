---
feature: settings-pane-cherry
status: delivered
updated: 2026-08-10
branch: feat/settings-pane-cherry
commits: 5ccd39c..<head-sha>
---

# 设置页中间栏 Cherry Studio 风格重做

## Report

**What was built** — 设置页「Provider & 模型」区块的中间栏（`provider-list-pane`）按 Cherry Studio 风格重做：宽度 160→200px，顶部新增搜索框（按 name/kind 大小写不敏感实时过滤，含「无匹配服务商」空态），列表行新增首字符头像色块（`providerColor(kind)` 映射 tokens 语义色）+ 状态指示点（`is_enabled` 绿/灰），「添加服务商」按钮移至 pane 底部固定（虚线边框），选中行加左侧 3px accent 指示条。添加 Provider 表单新增 7 项供应商预设库（OpenAI/Anthropic/Google Gemini/阿里云百炼/Xiaomi MiMo/Ollama/自定义，选中自动填充名称与 Base URL）+ 预设 chip 选择器。ASR 区块中间栏同步头像+状态点样式。

**Verification** — `npm run check`（svelte-check）0 errors / 0 warnings PASS；tokens.css 确认 `--color-red`/`--color-purple` 等色值存在。独立 reviewer 确认 5 项验收全部满足，无 critical 发现（3 个 non-critical：过滤谓词可提取 $derived、ASR 头像用默认灰、过滤后选中项仍显示详情——均为可选优化，未改动）。

**Journey log** —
- 中间栏改造纯前端：ProviderDto 已暴露 `is_enabled`/`kind`，状态点与头像无需后端改动。
- spec 初写误落到上一分支 worktree（settings-model-manage），定位后移动到正确 worktree。
- 响应式断点中 pane 宽度覆盖（原 140px）需同步调整，否则窄屏下仍为旧宽。

## [S1] Problem

设置页「Provider & 模型」区块的中间栏（`provider-list-pane`，位于左侧分类导航旁）仅 160px 宽，只有 Provider 名称 + kind 文本，缺少 Cherry Studio 中间栏的关键元素：搜索框、Provider 图标/状态指示点、底部固定添加按钮、选中态指示。添加 Provider 表单只有 openai/ollama 两种，无法覆盖常见服务商。ASR 区块共用同一 pane 样式，同样简陋。

## [S2] Design

### 供应商预设库（前端静态常量）

`PROVIDER_PRESETS: { kind, name, baseUrl }[]`，覆盖常见服务商（Cherry Studio 风格）：

| kind | name | baseUrl |
|------|------|---------|
| openai | OpenAI | https://api.openai.com/v1 |
| anthropic | Anthropic | https://api.anthropic.com |
| google | Google Gemini | https://generativelanguage.googleapis.com/v1beta |
| dashscope | 阿里云百炼 | https://dashscope.aliyuncs.com/compatible-mode/v1 |
| mimo | Xiaomi MiMo | https://api.xiaomimimo.com/v1 |
| ollama | Ollama | http://localhost:11434/v1 |
| custom | 自定义 | （空） |

- 添加 Provider 表单：Provider 类型下拉改为选择预设（选中自动填充 name/base_url）；kind 仍为下拉（预设 kind）。
- 配色映射 `providerColor(kind)`：openai=绿、anthropic=橙、google=蓝(accent)、dashscope=紫、mimo=红、ollama=灰、custom=默认。

### 中间栏（provider-list-pane）重做

1. **宽度**：160px → 200px（对齐 Cherry 中间栏比例；响应式断点同步 170px）。
2. **顶部搜索框**：`providerFilter` state，实时过滤 Provider 列表（按 name/kind 匹配）。
3. **列表行**：
   - 头像色块：provider 名称首字符 + `providerColor(kind)` 背景（Cherry 图标位）。
   - 名称：ellipsis。
   - 状态点：`is_enabled` 绿 / 灰（Cherry：绿=可用/灰=未配置）。
4. **底部固定「+ 添加服务商」**：从顶部移到 pane 底部固定（不随列表滚动）。
5. **选中态增强**：active 行加左侧 3px 指示条（accent 色）+ 背景。

### ASR 中间栏同步

- 复用新的 `provider-list-pane` 样式：行加头像色块 + 状态点（始终绿，视为可用后端）+ 名称 + languages 徽标。

### 数据源说明

- ProviderDto 已含 `is_enabled`（bool），状态点直接使用，无需后端改动。
- 搜索/预设库/配色均为前端实现，不动后端。

## [S3] Out of Scope

- 不做 Provider 图标真 Logo（仅首字符色块）。
- 不做 Provider 删除/启停开关（沿用现有：删除在 Agent 侧，Provider 无删除入口，保持现状）。
- 不改右侧详情布局（模型管理已在上一分支完成）。

## Tasks
- [x] T1: 供应商预设库常量 + providerColor 配色 + 添加表单改为预设选择 — 验收：添加表单可选 7 种预设，选中填充 name/base_url (covers: S2)
- [x] T2: provider-list-pane 重做（200px/搜索框/头像+状态点/底部固定添加/选中态指示条） — 验收：pane 具备全部 Cherry 元素且过滤生效 (covers: S2)
- [x] T3: ASR 中间栏同步（头像+状态点样式） — 验收：ASR pane 行样式与 Provider 一致 (covers: S2)
- [x] T4: 验证 — 验收：`npm run check` 通过 (covers: S2)
