<script lang="ts">
  import type { SessionLifecycle } from '$lib/api';

  interface Props {
    lifecycle: SessionLifecycle;
    showTooltip?: boolean;
  }

  let { lifecycle, showTooltip = true }: Props = $props();

  const statusConfig: Record<SessionLifecycle, { color: string; label: string }> = {
    Created: { color: 'var(--color-fg-tertiary)', label: '新建' },
    Init: { color: 'var(--color-bg-hover)', label: '初始化中' },
    Ready: { color: 'var(--color-green)', label: '就绪' },
    Running: { color: 'var(--color-accent)', label: '运行中' },
    Paused: { color: 'var(--color-orange)', label: '已暂停' },
    Verifying: { color: 'var(--color-accent)', label: '验证中' },
    Done: { color: 'var(--color-fg-tertiary)', label: '完成' },
    InitFailed: { color: 'var(--color-orange)', label: '初始化失败' },
  };

  let config = $derived(statusConfig[lifecycle] || statusConfig.Created);
</script>

<span
  class="status-badge"
  style:color={config.color}
  title={showTooltip ? config.label : undefined}
>
  <span class="status-dot" style:background={config.color}></span>
  {#if !showTooltip}
    <span class="status-text">{config.label}</span>
  {/if}
</span>

<style>
  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 0.75rem;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-text {
    color: var(--color-fg-secondary);
  }
</style>
