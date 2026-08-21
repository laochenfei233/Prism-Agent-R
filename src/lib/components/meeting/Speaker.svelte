<script lang="ts">
  // §10.3.9 TTS 播报控制条：播放/暂停/停止/语速
  import { ttsState, ttsPause, ttsResume, ttsStop, ttsSetRate } from '$lib/tts.svelte';

  let collapsed = $state(false);
</script>

{#if ttsState.supported && ttsState.queue.length > 0}
  <div class="speaker" class:collapsed>
    {#if !collapsed}
      <div class="info">
        <span class="dot" class:on={ttsState.playing} class:paused={ttsState.paused}></span>
        <span class="label">
          {#if ttsState.paused}
            已暂停 · {ttsState.current}/{ttsState.queue.length}
          {:else}
            播报中 · {ttsState.current + 1}/{ttsState.queue.length}
          {/if}
        </span>
      </div>
      <div class="controls">
        {#if ttsState.playing && !ttsState.paused}
          <button type="button" title="暂停" onclick={ttsPause}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"
              ><rect x="6" y="4" width="4" height="16" /><rect
                x="14"
                y="4"
                width="4"
                height="16"
              /></svg
            >
          </button>
        {:else if ttsState.paused}
          <button type="button" title="继续" onclick={ttsResume}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"
              ><polygon points="6 4 20 12 6 20" /></svg
            >
          </button>
        {:else}
          <button type="button" title="暂停" disabled>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"
              ><rect x="6" y="4" width="4" height="16" /><rect
                x="14"
                y="4"
                width="4"
                height="16"
              /></svg
            >
          </button>
        {/if}
        <button type="button" title="停止" onclick={ttsStop}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"
            ><rect x="5" y="5" width="14" height="14" /></svg
          >
        </button>
        <label class="rate" title="语速">
          <span>{ttsState.rate.toFixed(1)}x</span>
          <input
            type="range"
            min="0.5"
            max="2"
            step="0.1"
            value={ttsState.rate}
            oninput={(e) => ttsSetRate(Number((e.target as HTMLInputElement).value))}
          />
        </label>
      </div>
      <button type="button" class="collapse" title="收起" onclick={() => (collapsed = true)}
        >—</button
      >
    {:else}
      <button type="button" class="expand" title="展开播报控制" onclick={() => (collapsed = false)}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
          ><path d="M3 9v6h4l5 5V4L7 9H3z" /><path
            d="M16 8c1 1.3 1.5 2.5 1.5 4s-.5 2.7-1.5 4"
          /></svg
        >
      </button>
    {/if}
  </div>
{/if}

<style>
  .speaker {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    border-radius: 20px;
    background: var(--color-bg);
    border: 1px solid var(--color-separator);
    font-size: 12px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  }
  .speaker.collapsed {
    padding: 4px;
    border-radius: 50%;
  }
  .info {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--color-fg-tertiary, #999);
  }
  .dot.on {
    background: var(--color-green, #34c759);
    animation: pulse 1s ease-in-out infinite;
  }
  .dot.paused {
    background: var(--color-orange, #ff9f0a);
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }
  .label {
    color: var(--color-fg-secondary);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .controls button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 50%;
    background: var(--color-bg-tertiary);
    color: var(--color-fg);
    cursor: pointer;
  }
  .controls button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .rate {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: 6px;
    color: var(--color-fg-secondary);
  }
  .rate input {
    width: 56px;
    accent-color: var(--color-accent);
  }
  .collapse,
  .expand {
    border: none;
    background: none;
    color: var(--color-fg-secondary);
    cursor: pointer;
    font-size: 12px;
    padding: 2px 4px;
  }
</style>
