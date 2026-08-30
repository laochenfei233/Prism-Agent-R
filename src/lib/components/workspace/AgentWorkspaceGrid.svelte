<script lang="ts">
  import { goto } from '$app/navigation';
  import { listen } from '$lib/api/client';
  import { traceApi, type AgentTrace } from '$lib/api';
  import type { TaskItem } from '$lib/api';
  import type { KanbanCard } from '$lib/stores/dashboard.svelte';
  import { agentStore } from '$lib/stores/agents.svelte';
  import { dashboardStore } from '$lib/stores/dashboard.svelte';

  type WorkspaceStatus =
    'Idle' | 'Ready' | 'Running' | 'Paused' | 'Done' | 'Failed' | 'Uninitialized';

  type TraceState = {
    sessionId: string;
    status: 'loading' | 'loaded' | 'error';
    trace: AgentTrace | null;
  };

  type WorkspaceCard = {
    agentId: string;
    name: string;
    isOrchestrator: boolean;
    avatar: string | null;
    modelName: string | null;
    sessionId: string | null;
    sessionTitle: string | null;
    updatedAt: number | null;
    messageCount: number;
    status: WorkspaceStatus;
    tasks: TaskItem[];
    doingTask: TaskItem | null;
  };

  let gridElement = $state<HTMLElement | null>(null);
  let visibleAgentIds = $state<Set<string>>(new Set());
  let traceStates = $state<Record<string, TraceState>>({});
  let traceRequestTokens = $state<Record<string, number>>({});
  let traceRefreshToken = $state(0);
  let actionError = $state<string | null>(null);
  let openingAgentId = $state<string | null>(null);

  const kanbanByAgent = $derived.by(() => {
    const data = dashboardStore.kanban;
    return [data?.idle, data?.running, data?.done, data?.failed]
      .flat()
      .filter((card): card is KanbanCard => Boolean(card))
      .reduce<Record<string, KanbanCard>>((grouped, card) => {
        grouped[card.agent_id] = card;
        return grouped;
      }, {});
  });

  const modelByAgent = $derived.by(() => {
    const summaries = dashboardStore.overview?.agents ?? [];
    return summaries.reduce<Record<string, string | null>>((grouped, agent) => {
      grouped[agent.id] = agent.model_name;
      return grouped;
    }, {});
  });

  const tasksByAgent = $derived.by(() => {
    return dashboardStore.tasks.reduce<Record<string, TaskItem[]>>((grouped, task) => {
      const ownerId = task.owner;
      if (!ownerId) return grouped;
      grouped[ownerId] = [...(grouped[ownerId] ?? []), task];
      return grouped;
    }, {});
  });

  const cards = $derived.by<WorkspaceCard[]>(() => {
    return [...agentStore.agents]
      .sort((left, right) => left.order_key - right.order_key)
      .map((agent) => {
        const kanban = kanbanByAgent[agent.id];
        const tasks = tasksByAgent[agent.id] ?? [];
        return {
          agentId: agent.id,
          name: agent.name,
          isOrchestrator: agent.is_orchestrator,
          avatar: agent.avatar,
          modelName: modelByAgent[agent.id] ?? kanban?.model_name ?? agent.model_id ?? null,
          sessionId: kanban?.session_id ?? null,
          sessionTitle: kanban?.session_title ?? null,
          updatedAt: kanban?.session_updated_at ?? null,
          messageCount: kanban?.message_count ?? 0,
          status: deriveStatus(kanban),
          tasks,
          doingTask: tasks.find((task) => task.status.toLowerCase() === 'doing') ?? null,
        };
      });
  });

  const runningCount = $derived(
    cards.filter((card) => card.status === 'Running' || card.status === 'Paused').length,
  );
  const failedCount = $derived(cards.filter((card) => card.status === 'Failed').length);

  const columnCount = $derived(cards.length <= 9 ? 3 : cards.length <= 12 ? 4 : 5);
  const minimumRowCount = $derived(cards.length <= 9 ? 3 : 3);
  const rowCount = $derived(Math.max(minimumRowCount, Math.ceil(cards.length / columnCount)));
  const capacity = $derived(columnCount * rowCount);
  const adderCount = $derived(cards.length < capacity ? 1 : 0);
  const placeholderCount = $derived(Math.max(0, capacity - cards.length - adderCount));

  $effect(() => {
    void dashboardStore.loadOverview();
    void dashboardStore.loadKanban();
    void dashboardStore.loadTasks();
    void agentStore.loadAllSessions();
  });

  $effect(() => {
    const element = gridElement;
    const currentCards = cards;
    if (!element || currentCards.length === 0) {
      visibleAgentIds = new Set();
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        const nextVisible = new Set(visibleAgentIds);
        for (const entry of entries) {
          const agentId = (entry.target as HTMLElement).dataset.agentId;
          if (!agentId) continue;
          if (entry.isIntersecting) {
            nextVisible.add(agentId);
          } else {
            nextVisible.delete(agentId);
          }
        }
        visibleAgentIds = nextVisible;
      },
      { rootMargin: '120px' },
    );

    for (const card of currentCards) {
      const cardElement = element.querySelector<HTMLElement>(`[data-agent-id="${card.agentId}"]`);
      if (cardElement) observer.observe(cardElement);
    }

    return () => observer.disconnect();
  });

  $effect(() => {
    const currentCards = cards;
    const visible = visibleAgentIds;
    void traceRefreshToken;

    for (const card of currentCards) {
      if (!visible.has(card.agentId) || !card.sessionId) continue;
      void loadTrace(card.agentId, card.sessionId);
    }
  });

  $effect(() => {
    let disposed = false;
    const unlisteners: (() => void)[] = [];

    const attach = (event: string, refreshKanban = false) => {
      return listen<unknown>(event, () => {
        traceRefreshToken += 1;
        if (refreshKanban) void dashboardStore.loadKanban();
      }).then((unlisten) => {
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      });
    };

    void attach('chat:stream:done', true);
    void attach('chat:stream:error', true);

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  });

  function deriveStatus(card: KanbanCard | undefined): WorkspaceStatus {
    if (!card) return 'Uninitialized';
    if (!card.session_id) return 'Uninitialized';

    switch (card.lifecycle) {
      case 'Init':
      case 'Running':
      case 'Verifying':
        return 'Running';
      case 'Paused':
        return 'Paused';
      case 'InitFailed':
        return 'Failed';
      case 'Done':
        return 'Done';
      case 'Ready':
        return 'Ready';
      case 'Created':
        return card.message_count > 0 ? 'Done' : 'Idle';
      default:
        return 'Idle';
    }
  }

  async function loadTrace(agentId: string, sessionId: string) {
    const current = traceStates[agentId];
    if (current?.sessionId === sessionId && current.status === 'loading') return;

    const token = (traceRequestTokens[agentId] ?? 0) + 1;
    traceRequestTokens = { ...traceRequestTokens, [agentId]: token };
    traceStates = {
      ...traceStates,
      [agentId]: { sessionId, status: 'loading', trace: null },
    };

    try {
      const [trace] = await traceApi.list(sessionId, 1);
      if (traceRequestTokens[agentId] !== token) return;
      traceStates = {
        ...traceStates,
        [agentId]: { sessionId, status: 'loaded', trace: trace ?? null },
      };
    } catch {
      if (traceRequestTokens[agentId] !== token) return;
      traceStates = {
        ...traceStates,
        [agentId]: { sessionId, status: 'error', trace: null },
      };
    }
  }

  function activeTraceStep(agentId: string, sessionId: string | null) {
    const state = traceStates[agentId];
    if (!sessionId || state?.sessionId !== sessionId || state.trace?.finished_at !== null) {
      return null;
    }
    return state.trace.steps.at(-1);
  }

  function callLabel(agentId: string, sessionId: string | null) {
    const state = traceStates[agentId];
    if (!sessionId) return 'No recent calls';
    if (!state || state.sessionId !== sessionId || state.status === 'loading') {
      return 'Syncing...';
    }
    if (state.status === 'error') return 'Unavailable';

    const trace = state.trace;
    if (!trace) return 'No recent calls';
    if (trace.finished_at === null) {
      const currentStep = trace.steps.at(-1);
      return currentStep?.tool_name || currentStep?.kind || 'Calling...';
    }

    return trace.outcome.toLowerCase() === 'success'
      ? `Success · ${formatLatency(trace.finished_at - trace.started_at)}`
      : `Failed · ${latestError(trace) || 'call failed'}`;
  }

  function callMeta(agentId: string, sessionId: string | null) {
    const trace = traceStates[agentId]?.trace;
    if (!sessionId || trace?.finished_at === null) return '';
    if (!trace) return '';

    const input = trace.total_prompt_tokens;
    const output = trace.total_completion_tokens;
    return `${formatTokens(input)} in · ${formatTokens(output)} out · ${formatCost(trace.total_cost)}`;
  }

  function taskProgress(card: WorkspaceCard) {
    const done = card.tasks.filter((task) => task.status.toLowerCase() === 'done').length;
    return `${done} / ${card.tasks.length}`;
  }

  function progressPercent(card: WorkspaceCard) {
    if (card.tasks.length === 0) return 0;
    const done = card.tasks.filter((task) => task.status.toLowerCase() === 'done').length;
    return Math.round((done / card.tasks.length) * 100);
  }

  function latestError(trace: AgentTrace) {
    return [...trace.steps].reverse().find((step) => step.error)?.error ?? null;
  }

  function formatTokens(value: number) {
    if (value < 1000) return String(value);
    return `${(value / 1000).toFixed(value < 10000 ? 1 : 0)}k`;
  }

  function formatCost(value: number) {
    if (value === 0) return '$0';
    if (value < 1) return `$${value.toFixed(3)}`;
    return `$${value.toFixed(2)}`;
  }

  function formatLatency(milliseconds: number) {
    if (milliseconds < 1000) return `${Math.max(0, Math.round(milliseconds))}ms`;
    return `${(milliseconds / 1000).toFixed(1)}s`;
  }

  function formatRelative(timestamp: number | null) {
    if (!timestamp) return 'No sessions';
    const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });
    const elapsed = timestamp - Date.now();
    const minutes = Math.round(elapsed / (1000 * 60));
    if (Math.abs(minutes) < 60) return formatter.format(minutes, 'minute');
    const hours = Math.round(elapsed / (1000 * 60 * 60));
    if (Math.abs(hours) < 24) return formatter.format(hours, 'hour');
    return formatter.format(Math.round(elapsed / (1000 * 60 * 60 * 24)), 'day');
  }

  async function openAgent(card: WorkspaceCard) {
    const agent = agentStore.agents.find((item) => item.id === card.agentId);
    if (!agent || openingAgentId) return;

    openingAgentId = agent.id;
    actionError = null;
    try {
      await agentStore.loadAllSessions();
      const session = agentStore.sessions.find((item) => item.id === card.sessionId);
      agentStore.selectAgent(agent);

      if (session) {
        agentStore.selectSession(session);
      } else {
        await agentStore.createSession(agent.id, '新会话');
      }
      await goto('/agent');
    } catch (error) {
      actionError = error instanceof Error ? error.message : String(error);
    } finally {
      openingAgentId = null;
    }
  }

  function createAgent() {
    goto('/agent?new=1');
  }
</script>

<section class="workspace">
  <header class="workspace-header">
    <div class="title-block">
      <h1>Agent Workspace</h1>
      <p>{cards.length} agents · {runningCount} active</p>
    </div>
    <div class="summary">
      <span class="chip chip-running">Running {runningCount}</span>
      <span class="chip" class:chip-failed={failedCount > 0}>
        Failed {failedCount}
      </span>
      <button class="primary-btn" type="button" onclick={createAgent}> New Agent </button>
    </div>
  </header>

  {#if dashboardStore.overview && dashboardStore.overview.models.length === 0}
    <div class="setup-warning">
      <span>Provider 或模型未配置，Agent 可能无法调用。</span>
      <button type="button" onclick={() => goto('/settings')}>Settings</button>
    </div>
  {/if}

  {#if actionError}
    <div class="action-error" role="alert">{actionError}</div>
  {/if}

  {#if agentStore.agents.length === 0 && !dashboardStore.loading}
    <div class="empty-state">
      <h2>No agents yet</h2>
      <p>创建第一个 Agent 后，它会作为工作区卡片出现在这里。</p>
      <button type="button" onclick={createAgent}>Create Agent</button>
    </div>
  {:else}
    <div
      class="grid"
      class:columns-3={columnCount === 3}
      class:columns-4={columnCount === 4}
      class:columns-5={columnCount === 5}
      bind:this={gridElement}
    >
      {#each cards as card (card.agentId)}
        <button
          type="button"
          class="agent-card status-{card.status.toLowerCase()}"
          data-agent-id={card.agentId}
          disabled={openingAgentId === card.agentId}
          onclick={() => openAgent(card)}
        >
          <div class="identity">
            <div class="avatar">
              {#if card.avatar}
                <img src={card.avatar} alt="" />
              {:else}
                <span>{card.name[0]?.toUpperCase() ?? 'A'}</span>
              {/if}
            </div>
            <div class="identity-text">
              <div class="name-row">
                <strong>{card.name}</strong>
                {#if card.isOrchestrator}
                  <span class="orchestrator-badge">Orchestrator</span>
                {/if}
              </div>
              <span>{card.modelName || 'No model'}</span>
            </div>
            <span class="status-badge">{card.status}</span>
          </div>

          <div class="section">
            <div class="section-label">
              <span>Progress</span>
              <span>{taskProgress(card)}</span>
            </div>
            <div class="progress">
              <div class="progress-value" style:width="{progressPercent(card)}%"></div>
            </div>
            <p class="current-task">
              {#if card.doingTask}
                {card.doingTask.subject}
              {:else if activeTraceStep(card.agentId, card.sessionId)}
                {activeTraceStep(card.agentId, card.sessionId)?.tool_name ||
                  activeTraceStep(card.agentId, card.sessionId)?.kind ||
                  'Working...'}
              {:else}
                No tasks
              {/if}
            </p>
          </div>

          <div class="section">
            <div class="section-label">
              <span>Model Call</span>
              <span>{formatRelative(card.updatedAt)}</span>
            </div>
            <p
              class="call-label"
              class:call-running={callLabel(card.agentId, card.sessionId) === 'Calling...'}
              class:call-failed={callLabel(card.agentId, card.sessionId).startsWith('Failed')}
            >
              {callLabel(card.agentId, card.sessionId)}
            </p>
            <p class="call-meta">{callMeta(card.agentId, card.sessionId)}</p>
          </div>
        </button>
      {/each}

      {#if adderCount > 0}
        <button type="button" class="add-card" onclick={createAgent}>
          <span>+</span>
          <strong>Add Agent</strong>
        </button>
      {/if}

      {#each Array(placeholderCount) as _placeholder, index (index)}
        <div class="placeholder" aria-hidden="true"></div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .workspace {
    flex: 1;
    min-width: 0;
    padding: 24px 32px 40px;
    overflow-y: auto;
  }

  .workspace-header {
    max-width: 1560px;
    margin: 0 auto 20px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .title-block h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 700;
    color: var(--color-fg);
  }

  .title-block p {
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--color-fg-secondary);
  }

  .summary {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .chip {
    padding: 5px 10px;
    border-radius: 999px;
    border: 1px solid var(--color-separator);
    color: var(--color-fg-secondary);
    background: var(--color-bg-secondary);
    font-size: 12px;
    white-space: nowrap;
  }

  .chip-running {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
    border-color: color-mix(in srgb, var(--color-accent) 24%, transparent);
  }

  .chip-failed {
    color: var(--color-red);
    background: color-mix(in srgb, var(--color-red) 10%, transparent);
    border-color: color-mix(in srgb, var(--color-red) 24%, transparent);
  }

  .primary-btn,
  .setup-warning button,
  .empty-state button {
    border: none;
    border-radius: 8px;
    background: var(--color-accent);
    color: #fff;
    padding: 8px 14px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }

  .setup-warning,
  .action-error {
    max-width: 1560px;
    margin: 0 auto 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 10px;
    font-size: 13px;
  }

  .setup-warning {
    color: var(--color-fg-secondary);
    background: color-mix(in srgb, var(--color-yellow, #f59e0b) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-yellow, #f59e0b) 22%, transparent);
  }

  .action-error {
    color: var(--color-red);
    background: color-mix(in srgb, var(--color-red) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-red) 24%, transparent);
  }

  .empty-state {
    max-width: 1560px;
    min-height: 320px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
    color: var(--color-fg-secondary);
  }

  .empty-state h2 {
    margin: 0;
    color: var(--color-fg);
  }

  .empty-state p {
    margin: 0 0 12px;
  }

  .grid {
    max-width: 1560px;
    margin: 0 auto;
    display: grid;
    grid-auto-rows: minmax(190px, auto);
    gap: 14px;
  }

  .grid.columns-3 {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .grid.columns-4 {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .grid.columns-5 {
    grid-template-columns: repeat(5, minmax(0, 1fr));
  }

  .agent-card,
  .add-card {
    min-width: 0;
    min-height: 190px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px;
    text-align: left;
    overflow: hidden;
    border: 1px solid var(--color-separator);
    border-radius: 14px;
    background: var(--color-bg-secondary);
    color: var(--color-fg);
    cursor: pointer;
    transition:
      border-color 0.15s ease,
      transform 0.15s ease,
      box-shadow 0.15s ease;
  }

  .agent-card:hover,
  .add-card:hover {
    transform: translateY(-2px);
    border-color: color-mix(in srgb, var(--color-accent) 38%, transparent);
    box-shadow: var(--shadow-md);
  }

  .agent-card:focus-visible,
  .add-card:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
  }

  .agent-card:disabled {
    opacity: 0.65;
    cursor: progress;
  }

  .status-running {
    border-color: color-mix(in srgb, var(--color-accent) 45%, transparent);
  }

  .status-failed {
    border-color: color-mix(in srgb, var(--color-red) 45%, transparent);
  }

  .identity {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }

  .avatar {
    width: 38px;
    height: 38px;
    flex-shrink: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 11px;
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    color: var(--color-accent);
    font-weight: 700;
  }

  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .identity-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .name-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .identity-text strong {
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .identity-text .orchestrator-badge {
    flex-shrink: 0;
    padding: 2px 7px;
    border-radius: 999px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.03em;
    color: #fff;
    background: linear-gradient(135deg, var(--color-accent), color-mix(in srgb, var(--color-accent) 55%, #7c3aed));
  }

  .identity-text span {
    font-size: 12px;
    color: var(--color-fg-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-badge {
    flex-shrink: 0;
    padding: 3px 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    color: var(--color-fg-secondary);
    background: color-mix(in srgb, var(--color-fg) 7%, transparent);
  }

  .status-ready .status-badge,
  .status-done .status-badge {
    color: var(--color-green);
    background: color-mix(in srgb, var(--color-green) 11%, transparent);
  }

  .status-running .status-badge {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 11%, transparent);
  }

  .status-paused .status-badge {
    color: var(--color-yellow, #f59e0b);
    background: color-mix(in srgb, var(--color-yellow, #f59e0b) 11%, transparent);
  }

  .status-failed .status-badge {
    color: var(--color-red);
    background: color-mix(in srgb, var(--color-red) 11%, transparent);
  }

  .section {
    min-width: 0;
  }

  .section-label {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 7px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--color-muted);
  }

  .progress {
    height: 5px;
    overflow: hidden;
    border-radius: 999px;
    background: color-mix(in srgb, var(--color-fg) 8%, transparent);
  }

  .progress-value {
    height: 100%;
    border-radius: inherit;
    background: var(--color-accent);
    transition: width 0.2s ease;
  }

  .current-task,
  .call-label,
  .call-meta {
    margin: 0;
    font-size: 12px;
    color: var(--color-fg-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .current-task {
    margin-top: 7px;
  }

  .call-label {
    color: var(--color-fg);
    font-weight: 600;
  }

  .call-running {
    color: var(--color-accent);
  }

  .call-failed {
    color: var(--color-red);
  }

  .call-meta {
    margin-top: 4px;
  }

  .add-card {
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-style: dashed;
    color: var(--color-fg-secondary);
    background: transparent;
  }

  .add-card span {
    font-size: 24px;
    line-height: 1;
    color: var(--color-accent);
  }

  .placeholder {
    min-height: 190px;
    border-radius: 14px;
  }

  @media (max-width: 1400px) {
    .grid.columns-5 {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }
  }

  @media (max-width: 1180px) {
    .workspace {
      padding: 20px 20px 32px;
    }

    .grid.columns-4,
    .grid.columns-5 {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  @media (max-width: 900px) {
    .workspace-header {
      align-items: flex-start;
      flex-direction: column;
    }

    .grid,
    .grid.columns-3,
    .grid.columns-4,
    .grid.columns-5 {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 620px) {
    .grid,
    .grid.columns-3,
    .grid.columns-4,
    .grid.columns-5 {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
