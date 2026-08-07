---
feature: ui-apple-design-restore
status: delivered
updated: 2026-08-07
branch: feat/ui-apple-design-restore
commits: fed8f24..5837250
---

# UI Apple Design 恢复与打磨

## Report

**What was built** — 全应用回归设计文档声明的 Apple Design 视觉语言，并打磨到高级质感。`tokens.css` 补全语义层（`--color-bg-elevated`/`--color-bg-hover`/`--color-border-strong`/`--color-focus-ring`/`--color-overlay`/`--color-muted` + 分层阴影，浅色/暗色成对），新增 `theme.svelte.ts` runes store（跟随 `prefers-color-scheme`、手动切换持久化到 `localStorage`、切换 `.dark` 类与 `colorScheme`），`app.html` 内联脚本首帧前应用主题消除暗色启动闪白。34 个文件约 250 处硬编码颜色收敛为令牌，修正多处 Apple 蓝（`rgba(0,113,227)`/`#0071E3`）误用为主题橙，黑色按钮（setup/create/run）改 accent 色，面板级表面走 `.glass` 毛玻璃或 `bg-elevated + shadow` 分层阴影。base 组件焦点环统一为 `--color-focus-ring`，全局 `*:focus-visible` 焦点环 + 双模式滚动条/选中定制。装饰性 emoji（模板图标/空态气泡/⚡）替换为 SVG，任务画布角色色板保留为语义数据色。

**Verification** — `npm run check`：0 错误 20 警告（均为既有 a11y 警告）；`npm run build`：通过；Playwright 双模式截图 + ImageMagick 像素采样确认暗色背景 `#1C1C1E`/浅色 `#F2F2F7` 正确；主题切换机制（类切换/localStorage 持久化/colorScheme 同步）实测通过；FOUC 首帧暗色实测通过。Dashboard 纯浏览器渲染 500 经 master 对照确认 pre-existing（`task.svelte.ts:68` 顶层 `$effect` 触发 `effect_orphan`，非本次改动）。impeccable detector 12 项发现均判定合法（`--spring` 为设计文档钉死的 Apple 弹性曲线；side-tab 为 blockquote/注入状态语义用途）。

**Journey log** — (1) 项目文档声明 Apple Design 但代码已漂移为 OpenAI 硬编码风，恢复而非精修是正确取舍；(2) 暗色模式从未真正接线（无主题机制），tokens 的 `.dark` 类只是死代码，需配套 store + 首帧脚本；(3) 多处 hover/focus 误用 Apple 蓝而非主题橙，是隐藏的 AI 味来源；(4) 既有组件引用未定义 token（`--duration-base`/`--space-8` 等）导致入场动画静默失效，补别名即修复；(5) 纯浏览器无法验证 Tauri 应用渲染，需 mock `__TAURI_INTERNALS__`，dashboard 崩溃为既有 `effect_orphan` 问题需后续修复。

## [S1] Problem

设计文档（`phase1-core.md §9.1`）声明的视觉方向是 **Apple Design**（毛玻璃 + 半透明 + 圆角 + iOS 18 色板 + SF Pro/PingFang 系统字体链 + 弹性动效），但实际代码在近期提交中漂移成了 "OpenAI 白底风"：

- **34 个文件硬编码十六进制颜色**（`#171717`/`#f7f7f8`/`#efefef`/`rgba(0,0,0,0.06)` 等），绕过设计令牌，暗色模式无法工作，且配色正是 AI 味模板组合（白底 + 中性灰 + 黑按钮）
- **暗色模式从未跑通**：`tokens.css` 定义了 `.dark` 类，但没有任何机制应用它（无主题切换 store、无 `prefers-color-scheme` 接入），页面硬编码白色直接破坏暗色
- **状态不完整**：大量组件缺 focus-visible 焦点环、disabled 态、按压态；hover 态多为 `opacity`/`background` 微调，无统一反馈语言
- **观感平庸**：卡片式容器、中性灰文字、默认圆角，缺少 Apple 设计应有的材料质感（毛玻璃/层次/轻阴影）与排版层次

目标：**全应用**（Dashboard / 对话 / 侧边栏 / 设置 / TaskDesigner / 基础组件）回归 Apple Design 并打磨到高级质感，**浅色 + 暗色双模式齐全**，去除 AI 味。

## [S2] Design

### 设计定调（design read）

"Reading this as: a desktop AI-agent control surface for technical users, **Operate** mode, with an Apple Design language — restrained neutral iOS 18 palette, one saturated accent (orange `#FF6900`), system font stack (SF Pro / PingFang SC), real glass materials with backdrop blur, fluid spring motion. Structure, IA, and content stay untouched; the old OpenAI-style look is anti-reference."

三档设定：`DESIGN_VARIANCE 5`（应用 UI，扫读与一致性优先）· `MOTION_INTENSITY 5`（流体 CSS 动效）· `VISUAL_DENSITY 5`（日常应用密度）。

### S2.1 设计令牌（tokens.css）补全

保留 iOS 18 色板与系统字体链（设计文档钉死），补全语义层缺口：

| 新增令牌 | 用途 |
|---------|------|
| `--color-bg-elevated` | 卡片/浮层底色（浅色 `#FFFFFF` / 暗色 `#1C1C1E` 之上再提亮） |
| `--color-bg-hover` | hover 底色（浅色 `rgba(0,0,0,0.045)` / 暗色 `rgba(255,255,255,0.08)`） |
| `--color-border-strong` | 强调边框（浅色 `rgba(0,0,0,0.16)` / 暗色 `rgba(255,255,255,0.16)`） |
| `--color-focus-ring` | 焦点环（accent 45% 透明度） |
| `--color-overlay` | 遮罩层（浅色 `rgba(0,0,0,0.32)` / 暗色 `rgba(0,0,0,0.6)`） |
| `--color-muted` | 弱化文字（介于 secondary/tertiary 之间） |
| `--shadow-sm` / `--shadow-md` / `--shadow-lg` | 分层阴影，暗色下随背景色调（非纯黑） |
| `--text-title2` 等字号别名对齐现有 alias 体系 | 排版层级统一 |

暗色模式沿用文档钉死的纯黑背景（`#000000`，Apple 夜间风格），浅色不做纯白刺眼——正文对比度 ≥ 4.5:1。

### S2.2 主题机制

新增 `src/lib/stores/theme.svelte.ts`（runes store）：

- 默认跟随系统：`matchMedia('(prefers-color-scheme: dark)')` + 变更监听
- 手动切换覆盖并持久化到 `localStorage('prism-theme')`
- 应用方式：在 `+layout.svelte` 对 `<html>` 加/删 `.dark` 类（tokens 已按 `.dark` 选择器定义）
- 侧边栏底部加入主题切换按钮（太阳/月亮图标，用 SVG 而非 emoji）

### S2.3 硬编码颜色收敛

全部 34 个含硬编码颜色的文件改为引用令牌。规则：

- 禁止在组件内出现十六进制颜色（`.dark` 覆盖场景除外，必须成对出现）
- 现有令牌无法表达的语义 → 优先补 S2.1 令牌，其次用 `color-mix()` 派生，禁止新增孤立硬编码
- TaskCanvas（60 处）是最大头：节点色板收敛为令牌 + 少量语义角色色（agent/tool/mcp/workflow），两模式成对定义

### S2.4 材料与质感

- 面板级表面（侧边栏/侧栏/对话框/Toast/命令面板）使用 `.glass` 毛玻璃（`backdrop-filter: saturate(180%) blur(20px)`），已有工具类复用
- 阴影体系：静止扁平、交互浮起（浅阴影 → hover 加深），禁止纯黑投影
- 卡片避免「通用白卡 + 边框 + 阴影」三板斧：内容区用分隔线/负空间，提升时才浮起

### S2.5 交互状态

所有可交互元素补齐四态，统一语言：

| 状态 | 规则 |
|------|------|
| hover | 背景 `--color-bg-hover` 或轻微浮起（shadow-sm → md） |
| focus-visible | 2px 焦点环 `--color-focus-ring`，圆角跟随元素 |
| active | `scale(0.96~0.98)` 按压反馈 |
| disabled | opacity 0.4 + `cursor: not-allowed`，不触发 hover |

动效纪律：只动 `transform`/`opacity`（必要时 `filter`/`backdrop-filter`），UI 动效 ≤ 300ms，iOS 弹性曲线（`--spring`），尊重 `prefers-reduced-motion`。

### S2.6 反 AI 味清单（design-core 规则核对）

- 删除 setup banner 中的 emoji（`⚡`）→ SVG 图标
- 禁止 em-dash、装饰性状态点、版本脚注等 AI tell
- 标题层级：无 eyebrow/kicker 滥用，标题靠字重与字号承载
- 每个数据承载视图补 loading/empty/error 状态（已有 EmptyState 组件复用）
- 选中文字/滚动条/焦点环等浏览器表面用色板定制（app.css 已部分完成，补齐暗色）

### S2.7 各表面改动要点

| 表面 | 改动 |
|------|------|
| `tokens.css` | S2.1 令牌 + 暗色补齐 |
| `+layout.svelte` | 侧边栏 token 化 + 主题切换按钮 + `.dark` 应用 |
| `+page.svelte`（Dashboard） | 去硬编码白底，改 token 化玻璃卡片布局，保留现有信息架构与组件结构 |
| `settings/+page.svelte` | 16 处硬编码收敛 + 表单控件统一 |
| `chat/*`（MessageList/Composer/MessageBubble/ModelSelector） | token 化 + 气泡层次 + focus 态 |
| `dashboard/*`（8 组件） | 卡片 token 化 + 阴影体系 + 空态完善 |
| `sidebar/*`（7 组件） | token 化 + 毛玻璃侧栏 |
| `task/*`（TaskDesigner/TaskCanvas 等 5 组件） | 节点色板收敛 + 画布背景 token 化 |
| `base/*`（20+ 组件） | 状态补全 + 令牌核对（Button/Input/Modal/Sheet/Toast 等） |
| `market/*`、`dialogs/*` | token 化 |

## [S3] Out of Scope

- 不改信息架构、路由、组件结构与业务逻辑（纯视觉恢复与打磨）
- 不新增功能（如主题商店、自定义色板 UI）
- 不改动 Rust 后端与 IPC
- 不迁移 oklch 色彩空间（保持 hex，文档已定）
- 不引入外部字体（系统字体链为文档钉死选择）

## Tasks

- [x] T1: 设计令牌补全 + 主题 store + `.dark` 应用机制 — acceptance: tokens.css 含 S2.1 全部令牌；切换主题按钮在侧边栏可用且持久化；`<html>` 类随系统/手动切换 (covers: S2.1, S2.2)
- [x] T2: 硬编码颜色收敛（dashboard + layout + settings） — acceptance: 上述文件无残留十六进制颜色，暗色下渲染正确 (covers: S2.3; depends: T1)
- [x] T3: 硬编码颜色收敛（chat + sidebar + market + dialogs） — acceptance: 上述文件无残留十六进制颜色，暗色下渲染正确 (covers: S2.3; depends: T1)
- [x] T4: TaskDesigner/TaskCanvas 色板收敛 — acceptance: 节点色板走令牌/语义角色色，两模式成对，无孤立硬编码 (covers: S2.3; depends: T1)
- [x] T5: 基础组件状态补全 + 材料质感（glass/阴影/focus 环） — acceptance: base/* 组件四态齐全，面板类组件使用 glass 或分层阴影 (covers: S2.4, S2.5)
- [x] T6: 反 AI 味清理 + 浏览器表面定制 — acceptance: 无 emoji/em-dash 残留；滚动条/选中/焦点环暗色正确 (covers: S2.6)
- [x] T7: 全应用验证 — acceptance: `bun run check` 通过；浅色/暗色两模式下所有页面截图核对无硬编码白/黑残留 (covers: S2.1-S2.7; depends: T1-T6)
