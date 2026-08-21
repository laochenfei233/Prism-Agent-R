# Changelog

All notable changes to this project are documented here. This file is maintained by [semantic-release](https://semantic-release.gitbook.io/) — versions and entries are generated from commit messages following [Conventional Commits](https://www.conventionalcommits.org/).

## [Unreleased]

### Added
- Repository engineering polish: root `README.md`, `LICENSE` (MIT), `CONTRIBUTING.md`, initial `CHANGELOG.md`
- Frontend quality gates: eslint (flat config), prettier, `.editorconfig`
- Frontend test framework: vitest + @testing-library/svelte with first component tests
- Rust quality gates in CI: `cargo fmt --check`, `cargo clippy -- -D warnings`
- Automated releases: semantic-release (Conventional Commits → version + CHANGELOG + GitHub Release)

## 0.1.0 (2026-08-21)

Snapshot of the codebase before automated release tooling was introduced. Core functionality accumulated through five design phases:

### Agent Core (Phase 1)
- Agent chat loop with streaming responses, markdown + syntax highlighting, thinking blocks
- Multi-provider model support (OpenAI-compatible, Anthropic-compatible, Ollama), per-agent model assignment
- MCP protocol support (stdio/HTTP/SSE), skill system with marketplace
- Agent tool registry (file read/write/list), memory system, workflow engine with templates
- SQLite persistence with versioned migrations, message/session FTS search

### Panels (Phase 2)
- Main dashboard panels, agent sidebar (six tabs), human-in-the-loop tool approvals

### Extensions (Phase 3)
- Wiki + RAG (contextual retrieval, document parsing, traceable citations, multi-dimensional evals)
- Meeting transcription (ASR), translation with FTS history, OCR, glossary
- Reflection mode, goal setting & monitoring, security guardrails, evaluation & monitoring (agent traces, AgentJudge)
- Accessibility (a11y), context compaction

### Autonomous capabilities (Phase 4)
- Web search toolchain (Tavily/Serper/SearxNG) with caching and graceful degradation
- RAG retrieval enhancements: HyDE, RRF multi-path fusion, cliff-cutoff, idempotent import
- Harness engineering: session lifecycle state machine, agent loop (goal/timer/maker-checker), trace grading, session replay

### Production hardening (Phase 5)
- Three-tier token budgets (Global/Crew/Agent) with model fallback chain and degradation policies
- Guardrails: tool-level whitelist/blacklist/approval, trajectory-level checks, filesystem/network/process sandbox
- Observability: structured JSON Lines logger, exception recording with SQLite persistence
- Workflow engine V2 (budget/guardrails/retry integration), auto-orchestration loop (Spec→Plan→Execute→Review)
- Real-time monitor panel (budgets/exceptions/trends, pause/resume/terminate controls)

### Agent capabilities (ongoing)
- Agent capability test suite (`capability_test.rs`): conversation, tool-execution, local-fs read/write/truncation, sandbox path & command enforcement
- Kanban: Agent task management tools + task board merged into agent cards
