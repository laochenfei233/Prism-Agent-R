# Contributing to Prism Agent R

Thanks for your interest! This project follows a lightweight engineering discipline to keep the codebase healthy and the releases automatic.

## Development Setup

Prerequisites: Node.js ≥ 20 (LTS), Rust stable, Tauri v2 system dependencies ([official guide](https://v2.tauri.app/start/prerequisites/)).

```bash
npm ci              # install frontend dependencies
npm run dev         # Vite dev server
npm run tauri dev   # desktop app (dev)
```

## Quality Gates

Every change must pass the same gates CI runs:

```bash
# Frontend
npm run lint              # eslint (flat config)
npm run format:check      # prettier
npm test                  # vitest
npm run check             # svelte-check

# Rust (from src-tauri/)
cargo fmt --check
cargo clippy -- -D warnings
cargo test                # includes RAG eval gate + agent capability suite
```

> Note: `cargo test` runs the RAG evaluation regression gate — retrieval metrics below the recorded baseline fail the build. Do not weaken the baselines without a documented reason.

## Commit Conventions

This project uses **Conventional Commits** — release versions and the CHANGELOG are generated from commit messages by semantic-release, so formatting matters.

```
<type>(<scope>): <subject>
```

- **Types**: `feat` (minor release) · `fix` (patch release) · `refactor` · `docs` · `test` · `chore` · `perf` · `style`
- **Scope**: the affected area, e.g. `chat`, `rag`, `workflow`, `settings`, `ci`
- **Breaking changes**: append `!` after the type/scope (e.g. `feat(rag)!: ...`) or add a `BREAKING CHANGE:` footer — these produce a major release
- Keep the subject concise (≤ 72 chars), lowercase, imperative mood: `feat(chat): add thinking block streaming`

Examples from the history:

```
feat(kanban): add Agent task management tools + tasks display
refactor(composer): move mode toggle above textarea
fix(chat): fix SSE parsing for MiMo API responses
```

## Testing

- **Rust**: write unit tests next to the code (`#[cfg(test)]`) and integration tests in `src-tauri/tests/`. The agent capability suite (`capability_test.rs`) must stay green and dependency-free (no real LLM/network).
- **Frontend**: add a vitest test alongside the component/logic you change (`src/**/*.test.ts`). Prefer real rendering over mocks.
- A bug fix should ship with a regression test when one can be written.

## Branching & Workflow

- Use the repo's existing worktree convention: create a linked worktree under `.worktrees/` for feature work, e.g. `git worktree add .worktrees/<slug> -b feat/<slug>`.
- Never commit directly to `master`/`main`; work on a feature branch and open a PR.
- PRs target `master` (or `main`); CI must pass before merge. The `test` job is the merge gate (Rust tests + svelte-check + lint/format + frontend tests).
- Releases are automatic: merging to `master` triggers semantic-release — it versions from commit messages, updates `CHANGELOG.md`, tags `vX.Y.Z`, and publishes a GitHub Release with build artifacts.

## Documentation

- Design docs live in `docs/design/` (section matrix in `docs/design/README.md`).
- Feature work with a durable design surface should carry a spec in `docs/compose/specs/<feature>.md` (see `docs/compose/specs/phase5-production-spec.md` for the format).
- New DB migrations: bump the migration number and register it in the master migration table (`docs/compose/specs/prism-agent-r.md`). Never append to an applied migration.
