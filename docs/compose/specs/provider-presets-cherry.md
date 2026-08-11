---
feature: provider-presets-cherry
status: delivered
updated: 2026-08-10
branch: feat/provider-presets-cherry
commits: 19b0fd5..<head-sha>
---

# 移植 Cherry Studio 供应商预设（17 个主流 + SVG Logo）

## Report

**What was built** — 从 Cherry Studio（`packages/ui/src/components/icons/{providers,models}/*/light.tsx`）提取 16 个主流供应商 SVG Logo，转换为 Svelte 组件（`src/lib/components/icons/providers/`），新建 `ProviderLogo.svelte` 按 kind 分发（未知 kind 回退首字符色块）。`PROVIDER_PRESETS` 从 7 项扩至 17 项（新增 DeepSeek/智谱/Moonshot/豆包/MiniMax/百川/硅基流动/Groq/OpenRouter/Mistral），`PRESET_MODELS` 按供应商补充推荐模型。设置页 Provider 中间栏头像改用真实 Logo。

**Verification** — `npm run check`（svelte-check）0 errors / 0 warnings PASS；`npm run build`（vite build）PASS。首轮 reviewer 发现 critical bug：size prop 在 `{@html}` 字符串中被拼成字面量文本（`" + size + "`）而非模板插值，已修复为 `$derived` + `${size}` 并复审查确认 16/16 修复、无新问题。

**Journey log** —
- Cherry 图标是 React TSX（useId/filter/gradient），Svelte 类型系统对 SVG 属性检查不完整（mask-units/gradient-units 缺失），直接内联属性会报错；改用 `{@html}` 注入 SVG 字符串完全绕开 TS 检查。
- 转换脚本需处理：JSX 表达式展平（`fill={\`url(#x)\`}`→`fill="url(#x)"`）、useId→固定 id、camelCase→kebab-case、`{...props}` 移除；脚本用后即删。
- 坑：`{@html}` 字符串内 size 需模板插值 `${size}` 且用 `$derived` 响应式；`svelte-check` 只验类型不验字符串内容，必须靠 reviewer 人工审阅捕获运行时 bug。
- PowerShell 内嵌正则转义脆弱（`\`n` 字面化），改用临时 Node 脚本批量修正文件。

## [S1] Problem

当前 `PROVIDER_PRESETS` 仅 7 个供应商（openai/anthropic/google/dashscope/mimo/ollama/custom），`PRESET_MODELS` 模型推荐有限；中间栏头像为纯首字符色块，无真实品牌 Logo。Cherry Studio 内置 60+ 供应商与推荐模型，可借鉴其常用集合与官方 Logo。

## [S2] Design

### 供应商清单（17 个）

现有 7 个 + 新增 10 个主流：

| kind | name | baseUrl（默认） |
|------|------|----------------|
| openai | OpenAI | https://api.openai.com/v1 |
| anthropic | Anthropic | https://api.anthropic.com |
| google | Google Gemini | https://generativelanguage.googleapis.com/v1beta |
| deepseek | DeepSeek | https://api.deepseek.com |
| zhipu | 智谱 | https://open.bigmodel.cn/api/paas/v4 |
| moonshot | Moonshot AI | https://api.moonshot.cn |
| dashscope | 阿里云百炼 | https://dashscope.aliyuncs.com/compatible-mode/v1 |
| doubao | 豆包 | https://ark.cn-beijing.volces.com/api/v3 |
| minimax | MiniMax | https://api.minimaxi.com/v1 |
| baichuan | 百川 | https://api.baichuan-ai.com |
| silicon | 硅基流动 | https://api.siliconflow.cn/v1 |
| mimo | Xiaomi MiMo | https://api.xiaomimimo.com/v1 |
| groq | Groq | https://api.groq.com/openai |
| openrouter | OpenRouter | https://openrouter.ai/api/v1 |
| mistral | Mistral | https://api.mistral.ai |
| ollama | Ollama | http://localhost:11434/v1 |
| custom | 自定义 | （空） |

### 推荐模型（PRESET_MODELS 扩展）

每个供应商 3-6 个常用模型（参考 Cherry provider-registry models.json + 已有）：

| kind | 推荐模型 |
|------|---------|
| openai | gpt-4o, gpt-4o-mini, gpt-4.1, gpt-4.1-mini, o3, o4-mini, text-embedding-3-small, text-embedding-3-large |
| ollama | llama3.1, qwen2.5, qwen2.5-coder, deepseek-r1, gemma2, mistral, phi4, nomic-embed-text |
| anthropic | claude-sonnet-4-5, claude-opus-4-1, claude-haiku-4-5 |
| google | gemini-2.5-pro, gemini-2.5-flash, gemini-embedding-001 |
| dashscope | qwen-max, qwen-plus, qwen-turbo, qwen-vl-max, qwen-embedding |
| mimo | mimo-v2.5, mimo-v2.5-pro |
| deepseek | deepseek-chat, deepseek-reasoner |
| zhipu | glm-4-plus, glm-4-flash, glm-4v-plus, glm-4 |
| moonshot | kimi-k2, kimi-latest, moonshot-v1-32k, moonshot-v1-128k |
| doubao | doubao-pro-32k, doubao-lite-32k, doubao-1-5-pro, doubao-embedding |
| minimax | MiniMax-M2, MiniMax-M1, abab6.5s-chat |
| baichuan | Baichuan4, Baichuan4-Turbo |
| silicon | deepseek-ai/DeepSeek-V3, Qwen/Qwen2.5-72B-Instruct, BAAI/bge-m3 |
| groq | llama-3.3-70b-versatile, llama-3.1-8b-instant, mixtral-8x7b-32768 |
| openrouter | openai/gpt-4o, anthropic/claude-sonnet-4.5, deepseek/deepseek-chat |
| mistral | mistral-large-latest, mistral-medium-latest, mistral-small-latest |
| custom | [] |

### SVG Logo（16 个，custom 除外）

- 从 cherry-studio `packages/ui/src/components/icons/{providers,models}/*/light.tsx` 提取，React TSX → Svelte。
- 每个供应商一个组件（`src/lib/components/icons/providers/*.svelte`）：`let { size = 24 } = $props()` + `let svg = $derived(\`…\${size}…\`)` + `{@html svg}`（避 TS 属性检查）。
- 转换规则：JSX 表达式展平、useId→固定 id、camelCase→kebab-case、移除 `{...props}`。
- `ProviderLogo.svelte`：kind→组件映射，未知 kind 回退首字符色块。

### 接线点

- `settings/+page.svelte`：PROVIDER_PRESETS 17 项；PRESET_MODELS 扩展；pane-avatar 改 `<ProviderLogo kind={p.kind} />`。
- 无后端改动。

## [S3] Out of Scope

- 不移植 dark 变体（浅色 token 主题为主；后续需要再补）。
- 不移植其余 40+ 小众/代理供应商。
- 不做 Logo 自动配色探测。

## Tasks
- [x] T1: 提取 16 个供应商 SVG → Svelte 组件（icons/providers/） — 验收：每个组件可独立渲染且 path/fill 与 Cherry 一致 (covers: S2)
- [x] T2: 新建 ProviderLogo.svelte 分发组件（未知 kind 回退首字符） — 验收：kind→组件映射正确，fallback 生效 (covers: S2)
- [x] T3: PROVIDER_PRESETS 扩至 17 项 + PRESET_MODELS 扩展 — 验收：添加表单 17 预设可选，模型推荐按表填充 (covers: S2)
- [x] T4: 中间栏头像改用 ProviderLogo — 验收：pane-avatar 显示真实 Logo，未知 kind 显示首字符 (covers: S2)
- [x] T5: 验证 — 验收：`npm run check` + `npm run build` 通过 (covers: S2)
