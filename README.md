# Prism Agent R

> A cross-platform desktop AI Agent platform. Rust-powered backend, Svelte 5 frontend, wrapped in Tauri 2.

**Prism Agent R** is a desktop AI agent workspace for Windows, macOS, and Linux. It brings together an agentic chat loop, MCP tool integration, RAG knowledge base, meeting transcription, translation/OCR, multi-agent workflows, and production-grade guardrails — all in one native app with no Node.js runtime dependency (the backend is a compiled Rust binary).

## ✨ Features

| Area                     | Highlights                                                                                                                                      |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| **Agent Chat**           | Streaming responses, markdown + syntax highlighting, thinking blocks, tool-call cards                                                           |
| **Model Providers**      | OpenAI-compatible & Anthropic-compatible providers, Ollama, MCP servers, per-agent model assignment                                             |
| **MCP & Skills**         | MCP protocol (stdio/HTTP/SSE), skill marketplace, agent tool registry with file read/write                                                      |
| **Knowledge**            | Wiki + RAG pipeline: contextual retrieval, HyDE, RRF multi-path fusion, cliff-cutoff, traceable citations                                       |
| **Web Search**           | Search provider chain (Tavily / Serper / SearxNG) with caching and graceful degradation                                                         |
| **Workflows**            | Multi-agent workflow engine (V2: budget, guardrails, retry), auto-orchestration (Spec→Plan→Execute→Review)                                      |
| **Productivity**         | Meeting transcription (ASR), translation, OCR, glossary, memory system, goal tracking                                                           |
| **Production Hardening** | Three-tier token budgets + model fallback, tool/trajectory/sandbox guardrails, structured logging, exception recording, real-time monitor panel |
| **Agent Capabilities**   | Conversation / tool-execution / local-fs round-trip capability test suite (`capability_test.rs`)                                                |

## 🏗 Architecture

```
┌────────────────────────────────────────────┐
│  Frontend: Svelte 5 (WebView)              │
│  design-system · components · stores       │
└──────────────────┬─────────────────────────┘
                   │ Tauri 2 IPC (commands / events / streaming)
┌──────────────────▼─────────────────────────┐
│  Backend: Rust                             │
│  core/adk     — agent loop, tools, memory  │
│  core/rig     — provider adapters, stream  │
│  core/search  — web search providers       │
│  core/budget  — token budgets, fallback    │
│  core/guardrails — tool / trajectory / sandbox │
│  core/observability — logger, exceptions   │
│  data/        — SQLite (sqlx) + migrations │
│  commands/    — IPC command modules        │
└────────────────────────────────────────────┘
```

- **Backend**: Rust (tokio, sqlx/SQLite, tauri v2)
- **Frontend**: Svelte 5, Vite, Tailwind-style design tokens
- **Data**: SQLite with versioned migrations (`src-tauri/src/data/migrations/`)
- **CI**: GitHub Actions — Rust tests (with RAG eval regression gate) + svelte-check + multi-platform builds (Windows/macOS/Linux)

## 🚀 Getting Started

### Prerequisites

- [Node.js](https://nodejs.org) ≥ 20 (LTS)
- [Rust](https://rustup.rs) stable toolchain
- Tauri v2 system dependencies for your platform ([official guide](https://v2.tauri.app/start/prerequisites/))

### Run in development

```bash
npm ci          # install frontend dependencies
npm run dev     # start Vite dev server
npm run tauri dev   # launch the desktop app (dev)
```

### Build

```bash
npm run check   # svelte-check type check
npm test        # frontend unit/component tests (vitest)
npm run lint    # eslint
npm run format:check   # prettier

# Rust side (from src-tauri/)
cargo test      # includes RAG eval regression gate + agent capability suite
cargo clippy -- -D warnings
cargo fmt --check

npm run tauri build  # produce installers for the current platform
```

## 📚 Documentation

| Doc                                                                                        | Contents                                                                     |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------- |
| [`docs/design/README.md`](docs/design/README.md)                                           | Design docs index — section → file → phase matrix, reading guide             |
| [`docs/compose/specs/prism-agent-r.md`](docs/compose/specs/prism-agent-r.md)               | Master index: problem, architecture, tech selection, MVP, migration table    |
| [`docs/design/phase1-core.md`](docs/design/phase1-core.md)                                 | Phase 1 — Agent core loop (architecture, DB schema, MCP, streaming, IPC)     |
| [`docs/design/phase2-panel.md`](docs/design/phase2-panel.md)                               | Phase 2 — Panels, agent sidebar, human-in-the-loop approvals                 |
| [`docs/design/phase3-extend.md`](docs/design/phase3-extend.md)                             | Phase 3 — Wiki/RAG, meetings, translation/OCR, reflection, guardrails, evals |
| [`docs/design/phase4-agentic.md`](docs/design/phase4-agentic.md)                           | Phase 4 — Web search, RAG retrieval enhancements, harness engineering        |
| [`docs/design/phase5-production-hardening.md`](docs/design/phase5-production-hardening.md) | Phase 5 — Budgets, guardrails, observability, workflow V2, orchestration     |

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) — commit conventions (Conventional Commits), test requirements, and the PR flow.

## 📜 License

[MIT](LICENSE) © laochenfei233
