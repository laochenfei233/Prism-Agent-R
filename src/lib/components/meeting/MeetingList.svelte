<script lang="ts">
  import type { MeetingDto } from '$lib/api';
  import EmptyState from '$lib/components/base/EmptyState.svelte';
  import Skeleton from '$lib/components/base/Skeleton.svelte';
  import Badge from '$lib/components/base/Badge.svelte';

  let {
    meetings = [],
    loading = false,
    onselect,
    ondelete,
    oncreate,
  }: {
    meetings?: MeetingDto[];
    loading?: boolean;
    onselect?: (meeting: MeetingDto) => void;
    ondelete?: (meeting: MeetingDto) => void;
    oncreate?: () => void;
  } = $props();

  function formatDate(ts: number): string {
    return new Date(ts * 1000).toLocaleDateString('zh-CN', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  function formatDuration(seconds: number): string {
    if (seconds <= 0) return '--';
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>

<div class="meeting-list">
  {#if loading}
    <div class="list">
      {#each Array.from({ length: 4 }, (_, i) => i) as i (i)}
        <div class="meeting-item skeleton-item">
          <Skeleton lines={2} />
        </div>
      {/each}
    </div>
  {:else if meetings.length === 0}
    <EmptyState icon="🎙️" title="暂无会议记录" description="创建一个会议来开始记录和转写">
      {#snippet action()}
        <button class="btn-primary" onclick={oncreate}>创建会议</button>
      {/snippet}
    </EmptyState>
  {:else}
    <div class="list">
      {#each meetings as meeting (meeting.id)}
        <div
          class="meeting-item"
          role="button"
          tabindex="0"
          onclick={() => onselect?.(meeting)}
          onkeydown={(e) => e.key === 'Enter' && onselect?.(meeting)}
        >
          <div class="item-main">
            <div class="item-header">
              <h3 class="item-title">{meeting.title}</h3>
              <button
                class="delete-btn"
                onclick={(e) => {
                  e.stopPropagation();
                  ondelete?.(meeting);
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
            <div class="item-meta">
              <span>{formatDate(meeting.created_at)}</span>
              {#if meeting.recording_duration > 0}
                <span class="dot">·</span>
                <span>{formatDuration(meeting.recording_duration)}</span>
              {/if}
              {#if meeting.participants.length > 0}
                <span class="dot">·</span>
                <span>{meeting.participants.length} 人参会</span>
              {/if}
            </div>
          </div>
          <div class="item-status">
            {#if meeting.summary}
              <Badge variant="success">已摘要</Badge>
            {:else if meeting.transcript}
              <Badge variant="accent">已转写</Badge>
            {:else}
              <Badge>待处理</Badge>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .meeting-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-4);
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-separator);
    border-radius: var(--radius-md);
    cursor: pointer;
    transition:
      border-color var(--duration-fast),
      box-shadow var(--duration-fast);
  }
  .meeting-item:hover {
    border-color: var(--color-border-strong);
    box-shadow: var(--shadow-sm);
  }

  .skeleton-item {
    cursor: default;
    min-height: 64px;
  }

  .item-main {
    flex: 1;
    min-width: 0;
  }

  .item-header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .item-title {
    font-size: var(--text-headline);
    font-weight: 600;
    color: var(--color-fg);
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
    flex-shrink: 0;
  }
  .meeting-item:hover .delete-btn {
    opacity: 1;
  }
  .delete-btn:hover {
    background: var(--color-bg-tertiary);
    color: var(--color-red);
  }

  .item-meta {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    font-size: var(--text-xs);
    color: var(--color-fg-secondary);
    margin-top: var(--space-1);
  }

  .dot {
    color: var(--color-fg-tertiary);
  }

  .item-status {
    flex-shrink: 0;
    margin-left: var(--space-3);
  }
</style>
