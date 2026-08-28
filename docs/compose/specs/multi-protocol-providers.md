---
feature: multi-protocol-providers
status: designed
updated: 2026-08-28
branch: feat/multi-protocol-providers
commits:
---

# Multi-Protocol Providers

## Report

## [S1] Problem
The Agent only supports OpenAI Chat Completions API format (`/chat/completions`). DeepSeek now offers a Responses API (`/responses`) and Anthropic-protocol endpoint (`/anthropic/messages`), but the provider dispatch in `chat_send` hardcodes `OpenAiProvider`. Users cannot use DeepSeek Responses API or any Anthropic-protocol model (Claude, DeepSeek via Anthropic API) with the Agent.

## [S2] Design
Add two new provider kinds alongside existing `openai` and `ollama`:

1. **`responses-api`** — OpenAI Responses API protocol (`POST /responses`)
   - Generic protocol: works with OpenAI, DeepSeek, and any compatible endpoint
   - Request format: `model`, `input`, `instructions`, `tools`, `stream`, `temperature`, `max_output_tokens`, `reasoning`
   - Streaming events: `response.output_text.delta`, `response.reasoning_text.delta`, `response.function_call_arguments.delta`, `response.completed`
   - Default base_url: `https://api.openai.com/v1` (override to `https://api.deepseek.com` for DeepSeek)

2. **`anthropic`** — Anthropic Messages API protocol (`POST /messages`)
   - Generic protocol: works with Anthropic, DeepSeek (`/anthropic`), and any compatible endpoint
   - Request format: `model`, `messages` (content blocks), `system`, `max_tokens`, `tools`, `stream`, `thinking`
   - Streaming events: `content_block_delta` with `text_delta`, `input_json_delta`, `thinking_delta`
   - Default base_url: `https://api.anthropic.com/v1` (override to `https://api.deepseek.com/anthropic` for DeepSeek)
   - Auth header: `x-api-key` instead of `Bearer`

Provider dispatch in `chat_send` matches `provider.kind` to instantiate the correct provider:
- `openai` / `ollama` → `OpenAiProvider` (existing)
- `responses-api` → `ResponsesApiProvider` (new)
- `anthropic` → `AnthropicProvider` (new)

Frontend settings page gets two new preset providers in the "Add Provider" flow with correct defaults.

## [S3] Scope
- In scope:
  - `ResponsesApiProvider` trait impl (generate + stream)
  - `AnthropicProvider` trait impl (generate + stream)
  - Provider dispatch in `chat_send` based on `provider.kind`
  - Frontend: new provider kind presets in settings
  - Default base_url per kind
- Out of scope:
  - Responses API `previous_response_id` / `conversation` (stateless only)
  - Anthropic images / documents / MCP tools
  - Vision/multimodal content
  - Provider auto-detection from URL
  - New DB migrations (provider.kind is already a freeform string)

## Tasks
- [ ] T1: Create `ResponsesApiProvider` in `core/rig/provider/responses_api.rs` (depends: none)
- [ ] T2: Create `AnthropicProvider` in `core/rig/provider/anthropic.rs` (depends: none)
- [ ] T3: Register new providers in `core/rig/provider/mod.rs` (depends: T1, T2)
- [ ] T4: Update `chat_send` dispatch to route by `provider.kind` (depends: T3)
- [ ] T5: Update frontend settings with new provider kind presets (depends: none)
- [ ] T6: Verify build + tests pass (depends: T4, T5)

## [S4] Acceptance
- [ ] `ResponsesApiProvider` implements `ModelProvider` trait with `generate` and `stream`
- [ ] `AnthropicProvider` implements `ModelProvider` trait with `generate` and `stream`
- [ ] `chat_send` correctly routes to each provider based on `provider.kind`
- [ ] Frontend can add `responses-api` and `anthropic` providers with correct defaults
- [ ] `cargo test --lib` passes
- [ ] `cargo clippy` passes
