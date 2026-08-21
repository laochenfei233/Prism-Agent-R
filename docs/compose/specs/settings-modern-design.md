---
feature: settings-modern-design
status: delivered
updated: 2026-08-15
branch: feat/settings-modern-design
commits: 42f2adb..HEAD
---

# Settings Modern Design — LLM / ASR / TTS

## Report

**What was built** — Redesigned the three model-related settings sections (LLM 模型管理, 语音识别, 语音合成) with iOS 26 Liquid Glass design language. Added glass material CSS variables to `tokens.css`, applied Medium Glass to the settings nav sidebar, Solid Glass to list panes, and Light Glass to search fields. Redesigned the providers detail pane with iOS-style sticky glass header. Removed model capability badges (kind-badge) and added "可用于语音识别" hint for ASR models. Restructured the ASR section with model-row styling, inline download progress bars, card-header-row with add button, inset form sub-card, and proper empty states. Built a complete TTS configuration UI with voice dropdown, rate slider (0.5–2.0x), test button, and localStorage persistence. Implemented cross-section linkage: auto-linked ASR providers shown as info hint, TTS linked providers info card.

**Verification** — `npm run check` (svelte-check): 0 errors, 0 warnings.

**Journey log** — The original file was ~2068 lines with inline styles. The design direction evolved through user feedback: initially planned a manual "从 LLM 服务商导入" dropdown for ASR linkage, but user requested automatic sync instead. Removed the dropdown in favor of auto-linked provider hints. Model kind-badge removal was an additional user requirement not in the original spec. The TTS implementation leveraged the existing `tts.svelte.ts` API which already had all needed functions — only the UI was missing.

## [S1] Problem

The settings page's three model-related sections — LLM 模型管理 (providers), 语音识别 (ASR), and 语音合成 (TTS) — look like unstyled HTML. The providers section has a basic two-pane layout but lacks visual polish. The ASR section has bare cards with no visual hierarchy. The TTS section is nearly empty, showing only a single hint paragraph with no actual configuration controls. None of them use the iOS 26 Liquid Glass material system that the project's design tokens are built for.

## [S2] Design

### Design language: iOS 26 Liquid Glass

Add glass material CSS variables to `tokens.css`, then apply them to settings surfaces. Four tiers per the platform-core skill, but only three are needed here (Light, Medium, Solid):

| Tier | Variable | Light value | Dark value | Use for |
|---|---|---|---|---|
| Light Glass | `--glass-light-bg` / `--glass-light-blur` | `rgba(255,255,255,0.55)` / `blur(20px) saturate(180%)` | `rgba(30,30,30,0.55)` / `blur(20px) saturate(150%)` | Search fields, floating inputs |
| Medium Glass | `--glass-medium-bg` / `--glass-medium-blur` | `rgba(255,255,255,0.65)` / `blur(40px) saturate(180%)` | `rgba(30,30,30,0.65)` / `blur(40px) saturate(150%)` | Settings nav sidebar |
| Solid Glass | `--glass-solid-bg` / `--glass-solid-blur` | `rgba(255,255,255,0.82)` / `blur(80px) saturate(180%)` | `rgba(30,30,30,0.82)` / `blur(80px) saturate(150%)` | List pane containers |

Edge highlight (shared): `--glass-edge-highlight: inset 0 1px 0 rgba(255,255,255,0.5)` (light) / `inset 0 1px 0 rgba(255,255,255,0.08)` (dark).

Inner highlight (shared): `--glass-inner-shadow: inset 0 1px 1px rgba(255,255,255,0.3)` (light) / `inset 0 1px 1px rgba(255,255,255,0.04)` (dark).

`prefers-reduced-transparency` (already in `app.css`) disables `backdrop-filter` and falls back to `--color-bg` / `--color-bg-secondary`.

### Concrete CSS changes (all in the `<style>` block of `+page.svelte`)

1. **`.settings-nav`**: Add `background: var(--glass-medium-bg); backdrop-filter: var(--glass-medium-blur); -webkit-backdrop-filter: var(--glass-medium-blur); box-shadow: var(--glass-edge-highlight);` — replaces current `background: var(--color-bg-secondary)`.

2. **`.provider-list-pane`**: Add `background: var(--glass-solid-bg); backdrop-filter: var(--glass-solid-blur); -webkit-backdrop-filter: var(--glass-solid-blur); box-shadow: var(--glass-edge-highlight);` — replaces current `background: var(--color-bg-secondary)`.

3. **`.pane-search` / `.model-search`**: Switch from hard-coded `color-mix(in srgb, var(--color-bg) 72%, transparent)` + `blur(20px) saturate(180%)` to `background: var(--glass-light-bg); backdrop-filter: var(--glass-light-blur); -webkit-backdrop-filter: var(--glass-light-blur); box-shadow: var(--glass-edge-highlight);`.

4. **`.card`**: Increase radius from 12px to `var(--radius-md)` (12px, same — but add `box-shadow: var(--glass-edge-highlight)` for edge lift). Keep flat background `var(--color-bg-secondary)` for content readability. Add subtle top border highlight.

5. **`.content-header`**: Add `position: sticky; top: 0; z-index: 10; background: var(--glass-medium-bg); backdrop-filter: var(--glass-medium-blur); -webkit-backdrop-filter: var(--glass-medium-blur); padding: 12px 0; margin: -8px -8px 16px; padding-left: 8px; padding-right: 8px;` so the header stays visible when scrolling the detail pane.

6. **`.config-row`**: Add `border-radius: 8px; padding-left: 8px; padding-right: 8px;` so rows have breathing room inside cards. Change `border-bottom` to `border-bottom: 0.5px solid var(--color-separator)` for finer dividers.

7. **Model rows (`.model-row`)**: Add `box-shadow: var(--glass-edge-highlight)` on hover for subtle glass lift effect.

### Section 1: LLM Model Management (providers) — T2

**HTML changes**: Remove model capability badges (`kind-badge` spans) from model rows — the user does not want them. All other HTML structure stays the same.

**Detail pane visual design** (the right-side content area — the largest and most important part):

The detail pane has three vertical zones. Each needs specific iOS 26 treatment:

**Zone A — Sticky header** (`.content-header`):
- Provider icon (36px, rounded 10px, white bg, 1px border) + title (20px bold) + description (13px secondary)
- Sticky at top with Medium Glass background so it stays visible when scrolling
- Bottom border: `0.5px solid var(--color-separator)` to separate from content below
- Padding: `16px 8px 12px`

**Zone B — Connection settings** (`.conn-section`):
- Restructure as an iOS Settings-style **grouped list card**: white/secondary background, `border-radius: 12px`, `border: 1px solid var(--color-separator)`, `box-shadow: var(--glass-edge-highlight)`
- Two rows inside, separated by `0.5px solid var(--color-separator)` hairline:
  - Row 1 "API 地址": label (14px medium, 80px width) on left, input on right (flex: 1, transparent bg, no border, 13px). Input gets Light Glass treatment on focus.
  - Row 2 "API 密钥": same layout. Key display shows `••••••••••••••••` (monospace) or "未设置". Edit mode shows input + show/hide toggle + save/cancel buttons.
- Each row: `padding: 12px 16px`, `min-height: 44px` (iOS touch target), `display: flex; align-items: center; gap: 12px`
- Section label "连接" (13px semibold, `--color-fg-secondary`) above the card, `margin-bottom: 8px`

**Zone C — Model list** (`.model-section`):
- Section header: "模型列表" (15px semibold) on left, toolbar on right (search field 180px + "获取模型列表" button + "保存" button)
- Toolbar buttons: `btn-sm` style, consistent height (28px)
- **Model rows** (`.model-row`): clean, no badges
  - Layout: `display: flex; align-items: center; padding: 10px 12px; border-radius: 8px;`
  - Left: model name (13px, `--color-fg`), `flex: 1`
  - Right: "more" menu button (three dots, `btn-icon` style)
  - No kind-badge spans — removed per user request
  - Hover: `background: var(--color-bg-hover)` with smooth transition
  - Separator between rows: `0.5px solid var(--color-separator)`, inset 12px from left (iOS grouped list style)
- "已启用" sub-header: 12px medium, `--color-fg-secondary`, `margin: 14px 0 4px`, `padding: 0 4px`
- Available models section: same row style, but right side shows "添加" button or "已添加" text badge (this is a status badge, not a capability badge — keep it)
- Empty state: centered text with muted color, `padding: 24px 12px`

**Add provider mode**: Same visual treatment as detail mode. Connection section uses the same grouped list card. Model list shows preset models in the same clean row style (no badges, no buttons — display only).

### Section 2: ASR (语音识别) — T3

**HTML restructure** of the detail pane (lines ~1016-1075 in current file):

**Model Management card** — restructure each model row:
- Current: bare `config-row` with name + badge + button.
- New: `model-row` class (reuse from providers section) with name on left, `config-badge` for size, and action button on right. When downloading, replace the text badge with a thin `<div class="download-progress"><div class="download-progress-bar" style="width: {pct}%"></div></div>` inline progress bar (height 4px, accent-colored, rounded).
- Empty state: replace `<p class="hint">` with a centered `<div class="empty-state">` containing an icon + text.

**ASR Configuration card** — restructure the form and config list:
- "+ 新建配置" button: change from `btn-secondary` to `btn-sm` with a "+" icon, placed in the card header row (title + button side by side).
- Form (`asr-form`): wrap inputs in a sub-card with `background: var(--color-bg); border-radius: var(--radius-sm); padding: 12px;` so the form looks like an inset panel.
- Config rows: same `config-row` treatment as model rows — consistent spacing, badge styling.
- Empty state: same centered empty-state pattern.

**Backend list** (left pane): Apply the same `.provider-list-pane` glass treatment (already shared via CSS class). No HTML changes needed — the pane already uses `provider-list-pane` class.

### Section 3: TTS (语音合成) — T4

**Script additions** (after ASR state block, ~line 313):

```typescript
// TTS 语音合成
let ttsVoicesList = $state<SpeechSynthesisVoice[]>([]);
let ttsSelectedVoiceURI = $state('');
let ttsRate = $state(1);
let ttsTesting = $state(false);

function loadTTSConfig() {
    try {
        ttsSelectedVoiceURI = localStorage.getItem('prism-tts-voice') ?? '';
        const r = localStorage.getItem('prism-tts-rate');
        ttsRate = r ? Number(r) : 1;
    } catch (e) {}
}

function loadTTSVoices() {
    ttsVoicesList = ttsVoices();
    if (!ttsSelectedVoiceURI && ttsVoicesList.length > 0) {
        // Auto-select first zh-CN voice or first voice
        const zh = ttsVoicesList.find(v => v.lang.startsWith('zh'));
        ttsSelectedVoiceURI = (zh ?? ttsVoicesList[0]).voiceURI;
    }
}

function onTTSVoiceChange(uri: string) {
    ttsSelectedVoiceURI = uri;
    try { localStorage.setItem('prism-tts-voice', uri); } catch (e) {}
}

function onTTSRateChange(rate: number) {
    ttsRate = rate;
    ttsSetRate(rate);
    try { localStorage.setItem('prism-tts-rate', String(rate)); } catch (e) {}
}

async function testTTS() {
    if (ttsTesting) return;
    ttsTesting = true;
    const voice = ttsVoicesList.find(v => v.voiceURI === ttsSelectedVoiceURI);
    // ttsSpeak uses ttsState.rate; set it before speaking
    ttsState.rate = ttsRate;
    // Use a sample sentence
    ttsSpeak('你好，这是语音合成测试。当前语速为' + ttsRate.toFixed(1) + '倍。', ttsRate);
    // Reset after a delay (sample text ~3s at 1x)
    setTimeout(() => { ttsTesting = false; }, 4000);
}
```

Voice loading: `speechSynthesis.getVoices()` can return `[]` initially. In `onMount`, listen for the `voiceschanged` event and call `loadTTSVoices()`. Also call once on mount. Add cleanup for the event listener.

**HTML structure** (replaces current lines ~1078-1085):

```html
{:else if section === 'tts'}
    <div class="content-header">
        <h2 class="content-title">语音合成 (TTS)</h2>
        <p class="content-desc">配置文本转语音的音色与语速</p>
    </div>

    <!-- Info card -->
    <div class="card tts-info-card">
        <div class="tts-info-icon">[speaker icon SVG]</div>
        <div>
            <p class="hint">TTS 使用浏览器内置的 Web Speech API 播报（如会议待办播报），无需额外安装模型。</p>
            {!ttsState.supported && <p class="hint" style="color: var(--color-red)">当前环境不支持 Web Speech API</p>}
        </div>
    </div>

    <!-- Voice configuration card -->
    <div class="card">
        <div class="section-title">音色与语速</div>
        
        <!-- Voice selection -->
        <div class="config-row">
            <div class="config-info">
                <span class="config-name">音色</span>
            </div>
            <select class="field-input tts-select" value={ttsSelectedVoiceURI} onchange={(e) => onTTSVoiceChange(e.currentTarget.value)} disabled={!ttsState.supported}>
                {#each ttsVoicesList as v}
                    <option value={v.voiceURI}>{v.name} ({v.lang})</option>
                {/each}
            </select>
        </div>

        <!-- Rate slider -->
        <div class="config-row tts-rate-row">
            <div class="config-info">
                <span class="config-name">语速</span>
                <span class="config-badge">{ttsRate.toFixed(1)}x</span>
            </div>
            <div class="tts-slider-wrap">
                <Slider value={ttsRate} min={0.5} max={2} step={0.1} disabled={!ttsState.supported} />
                <div class="tts-rate-labels">
                    <span>0.5x</span>
                    <span>1.0x</span>
                    <span>2.0x</span>
                </div>
            </div>
        </div>

        <!-- Test button -->
        <div class="tts-test-row">
            <button class="btn-primary" onclick={testTTS} disabled={!ttsState.supported || ttsTesting}>
                {ttsTesting ? '播报中…' : '试听'}
            </button>
        </div>
    </div>
```

**Layout**: Single-column, constrained by existing `.settings-content > .card { max-width: 720px }` rule. No left sub-pane.

**Additional CSS** for TTS-specific elements:
- `.tts-info-card`: flex row, icon + text, `background: color-mix(in srgb, var(--color-accent) 6%, var(--color-bg-secondary))`.
- `.tts-info-icon`: 32px circle, accent-tinted background.
- `.tts-select`: `max-width: 280px` for the voice dropdown.
- `.tts-rate-row`: `flex-direction: column; align-items: stretch;` on narrow screens.
- `.tts-slider-wrap`: `flex: 1; max-width: 300px;` with the slider and rate labels below.
- `.tts-rate-labels`: flex row, `justify-content: space-between`, `font-size: 11px; color: var(--color-fg-tertiary)`.
- `.tts-test-row`: `margin-top: 8px;`

### Cross-section linkage (LLM → ASR / TTS) — T6

The three settings sections should be connected: when an LLM provider has models that support ASR (kind='asr'), the provider's API key and base URL should be usable from the ASR settings. Same for TTS-capable providers.

**Constraint**: The frontend cannot read the actual API key value — `has_key` is a boolean, the key is write-only. So the linkage pre-fills everything except the key, and shows a hint to enter the same key.

**LLM → ASR linkage**:

In the ASR "新建配置" form, when the backend kind supports HTTP/API (`WhisperApi`, `Custom`, or kind includes `Http`), add a "从 LLM 服务商导入" dropdown at the top of the form:

```typescript
// Derived: providers that have at least one ASR-type model
const asrLinkedProviders = $derived(
    providers.filter(p => models.some(m => m.provider_id === p.id && m.kind === 'asr'))
);
```

When a provider is selected from this dropdown:
- `asrNewConfig.name` → `{provider.name} ASR` (e.g., "OpenAI ASR")
- `asrModelPathInput` → the first ASR model ID from that provider (e.g., "whisper-1")
- Show the provider's `base_url` as a read-only hint below the model path input
- Show a note: "API 密钥需与 {provider.name} 一致" with a green checkmark if `provider.has_key` is true
- `asrNewConfig.api_key` remains empty — user enters manually

**LLM → TTS linkage**:

In the TTS section, add a "关联服务商" info card below the voice configuration card:

```typescript
// Derived: providers that could support TTS (based on provider kind — OpenAI, DashScope, etc.)
const ttsLinkedProviders = $derived(
    providers.filter(p => ['openai', 'dashscope', 'minimax', 'custom'].includes(p.kind) && p.has_key)
);
```

This card shows:
- Title: "可用的 API TTS 服务商" (13px semibold)
- For each linked provider: name + base_url + key status badge
- Note: "这些服务商支持 TTS API，当前 TTS 使用浏览器内置 Web Speech API。如需使用 API TTS，请在对应服务商配置中添加 TTS 模型。"
- This is informational only — no auto-fill or config changes

**Providers section hint**: When a model with kind='asr' is added or exists in the model list, show a subtle text hint below the model name: "可用于语音识别" (11px, `--color-fg-tertiary`). This is not a badge — just small helper text.

### Shared improvements (all sections)

- Standardize `config-row` spacing: `padding: 12px 8px` (was `16px 0`), `border-bottom: 0.5px solid var(--color-separator)`.
- Add `.empty-state` class: `text-align: center; padding: 32px 16px; color: var(--color-fg-tertiary);` with an optional icon.
- Consistent `content-header` across all three sections (already exists, just add sticky glass treatment).

## [S3] Out of Scope

- Redesigning other settings sections (agents, mcp, skills, market, memory, rag, security, advanced).
- Backend API changes — all data comes from existing API endpoints.
- Extracting settings sections into separate Svelte components — keeping the single-file structure.
- Updating the left navigation sidebar HTML structure — only CSS glass treatment.
- Adding new ASR backends or model sources.
- TTS voice customization beyond what Web Speech API provides (no external TTS services).
- Device tilt response (gyroscope-based glass highlight shifting) — desktop app, no gyroscope.

## Tasks

- [ ] T1: Add glass material CSS variables to tokens.css — acceptance: `--glass-light-bg`, `--glass-light-blur`, `--glass-medium-bg`, `--glass-medium-blur`, `--glass-solid-bg`, `--glass-solid-blur`, `--glass-edge-highlight`, `--glass-inner-shadow` variables exist in `:root` and `.dark` with values matching the tier table in S2 (covers: S2)
- [ ] T2: Apply iOS 26 glass materials to providers section and redesign detail pane — acceptance: `.settings-nav` uses Medium Glass; `.provider-list-pane` uses Solid Glass; `.pane-search` and `.model-search` use Light Glass tokens; `.content-header` is sticky with glass background; connection settings use iOS-style grouped list card; model rows have NO kind-badge spans; model rows have 0.5px inset separators; "可用于语音识别" hint text shown for kind='asr' models (covers: S2; depends: T1)
- [ ] T3: Redesign ASR section with grouped glass cards and improved model/config layout — acceptance: model rows use `model-row` class with inline download progress bar (not text badge); form wrapped in inset sub-card; "+ 新建配置" button in card header row; empty states use `.empty-state` class; config rows have consistent spacing (covers: S2; depends: T1)
- [ ] T4: Build TTS configuration UI with voice selection, rate slider, and test button — acceptance: voice dropdown populated from `ttsVoices()` with `voiceschanged` event handling; rate slider (0.5–2.0, step 0.1) uses Slider component and updates `ttsState.rate`; test button calls `ttsSpeak()` with sample text; voice and rate persist to localStorage keys `prism-tts-voice` / `prism-tts-rate`; unsupported state shown when `!ttsState.supported`; info card styled with accent tint (covers: S2; depends: T1)
- [ ] T5: Implement cross-section linkage (LLM → ASR / TTS) — acceptance: ASR new config form has "从 LLM 服务商导入" dropdown listing providers with ASR models; selecting a provider pre-fills name, model path, and shows base_url hint; TTS section has "关联服务商" info card listing TTS-capable providers; providers section shows "可用于语音识别" hint for ASR models (covers: S2; depends: T2, T3, T4)
- [ ] T6: Verify svelte-check passes and no runtime errors — acceptance: `npm run check` exits 0; dev server loads `/settings` page, switches between providers/asr/tts sections without console errors (covers: S2; depends: T2, T3, T4, T5)
