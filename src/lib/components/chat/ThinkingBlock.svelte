<script lang="ts">
  import MarkdownViewer from './MarkdownViewer.svelte';

  interface Props {
    content: string;
    streaming?: boolean;
  }

  let { content, streaming = false }: Props = $props();
  let expanded = $state(false);

  function toggle() {
    expanded = !expanded;
  }
</script>

<div class="thinking-block" class:expanded>
  <button class="thinking-header" onclick={toggle}>
    <span class="thinking-icon">
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path
          d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 1 1 7.072 0l-.548.547A3.374 3.374 0 0 0 14 18.469V19a2 2 0 1 1-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z"
        />
      </svg>
    </span>
    <span class="thinking-label">{streaming ? '思考中...' : '已深度思考'}</span>
    <span class="thinking-chevron" class:expanded>&#9662;</span>
  </button>
  {#if expanded}
    <div class="thinking-content">
      <MarkdownViewer {content} {streaming} />
    </div>
  {/if}
</div>

<style>
  .thinking-block {
    margin-bottom: 0.5em;
    border: 1px solid var(--color-separator);
    border-radius: var(--radius-md);
    overflow: hidden;
    background: var(--color-bg-secondary);
  }

  .thinking-header {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 8px 12px;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--color-fg-secondary);
    font-size: var(--text-caption1);
    font-family: inherit;
    transition: color 0.15s ease;
  }

  .thinking-header:hover {
    color: var(--color-fg);
  }

  .thinking-icon {
    display: flex;
    align-items: center;
    color: var(--color-accent);
  }

  .thinking-label {
    flex: 1;
    text-align: left;
  }

  .thinking-chevron {
    font-size: 10px;
    transition: transform 0.2s ease;
  }

  .thinking-chevron.expanded {
    transform: rotate(180deg);
  }

  .thinking-content {
    padding: 0 12px 12px;
    border-top: 1px solid var(--color-separator);
    max-height: 400px;
    overflow-y: auto;
  }
</style>
