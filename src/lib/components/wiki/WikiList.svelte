<script lang="ts">
  import type { WikiDto } from '$lib/api';
  import EmptyState from '$lib/components/base/EmptyState.svelte';
  import Skeleton from '$lib/components/base/Skeleton.svelte';

  let {
    wikis = [],
    loading = false,
    onselect,
    ondelete,
    oncreate,
  }: {
    wikis?: WikiDto[];
    loading?: boolean;
    onselect?: (wiki: WikiDto) => void;
    ondelete?: (wiki: WikiDto) => void;
    oncreate?: () => void;
  } = $props();

  function formatDate(ts: number): string {
    return new Date(ts * 1000).toLocaleDateString('zh-CN', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  }
</script>

<div class="wiki-list">
  {#if loading}
    <div class="grid">
      {#each Array.from({ length: 3 }, (_, i) => i) as i (i)}
        <div class="card skeleton-card">
          <Skeleton lines={2} />
        </div>
      {/each}
    </div>
  {:else if wikis.length === 0}
    <EmptyState icon="📚" title="暂无 Wiki 知识库" description="创建一个知识库来组织和管理你的文档">
      {#snippet action()}
        <button class="btn-primary" onclick={oncreate}>创建知识库</button>
      {/snippet}
    </EmptyState>
  {:else}
    <div class="grid">
      {#each wikis as wiki (wiki.id)}
        <div
          class="card"
          role="button"
          tabindex="0"
          onclick={() => onselect?.(wiki)}
          onkeydown={(e) => e.key === 'Enter' && onselect?.(wiki)}
        >
          <div class="card-header">
            <div class="card-icon">📚</div>
            <button
              class="delete-btn"
              onclick={(e) => {
                e.stopPropagation();
                ondelete?.(wiki);
              }}
              title="删除"
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                ><polyline points="3 6 5 6 21 6" /><path
                  d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                /></svg
              >
            </button>
          </div>
          <h3 class="card-title">{wiki.name}</h3>
          {#if wiki.description}
            <p class="card-desc">{wiki.description}</p>
          {/if}
          <div class="card-meta">{formatDate(wiki.updated_at)}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--space-3);
  }

  .card {
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-separator);
    border-radius: var(--radius-md);
    padding: var(--space-4);
    cursor: pointer;
    transition:
      border-color var(--duration-fast),
      box-shadow var(--duration-fast);
  }
  .card:hover {
    border-color: var(--color-border-strong);
    box-shadow: var(--shadow-sm);
  }

  .skeleton-card {
    cursor: default;
    min-height: 120px;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-2);
  }

  .card-icon {
    font-size: 24px;
  }

  .delete-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--color-fg-secondary);
    cursor: pointer;
    opacity: 0;
    transition:
      opacity 0.15s,
      background 0.15s;
  }
  .card:hover .delete-btn {
    opacity: 1;
  }
  .delete-btn:hover {
    background: var(--color-bg-tertiary);
    color: var(--color-red);
  }

  .card-title {
    font-size: var(--text-headline);
    font-weight: 600;
    color: var(--color-fg);
    margin: 0 0 var(--space-1);
  }

  .card-desc {
    font-size: var(--text-sm);
    color: var(--color-fg-secondary);
    margin: 0 0 var(--space-2);
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .card-meta {
    font-size: var(--text-xs);
    color: var(--color-fg-tertiary);
  }
</style>
