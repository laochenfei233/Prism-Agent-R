<script lang="ts">
  interface NormalizedCall {
    id?: string;
    name: string;
    argumentsText: string;
    status?: string;
    durationMs?: number;
  }

  let { call }: { call: unknown } = $props();

  let expanded = $state(false);

  function normalize(call: unknown): NormalizedCall | null {
    if (!call || typeof call !== 'object') return null;
    const c = call as Record<string, unknown>;
    // OpenAI 格式: { id, type: 'function', function: { name, arguments } }
    const fn =
      c.function && typeof c.function === 'object' ? (c.function as Record<string, unknown>) : c;
    const name = typeof fn.name === 'string' ? fn.name : 'tool';
    let argumentsText = '{}';
    const args = fn.arguments;
    if (typeof args === 'string') argumentsText = args;
    else if (args && typeof args === 'object') argumentsText = JSON.stringify(args, null, 2);
    const status = typeof c.status === 'string' ? c.status : undefined;
    let durationMs: number | undefined;
    if (typeof c.duration_ms === 'number') durationMs = c.duration_ms;
    else if (typeof c.duration_ms === 'string') durationMs = Number(c.duration_ms);
    else if (typeof c.durationMs === 'number') durationMs = c.durationMs;
    else if (typeof c.elapsed_ms === 'number') durationMs = c.elapsed_ms;
    return {
      id: typeof c.id === 'string' ? c.id : undefined,
      name,
      argumentsText,
      status,
      durationMs: durationMs && !Number.isNaN(durationMs) ? durationMs : undefined,
    };
  }

  const info = $derived(normalize(call));

  function statusLabel(): string {
    const s = info?.status;
    if (!s) return '已完成';
    return s;
  }
</script>

{#if info}
  <div class="tool-call">
    <div
      class="tool-call-header"
      onclick={() => (expanded = !expanded)}
      onkeydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          expanded = !expanded;
        }
      }}
      role="button"
      tabindex="0"
    >
      <span class="tool-icon">
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <path
            d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
          />
        </svg>
      </span>
      <span class="tool-name">{info.name}</span>
      {#if info.durationMs !== undefined}
        <span class="tool-duration">{info.durationMs}ms</span>
      {/if}
      <span class="tool-status"><span class="dot"></span>{statusLabel()}</span>
      <span class="tool-chevron" class:expanded>{expanded ? '▾' : '▸'}</span>
    </div>
    {#if expanded}
      <div class="tool-args">
        <pre>{info.argumentsText}</pre>
      </div>
    {/if}
  </div>
{/if}

<style>
  .tool-call {
    border: 1px solid var(--color-separator);
    border-radius: var(--radius-sm);
    background: var(--color-bg);
    overflow: hidden;
    font-size: var(--text-caption1);
  }

  .tool-call-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    cursor: pointer;
    user-select: none;
  }

  .tool-icon {
    display: flex;
    align-items: center;
    color: var(--color-fg-tertiary);
    flex-shrink: 0;
  }

  .tool-name {
    flex: 1;
    font-weight: var(--font-weight-semibold);
    font-family: var(--font-mono);
    font-size: var(--text-caption1);
    color: var(--color-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tool-status {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--color-green);
    font-size: var(--text-caption2);
    white-space: nowrap;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-green);
  }

  .tool-duration {
    color: var(--color-fg-tertiary);
    font-size: var(--text-caption2);
    white-space: nowrap;
  }

  .tool-chevron {
    color: var(--color-fg-tertiary);
    font-size: 10px;
    transition: transform var(--duration-fast) var(--ease-default);
  }
  .tool-chevron.expanded {
    transform: rotate(180deg);
  }

  .tool-args {
    padding: 8px 10px;
    border-top: 1px solid var(--color-separator);
    background: var(--color-bg-secondary);
  }

  .tool-args pre {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-fg-secondary);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 200px;
    overflow-y: auto;
  }
</style>
