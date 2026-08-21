---
feature: engineering-polish
status: in-progress
updated: 2026-08-21
branch: feat/eng-hardening
commits: # filled at delivery
---

# 工程化改造（商业软件化）

## Report

## [S1] Problem

项目功能已覆盖 Phase 1-5（核心对话、RAG、会议/翻译/OCR、Agent 编排、生产加固），但仓库层面缺少商业软件的工程化标配，观感停留在「玩具/个人项目」：

1. **无门面**：无根 README、无 LICENSE、无 CHANGELOG、无 CONTRIBUTING —— 仓库第一眼不像正式产品
2. **无质量门禁**：前端无 eslint/prettier/.editorconfig；CI 无 clippy/fmt 检查，代码风格无强制约束
3. **前端零测试**：`src/lib` 全组件无单测/组件测试，回归只能靠手动
4. **无发版流程**：`private: true`、版本冻结 0.1.0，无自动版本号/CHANGELOG/Release 产物发布

## [S2] Design

### 门面工程（S2.1）

- **README.md**（根目录，英文为主 + 中文要点）：项目定位、功能特性清单（对齐 docs/design 五阶段）、架构概览（Tauri 2 + SvelteKit 5 + Rust core）、快速开始（npm install + tauri dev）、构建/测试命令、文档索引（docs/design、docs/compose/specs）、徽章占位（CI/版本）
- **LICENSE**：MIT，2026，作者名用 GitHub 用户名 chenfei 的实际显示名？→ 采用 MIT + `Chenfei`，占位符不引入
- **CHANGELOG.md**：手写初始版本，后续由 semantic-release 自动追加
- **CONTRIBUTING.md**：开发环境、代码规范（commit 规范 → 对齐 semantic-release）、测试要求、PR 流程

### 质量门禁（S2.2）

- **eslint**：flat config（`eslint.config.js`），svelte 插件 + typescript-eslint，script `lint`（`eslint .`）
- **prettier**：`.prettierrc`（svelte 插件），script `format` / `format:check`
- **.editorconfig**：基础缩进/换行约定
- **CI 增强**（`.github/workflows/build.yml` test job）：
  - `cargo fmt --check`（Rust 格式门禁）
  - `cargo clippy -- -D warnings`（Rust lint 门禁）
  - `npm run lint`、`npm run format:check`
  - `npm test`（vitest run，见 S2.3）
- **前端存量 lint 问题**：新引入 eslint 后存量代码必有告警；策略 = 首次运行将存量报错全部修复（代码量小，~90 个 svelte 文件），不留 baseline 豁免

### 前端测试（S2.3）

- **vitest + @testing-library/svelte + jsdom**（svelte 5 兼容）：`vitest.config.ts` 独立于 vite.config.ts（避免污染 tauri 构建）
- script `test`: `vitest run`；CI 接入
- **首批测试对象**（选低耦合、纯逻辑优先）：
  - base 组件：Button、Badge、Switch、Input（渲染 + 交互 + 事件）
  - 纯逻辑：`src/lib/api/client.ts` 的事件解析/格式化函数（若有可测纯函数）
  - store：`src/lib/stores/`（若有）—— 以实际文件为准
- 测试规范：真实渲染组件，不 mock 实现；`svelte-check` 覆盖类型

### 发版流程（S2.4）

- **semantic-release**（GitHub Actions）：`feat`→minor、`fix`→patch、BREAKING→major
- 配置：`release.config.mjs`（branches: master/main、plugins: conventionalcommits + changelog + git + github）
- 新增 workflow `.github/workflows/release.yml`：push 到 master/main 时运行（test 通过后）
- **package.json 调整**：`private` 保留 `true`（桌面应用，不发布 npm），semantic-release 仅打 git tag + GitHub Release（附构建产物）
- **commit 规范**：CONTRIBUTING 声明 Conventional Commits（项目现有 commit 已基本符合 `feat(...)`/`fix(...)`/`refactor(...)` 格式）
- 首次发布：CI 对 master 首次提交打 `v1.0.0`

## [S3] Out of Scope

- Agent 能力测试扩展（已有 `capability_test.rs` 6 用例，无匹配 skill，用户确认不做）
- 代码覆盖率门槛（本轮只建测试框架与首批测试，不设 % 门槛）
- Docker 化 / 沙箱运行环境（桌面应用，交付格式为安装包）
- 双因素签名 / 公证（Apple notarization、Windows 签名后续单独处理）
- 分支保护硬性配置（GitHub 设置项，非代码，仅文档建议）

## Tasks

- [x] T1: 门面文件（README/LICENSE/CHANGELOG/CONTRIBUTING） — acceptance: 四个文件存在且内容完整，README 覆盖定位/特性/快速开始/文档索引 (covers: S2.1)
- [x] T2: 前端质量门禁（eslint/prettier/.editorconfig + scripts） — acceptance: `npm run lint` 与 `npm run format:check` 零报错 (covers: S2.2)
- [x] T3: 存量代码 lint/format 修复 — acceptance: eslint/prettier 全量通过，无 baseline 豁免 (covers: S2.2; depends: T2)
- [x] T4: Rust 质量门禁（fmt/clippy 进 CI） — acceptance: 本地 `cargo fmt --check` 与 `cargo clippy -D warnings` 通过 (covers: S2.2)
- [x] T5: vitest 测试框架接入 — acceptance: `npm test` 跑通示例测试，CI 含 test 步骤 (covers: S2.3)
- [x] T6: 首批前端测试（base 组件 + 逻辑/store） — acceptance: ≥6 个真实渲染测试用例全部通过 (covers: S2.3; depends: T5)
- [x] T7: semantic-release 配置 + release workflow — acceptance: 本地 dry-run 通过，workflow 文件就位 (covers: S2.4)
- [ ] T8: CI 全链路验证 — acceptance: 本地模拟 CI 各步骤（test/lint/format/clippy/fmt）全部通过 (covers: S2.2, S2.3, S2.4; depends: T3, T4, T6, T7)
