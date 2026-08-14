---
feature: agent-capability-tests
status: delivered
updated: 2026-08-14
branch: feat/agent-capability-tests
commits: 7326ada..HEAD # 工作区未提交（base 7326ada），提交由用户决定
---

# Agent 能力检测测试套件

## Report

**What was built** — 集中式 Agent 能力检测集成测试套件 `src-tauri/tests/capability_test.rs`（6 个用例），零外部依赖（FakeProvider 驱动 RigAgent，无真实 LLM/网络/MCP）：① `conversation_roundtrip` 对话闭环（流式生成→文本→usage）；② `tool_execution_roundtrip` 工具执行闭环（echo_tool + HITL always-approve 直通，trace 含 llm_call/tool_call）；③ `local_fs_read_write` 真实文件系统写/读/解析/列表/选取；④ `local_fs_truncation` 200KB 截断；⑤ `sandbox_path_and_command` 沙箱黑白名单；⑥ `end_to_end_capability_loop` 端到端闭环（FakeProvider 驱动 RigAgent 真实调用 write_file/read_file 工具，写→读→总结，断言最终文本、落盘内容与 trace）。配套 Windows 测试 manifest 注入（`src-tauri/manifests/comctl32-v6.manifest` + `build.rs` 在 windows 下输出 `/MANIFEST:EMBED /MANIFESTINPUT:`），修复测试 exe 因 tauri 事件链拉入 comctl32 v6 符号而无 manifest 加载 v5.82 导致的 `STATUS_ENTRYPOINT_NOT_FOUND`。

**Verification** — `cargo test`（worktree src-tauri）PASS：lib 130 + 集成 6 = 136 全部通过（0.01s）；`cargo check` PASS 无错误（既有 PRE-EXISTING：`src/core/adk/instructions.rs:190` unused import `std::fs`）。Windows 本机实测：修复前测试 exe 启动即 0xC0000139，嵌入 manifest 后正常。CI（ubuntu-22.04）不受影响（build.rs 的 manifest 参数仅在 windows 输出；全部测试确定性无网络）。两轮评审：首轮通过，无 CRITICAL，仅 1 处 MINOR（spec 结构描述与实现的集成测试形态偏差，本文档已同步）。

**Journey log** —
- **根因排查（关键）**：本机 Windows 上 worktree 全新构建的测试 exe 启动即 `STATUS_ENTRYPOINT_NOT_FOUND`，而主仓库（历史 target 缓存）正常。逐项排除依赖版本（Cargo.lock gitignored 导致 worktree 重新解析出 49 个升级传递依赖）、rustc 版本、构建路径、`.cargo/config.toml` 后，用「主仓库源码 + fresh target」对照实验锁定**源码文件**为变量，最终用 dumpbin 对比导入表 + mt.exe 手动嵌入 manifest 验证，确定根因是 **comctl32 v6 符号无 SxS manifest**。教训：Tauri 项目测试二进制在本机 Windows 首次全量构建时，凡拉入 `RigAgent::run`→tauri 事件链（muda 菜单）的测试都需要 comctl32 v6 manifest。
- **`cargo:rustc-link-arg-tests` 仅对 tests target 生效**：lib 单元测试（`cargo test --lib`）不属于 tests target，指令会报 "does not have a test target"。因此套件采用集成测试形态（`src-tauri/tests/`），且 build.rs 注入只影响测试二进制、绝不污染应用 bin。
- **`/MANIFESTINPUT` 必须与 `/MANIFEST:EMBED` 配对**，否则 LNK1220。
- 截断断言初版数错行（101）：`read_text_limited` 的截断标记前置 `\n` 产生空行，实际 `lines()` = 102（100 数据行 + 1 空行 + 1 标记行），改为按行内容断言更稳健。
- Tauri 集成测试的可复用模式：Windows 测试 manifest 用 `cargo:rustc-link-arg-tests`（非 `rustc-link-arg`，后者会命中应用 bin）。

## [S1] Problem

项目已通过 `cargo test`（127 passed）与 RAG 评测门槛覆盖大部分业务逻辑，但没有一个**面向 Agent 三大核心能力的整体回归检测**：

1. **对话** —— Agent 能否正常走完「消息 → LLM 生成 → 流式返回 → 结果持久化语义」闭环；
2. **运行** —— Agent 能否通过工具注册表执行工具并把结果回填到循环；
3. **本地读写** —— Agent 相关文件命令能否在工作目录内完成写/读/解析/列表，且沙箱黑白名单生效。

三块能力散布在 `RigAgent`、`ToolRegistry`、`commands::file`、`SandboxPolicy` 中，没有集中、确定性的回归测试证明它们协同可用。用户需要一份**能力自检套件**：零外部依赖（不调真实 LLM/网络，CI 可直接跑），一次 `cargo test` 即可回答「对话、运行、本地读写能力怎么样」。

## [S2] Design

### 套件位置与挂载

- 新建 `src-tauri/tests/capability_test.rs`（**集成测试**形态）。采用集成测试而非 in-module `#[cfg(test)] mod` 的原因：`cargo:rustc-link-arg-tests` 仅对 tests target 生效（lib 单元测试会报 "does not have a test target"），而 Windows manifest 注入必须走该指令；且所需 API（`RigAgent`/`ToolRegistry`/`commands::file`/`SandboxPolicy`/`AgentTrace`）全部 pub，集成测试可直接访问，**生产代码零改动**。
- 随 `cargo test` 自动进入现有 CI test 门槛（`.github/workflows/build.yml` test job 已运行 `cargo test --manifest-path src-tauri/Cargo.toml`），无需改动 CI。

### 测试基座（模块内定义）

**`FakeProvider`**（实现 `core::adk::model::ModelProvider`）：
- 内部持有一个响应队列 `Arc<tokio::sync::Mutex<VecDeque<GenerationResponse>>>`，`generate()` 每次 pop 一条，`stream()` 委托 `generate()` 并把响应转为 `StreamEvent` 流（ToolCall 事件先于 Text 事件，最后 `Finish{usage}`）。
- 这样测试可预置「第 1 轮返回工具调用、第 2 轮返回总结文本」等任意序列，确定性驱动 agentic loop。

**`EchoTool`**（实现 `core::adk::tool::ToolExecutor`）：`echo_tool`，`execute()` 返回 `ToolOutput::text("echo: <text>")`，用 `Arc<AtomicUsize>` 记录调用次数。

**`FileTools`**（实现 `ToolExecutor`，包装真实文件命令）：`write_file`（args: `path`/`content` → `commands::file::file_write`）、`read_file`（args: `path` → `commands::file::file_read_text`），使端到端闭环测试使用**真实文件系统**而非 mock。

**临时目录**：遵循项目先例 `std::env::temp_dir().join(format!("prism_cap_test_{}", uuid::Uuid::new_v4()))`，测试末尾清理。

### 测试用例

| 用例 | 能力维度 | 断言要点 |
|------|---------|---------|
| `conversation_roundtrip` | 对话 | FakeProvider 返回固定文本 → `RigAgent::run` 返回 `Ok`，`result.text` 等于预期，`tool_calls` 为空，`usage` 有估算值 |
| `tool_execution_roundtrip` | 运行 | 第 1 轮 `echo_tool` 调用 → 第 2 轮总结文本；断言最终文本正确、EchoTool 调用计数 = 1、trace 步骤含 `tool_call` |
| `local_fs_read_write` | 本地读写 | 临时目录内 `file_write` → `file_read_text` 内容一致；`file_parse` txt/json 分类正确；`file_list` 含子项；`file_pick` 存在/不存在分支 |
| `local_fs_truncation` | 本地读写 | 写入 >200KB 文件 → `file_read_text` 返回前 100 行 + 截断标记 |
| `sandbox_path_and_command` | 本地读写（沙箱） | `check_path` 拒绝黑名单（`/etc/passwd`）、允许工作目录内路径；`check_command` 拒绝 `rm`、允许 `ls`；拒绝黑名单外的路径 |
| `end_to_end_capability_loop` | 闭环串测 | FakeProvider 预置：① `write_file`（临时目录）② `read_file` ③ 总结文本；断言最终文本包含读回内容、临时文件真实存在且内容正确、trace 含 `llm_call` 与 `tool_call` 步骤 |

### 设计约束

- 零外部依赖：不连真实 LLM / 网络 / MCP；所有测试确定性、无真实延时（避免 flaky）。
- 只测公共行为（`RigAgent::run`、`ToolExecutor::execute`、`commands::file` 公开命令、`SandboxPolicy` 公开方法），不为测试新增生产 API。
- 平台无关：Windows/macOS/Linux 下 `temp_dir` 与 `PathBuf::join` 均可用，CI 三平台一致。

### Windows 测试 manifest（comctl32 v6）

能力检测测试调用 `RigAgent::run()` 会拉入 tauri 事件发射链（`app.emit` → runtime → muda 菜单库），其 `comctl32.dll` v6 符号（`TaskDialogIndirect`/`SetWindowSubclass`/`DefSubclassProc`）被写入测试 exe 导入表。**无 app manifest 时 Windows 加载 comctl32 v5.82（SxS 隔离）→ 入口点缺失 → `STATUS_ENTRYPOINT_NOT_FOUND`（0xC0000139）**，本机无法运行任何 `cargo test`。已用 mt.exe 手动嵌入 manifest 验证修复。

**解法**（入库，Windows 本机与 CI 均生效）：
- `src-tauri/manifests/comctl32-v6.manifest`：声明 `Microsoft.Windows.Common-Controls 6.0.0.0` 依赖（与 Tauri 应用 manifest 一致）。
- `src-tauri/build.rs`：仅 `target_os = "windows"` 时输出 `cargo:rustc-link-arg-tests=/MANIFEST:EMBED` 与 `cargo:rustc-link-arg-tests=/MANIFESTINPUT:<manifest 绝对路径>`（link.exe 将该文件与默认生成 manifest 合并后嵌入；`/MANIFEST:EMBED` 必须配对，否则 LNK1220）。该指令只作用于测试目标，不影响应用 bin。Linux/macOS 不受影响（无 comctl32 概念）。

> 该项目 `Cargo.lock` 被 gitignore 不入库；全新 worktree 重新解析依赖会升级传递依赖（如 icu 2.2→2.3），但与本 feature 无关（依赖版本差异不改变上述根因，用主仓库 lock 复现时仍触发 comctl32 问题）。

## [S3] Out of Scope

- 不新增前端 UI / IPC 命令 / 检测报告页面（本次仅 Rust 测试套件）。
- 不接入真实 LLM 的「在线评测」（agent_eval/rag:eval 已覆盖 LLM-as-Judge 场景）。
- 不修改 `chat_send` 等 tauri 命令的集成路径（需 AppHandle，属于手工验证范畴）。
- 不为测试改生产 API 可见性（已确认所需 API 均 pub）。

## Tasks

- [x] T1: 新建 `src-tauri/tests/capability_test.rs`（FakeProvider/EchoTool/FileTools 基座 + 6 个用例） — 验收：`cargo test --manifest-path src-tauri/Cargo.toml` 中 capability_test 全部通过 (covers: S2)
- [x] T2: 集成测试挂载（tests/ 目录即 tests target，无需 mod.rs 改动；生产代码零改动） — 验收：测试被实际编译执行（用例数体现在测试输出） (covers: S2)
- [x] T3: Windows 测试 manifest：`src-tauri/manifests/comctl32-v6.manifest` + `build.rs` 输出 `cargo:rustc-link-arg-tests=/MANIFEST:EMBED` 与 `/MANIFESTINPUT:<path>`（仅 windows） — 验收：Windows 本机 `cargo test` 不再 `STATUS_ENTRYPOINT_NOT_FOUND`；Linux/macOS 构建不受影响 (covers: S2)
- [x] T4: 验证与回归门槛 — 验收：`cargo check` PASS；全量 `cargo test` 通过且既有 130 测试无回归；新测试为零外部依赖（CI ubuntu runner 可直接跑） (covers: S2)
