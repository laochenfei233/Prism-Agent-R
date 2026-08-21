<script lang="ts">
  import type { AgentContext } from '$lib/stores/context.svelte';
  import Progress from '$lib/components/base/Progress.svelte';

  let { data }: { data: AgentContext } = $props();

  const usage = $derived(data.session_usage);
  const contextPercent = $derived(
    usage.context_limit > 0 ? Math.round((usage.context_used / usage.context_limit) * 100) : 0,
  );
  const contextVariant = $derived(
    contextPercent > 90 ? 'error' : contextPercent > 70 ? 'warning' : 'default',
  );

  function formatTokens(n: number): string {
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
    if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
    return n.toString();
  }

  function formatCost(n: number): string {
    return '$' + n.toFixed(4);
  }
</script>

<div class="usage-panel">
  <!-- Context Window -->
  <div class="section">
    <div class="section-label">上下文窗口</div>
    <div class="context-bar">
      <div class="context-info">
        <span>{formatTokens(usage.context_used)}</span>
        <span class="separator">/</span>
        <span class="limit">{formatTokens(usage.context_limit)}</span>
      </div>
      <Progress value={usage.context_used} max={usage.context_limit} variant={contextVariant} />
      <div class="percent">{contextPercent}%</div>
    </div>
  </div>

  <!-- Session Stats -->
  <div class="section">
    <div class="section-label">会话统计</div>
    <div class="stats-grid">
      <div class="stat">
        <span class="stat-value">{formatTokens(usage.input_tokens)}</span>
        <span class="stat-label">输入 Token</span>
      </div>
      <div class="stat">
        <span class="stat-value">{formatTokens(usage.output_tokens)}</span>
        <span class="stat-label">输出 Token</span>
      </div>
      <div class="stat">
        <span class="stat-value">{usage.tool_calls}</span>
        <span class="stat-label">工具调用</span>
      </div>
      <div class="stat">
        <span class="stat-value">{formatCost(usage.cost_est)}</span>
        <span class="stat-label">估算费用</span>
      </div>
    </div>
  </div>

  <!-- Today -->
  <div class="section">
    <div class="section-label">今日累计</div>
    <div class="stats-grid">
      <div class="stat">
        <span class="stat-value">{usage.today_calls}</span>
        <span class="stat-label">调用次数</span>
      </div>
      <div class="stat">
        <span class="stat-value">{formatTokens(usage.today_tokens)}</span>
        <span class="stat-label">总 Token</span>
      </div>
      <div class="stat">
        <span class="stat-value">{formatCost(usage.today_cost)}</span>
        <span class="stat-label">总费用</span>
      </div>
    </div>
  </div>
</div>

<style>
  .usage-panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .section-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-fg-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .context-bar {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .context-info {
    display: flex;
    align-items: baseline;
    gap: 2px;
    font-size: 13px;
    color: var(--color-fg);
  }

  .separator {
    color: var(--color-fg-secondary);
  }

  .limit {
    color: var(--color-fg-secondary);
  }

  .percent {
    font-size: 11px;
    color: var(--color-fg-secondary);
    text-align: right;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    background: var(--color-bg);
    border-radius: 8px;
    border: 1px solid var(--color-separator);
  }

  .stat-value {
    font-size: 15px;
    font-weight: 600;
    color: var(--color-fg);
    font-variant-numeric: tabular-nums;
  }

  .stat-label {
    font-size: 11px;
    color: var(--color-fg-secondary);
  }
</style>
