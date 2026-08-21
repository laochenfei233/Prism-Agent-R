<script lang="ts">
  import { traceApi, type AgentTrace } from '$lib/api';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();

  let traces = $state<AgentTrace[]>([]);
  let loading = $state(false);
  let expandedId = $state<string | null>(null);
  let msg = $state('');

  let minGrade = $state<number | undefined>(undefined);
  let toolFailedOnly = $state(false);

  async function loadTraces() {
    loading = true;
    try {
      traces = await traceApi.list(sessionId, 50, minGrade, toolFailedOnly || undefined);
    } catch (e) {
      msg = '加载失败: ' + String(e);
    } finally {
      loading = false;
    }
  }

  function getGradeColor(score: number | null): string {
    if (score === null) return 'var(--color-fg-tertiary)';
    if (score >= 0.8) return 'var(--color-green)';
    if (score >= 0.5) return 'var(--color-orange)';
    return 'var(--color-red)';
  }

  function formatTime(ts: number): string {
    return new Date(ts).toLocaleString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  $effect(() => {
    if (sessionId) loadTraces();
  });
</script>

<div class="trace-replay">
  <div class="header">
    <h3 class="title">轨迹回放</h3>
    <div class="filters">
      <select bind:value={minGrade} onchange={() => loadTraces()}>
        <option value={undefined}>全部</option>
        <option value={0.5}>≥0.5</option>
        <option value={0.8}>≥0.8</option>
      </select>
      <label class="filter-label">
        <input type="checkbox" bind:checked={toolFailedOnly} onchange={() => loadTraces()} />
        仅失败
      </label>
    </div>
  </div>

  {#if msg}
    <p class="message">{msg}</p>
  {/if}

  {#if loading}
    <p class="empty">加载中...</p>
  {:else if traces.length === 0}
    <p class="empty">暂无轨迹记录</p>
  {:else}
    <div class="trace-list">
      {#each traces as trace (trace.id)}
        <div class="trace-item" class:expanded={expandedId === trace.id}>
          <button class="trace-header" onclick={() => toggleExpand(trace.id)}>
            <span class="trace-time">{formatTime(trace.started_at)}</span>
            <span class="trace-agent">{trace.agent_id.slice(0, 8)}</span>
            <span
              class="badge"
              class:badge-success={trace.outcome === 'success'}
              class:badge-error={trace.outcome !== 'success'}
            >
              {trace.outcome}
            </span>
            {#if trace.grade_score !== null}
              <span class="trace-grade" style:color={getGradeColor(trace.grade_score)}>
                {(trace.grade_score * 100).toFixed(0)}分
              </span>
            {:else}
              <span class="trace-grade ungraded">未评分</span>
            {/if}
            <span class="expand-icon">{expandedId === trace.id ? '▼' : '▶'}</span>
          </button>

          {#if expandedId === trace.id}
            <div class="trace-detail">
              <div class="steps-timeline">
                {#each trace.steps as step (step.step_index)}
                  <div class="step-item">
                    <div class="step-marker">
                      <span class="step-index">#{step.step_index + 1}</span>
                      <span class="step-kind">{step.kind}</span>
                      {#if step.tool_name}
                        <span class="step-tool">[{step.tool_name}]</span>
                      {/if}
                    </div>
                    <div class="step-content">
                      <p class="step-text">{step.input_summary.slice(0, 100)}</p>
                    </div>
                    <span class="step-latency">{step.latency_ms}ms</span>
                  </div>
                {/each}
              </div>

              {#if trace.grade_score !== null}
                <div class="grade-info">
                  <span class="grade-label">评分:</span>
                  <span class="grade-score" style:color={getGradeColor(trace.grade_score)}>
                    {trace.grade_score.toFixed(2)}
                  </span>
                  {#if trace.grade_reason}
                    <span class="grade-reason">{trace.grade_reason}</span>
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .trace-replay {
    padding: 1rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-elevated);
  }
  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  .title {
    font-size: 1rem;
    font-weight: 600;
    margin: 0;
  }
  .filters {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .filter-label {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.8125rem;
    color: var(--color-fg-secondary);
  }
  .message {
    font-size: 0.8125rem;
    margin-bottom: 0.5rem;
    color: var(--color-fg-secondary);
  }
  .empty {
    font-size: 0.875rem;
    color: var(--color-fg-tertiary);
    text-align: center;
    padding: 2rem;
  }
  .trace-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .trace-item {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg);
    overflow: hidden;
  }
  .trace-item.expanded {
    border-color: var(--color-border-strong);
  }
  .trace-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem;
    width: 100%;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
  }
  .trace-header:hover {
    background: var(--color-bg-hover);
  }
  .trace-time {
    font-size: 0.75rem;
    color: var(--color-fg-tertiary);
    font-family: var(--font-mono);
  }
  .trace-agent {
    font-size: 0.8125rem;
    color: var(--color-fg-secondary);
  }
  .badge {
    padding: 0.125rem 0.375rem;
    border-radius: var(--radius-sm);
    font-size: 0.75rem;
  }
  .badge-success {
    background: var(--color-green);
    color: white;
  }
  .badge-error {
    background: var(--color-red);
    color: white;
  }
  .trace-grade {
    font-size: 0.8125rem;
    font-weight: 600;
    margin-left: auto;
  }
  .trace-grade.ungraded {
    color: var(--color-fg-tertiary);
    font-weight: normal;
  }
  .expand-icon {
    font-size: 0.625rem;
    color: var(--color-fg-tertiary);
  }
  .trace-detail {
    padding: 0 0.75rem 0.75rem;
    border-top: 1px solid var(--color-border);
  }
  .steps-timeline {
    margin-top: 0.75rem;
  }
  .step-item {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 0.5rem;
    padding: 0.5rem;
    border-left: 2px solid var(--color-border);
    margin-left: 0.25rem;
    font-size: 0.75rem;
  }
  .step-marker {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .step-index {
    color: var(--color-fg-tertiary);
    font-family: var(--font-mono);
  }
  .step-kind {
    color: var(--color-fg-secondary);
  }
  .step-tool {
    color: var(--color-accent);
  }
  .step-text {
    margin: 0;
    color: var(--color-fg-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .step-latency {
    color: var(--color-fg-tertiary);
    font-family: var(--font-mono);
  }
  .grade-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
    padding: 0.5rem;
    background: var(--color-bg-hover);
    border-radius: var(--radius-sm);
  }
  .grade-label {
    font-size: 0.8125rem;
    color: var(--color-fg-secondary);
  }
  .grade-score {
    font-weight: 600;
  }
  .grade-reason {
    font-size: 0.75rem;
    color: var(--color-fg-tertiary);
  }
</style>
