<script lang="ts">
  import { page } from '$app/stores';
  import { invoke } from '$lib/api/client';
  import { agentApi, sessionLifecycleApi } from '$lib/api';
  import { agentStore } from '$lib/stores/agents.svelte';
  import { chatStore } from '$lib/stores/chat.svelte';

  import MessageList from '$lib/components/chat/MessageList.svelte';
  import Composer from '$lib/components/chat/Composer.svelte';
  import ModelSelector from '$lib/components/chat/ModelSelector.svelte';

  type ModelOption = { id: string; model_id: string; display_name?: string | null };
  interface CapForm {
    max_mcp_servers: string;
    max_mcp_tools: string;
    max_skills: string;
    max_total_tools: string;
    tool_allowlist: string;
    tool_denylist: string;
  }
  interface ApprovalRecord {
    id: string;
    kind: string;
    status: string;
    tool_name: string | null;
    child_agent_id: string | null;
    task_summary: string | null;
    decision: string | null;
    decision_reason: string | null;
    created_at: number;
  }

  let newAgentName = $state('');
  let showNewAgent = $state(false);
  let models = $state<ModelOption[]>([]);
  let expandedAgents = $state<Record<string, boolean>>({});
  let initStatus = $state<'idle' | 'initializing' | 'ok' | 'degraded' | 'failed'>('idle');
  let actionMessage = $state<string | null>(null);
  let initializationToken = 0;
  let showCapPanel = $state(false);
  let capForm = $state<CapForm>({
    max_mcp_servers: '0',
    max_mcp_tools: '0',
    max_skills: '0',
    max_total_tools: '0',
    tool_allowlist: '',
    tool_denylist: '',
  });
  let approvalHistory = $state<ApprovalRecord[]>([]);

  $effect(() => {
    void agentStore.loadAgents();
    void agentStore.loadAllSessions();
    void invoke<ModelOption[]>('model_list')
      .then((items) => {
        models = items;
      })
      .catch(() => {});
  });

  $effect(() => {
    if ($page.url.searchParams.get('new') === '1') {
      showNewAgent = true;
    }
  });

  $effect(() => {
    const session = agentStore.currentSession;
    const sessionId = session?.id;
    const agentId = session?.agent_id;
    if (!sessionId || !agentId || agentStore.currentAgent?.id !== agentId) return;
    void initializeSession(sessionId);
  });

  function isExpanded(agentId: string) {
    return (
      expandedAgents[agentId] ??
      (agentStore.currentAgent?.id === agentId ||
        (!agentStore.currentAgent && agentStore.agents[0]?.id === agentId))
    );
  }

  function toggleAgent(agentId: string) {
    expandedAgents = {
      ...expandedAgents,
      [agentId]: !isExpanded(agentId),
    };
  }

  function selectAgent(agentId: string) {
    const agent = agentStore.agents.find((item) => item.id === agentId);
    if (!agent) return;
    agentStore.selectAgent(agent);
    if (expandedAgents[agentId] === false) {
      expandedAgents = { ...expandedAgents, [agentId]: true };
    }
  }

  function sortedSessions(agentId: string) {
    return [...(agentStore.sessionsByAgent[agentId] ?? [])].sort((left, right) => {
      if (left.pinned !== right.pinned) return left.pinned ? -1 : 1;
      return right.updated_at - left.updated_at;
    });
  }

  async function createAgent() {
    if (!newAgentName.trim()) return;
    try {
      const agent = await agentStore.createAgent(newAgentName.trim());
      newAgentName = '';
      showNewAgent = false;
      expandedAgents = { ...expandedAgents, [agent.id]: true };
      agentStore.selectAgent(agent);
    } catch (error) {
      console.error('Failed to create agent:', error);
    }
  }

  async function handleNewSession(agentId: string) {
    const agent = agentStore.agents.find((item) => item.id === agentId);
    if (!agent) return;
    try {
      selectAgent(agent.id);
      expandedAgents = { ...expandedAgents, [agent.id]: true };
      await agentStore.createSession(agent.id, '新会话');
    } catch (error) {
      console.error('Failed to create session:', error);
      actionMessage = '创建会话失败，请重试';
    }
  }

  async function initializeSession(sessionId: string) {
    const token = ++initializationToken;
    initStatus = 'initializing';
    actionMessage = null;
    void chatStore.loadHistory(sessionId);

    try {
      const report = await sessionLifecycleApi.init(sessionId);
      if (token !== initializationToken) return;

      const healthy = report.provider_ok && report.memory_ok && report.mcp_ok;
      initStatus = healthy ? 'ok' : 'degraded';

      if (!healthy) {
        const problems = [
          !report.provider_ok && (report.provider_error ?? 'Provider 不可用'),
          !report.memory_ok && (report.memory_error ?? 'Memory 不可用'),
          !report.mcp_ok && (report.mcp_error ?? 'MCP 不可用'),
        ]
          .filter(Boolean)
          .join('；');
        actionMessage = problems || '部分组件初始化失败';
      }
    } catch (error) {
      if (token !== initializationToken) return;
      console.error('会话初始化失败:', error);
      initStatus = 'failed';
      actionMessage = error instanceof Error ? error.message : String(error);
    }
  }

  async function handleSend(content: string, attachments?: string[]) {
    if (!agentStore.currentSession) return;
    await chatStore.send(agentStore.currentSession.id, content, attachments);
  }

  async function handleSelectModel(modelId: string) {
    const agent = agentStore.currentAgent;
    if (!agent) return;
    try {
      await agentApi.update(agent.id, { model_id: modelId });
      await agentStore.loadAgents();
      agentStore.currentAgent = agentStore.agents.find((item) => item.id === agent.id) ?? agent;
    } catch (error) {
      console.error('Failed to update model:', error);
    }
  }

  function capsFromAgent(agent: { configuration?: Record<string, unknown> }): CapForm {
    const caps = (agent.configuration?.sub_agent_capabilities ?? {}) as Record<string, unknown>;
    return {
      max_mcp_servers: String(caps.max_mcp_servers ?? 0),
      max_mcp_tools: String(caps.max_mcp_tools ?? 0),
      max_skills: String(caps.max_skills ?? 0),
      max_total_tools: String(caps.max_total_tools ?? 0),
      tool_allowlist: Array.isArray(caps.tool_allowlist) ? caps.tool_allowlist.join(', ') : '',
      tool_denylist: Array.isArray(caps.tool_denylist) ? caps.tool_denylist.join(', ') : '',
    };
  }

  function toggleCapPanel() {
    const agent = agentStore.currentAgent;
    if (!agent) return;
    showCapPanel = !showCapPanel;
    if (showCapPanel) {
      capForm = capsFromAgent(agent);
      approvalHistory = [];
      void invoke<ApprovalRecord[]>('approval_history', { agentId: agent.id, limit: 15 })
        .then((rows) => (approvalHistory = rows))
        .catch(() => {});
    }
  }

  async function saveCapabilities() {
    const agent = agentStore.currentAgent;
    if (!agent) return;
    const toNumber = (value: string) => Math.max(0, Number.parseInt(value || '0', 10) || 0);
    const splitList = (value: string) =>
      value
        .split(',')
        .map((item) => item.trim())
        .filter(Boolean);
    try {
      await agentApi.update(agent.id, {
        configuration: {
          sub_agent_capabilities: {
            max_mcp_servers: toNumber(capForm.max_mcp_servers),
            max_mcp_tools: toNumber(capForm.max_mcp_tools),
            max_skills: toNumber(capForm.max_skills),
            max_total_tools: toNumber(capForm.max_total_tools),
            tool_allowlist: splitList(capForm.tool_allowlist),
            tool_denylist: splitList(capForm.tool_denylist),
          },
        },
      });
      await agentStore.loadAgents();
      actionMessage = '子 Agent 能力配额已保存，下一次委派生效';
    } catch (error) {
      actionMessage = error instanceof Error ? error.message : '保存配额失败';
    }
  }
</script>

<div class="agent-page">
  <aside class="agent-tree-pane">
    <div class="list-header">
      <span class="pane-title">Agent</span>
      <button
        class="icon-btn-sm"
        type="button"
        onclick={() => (showNewAgent = !showNewAgent)}
        aria-label="新建 Agent"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <line x1="12" y1="5" x2="12" y2="19" />
          <line x1="5" y1="12" x2="19" y2="12" />
        </svg>
      </button>
    </div>

    {#if showNewAgent}
      <div class="new-form">
        <input
          type="text"
          placeholder="Agent 名称"
          bind:value={newAgentName}
          onkeydown={(event) => event.key === 'Enter' && createAgent()}
          aria-label="Agent 名称"
        />
        <button class="btn-confirm" type="button" onclick={createAgent}>创建</button>
      </div>
    {/if}

    <div class="tree">
      {#each agentStore.agents as agent (agent.id)}
        {@const expanded = isExpanded(agent.id)}
        {@const sessions = sortedSessions(agent.id)}
        <div class="agent-group" class:active={agentStore.currentAgent?.id === agent.id}>
          <div class="agent-row">
            <button
              class="chevron"
              type="button"
              onclick={() => toggleAgent(agent.id)}
              aria-label={expanded ? `折叠 ${agent.name}` : `展开 ${agent.name}`}
              aria-expanded={expanded}
            >
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="m9 18 6-6-6-6" class:chevron-open={expanded} />
              </svg>
            </button>

            <button class="agent-name" type="button" onclick={() => selectAgent(agent.id)}>
              <span class="avatar">{agent.name[0]?.toUpperCase() ?? 'A'}</span>
              <span class="agent-label">{agent.name}</span>
              <span class="session-count">{sessions.length}</span>
            </button>

            <button
              class="add-btn"
              type="button"
              onclick={() => handleNewSession(agent.id)}
              title="新建对话"
              aria-label={`为 ${agent.name} 新建对话`}
            >
              <svg
                width="13"
                height="13"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <line x1="12" y1="5" x2="12" y2="19" />
                <line x1="5" y1="12" x2="19" y2="12" />
              </svg>
            </button>
          </div>

          {#if expanded}
            <div class="session-tree" role="group" aria-label={`${agent.name} sessions`}>
              {#if sessions.length === 0}
                <button
                  class="empty-session"
                  type="button"
                  onclick={() => handleNewSession(agent.id)}
                >
                  新建对话
                </button>
              {:else}
                {#each sessions as session (session.id)}
                  <button
                    class="session-item"
                    class:active={agentStore.currentSession?.id === session.id}
                    type="button"
                    onclick={() => {
                      selectAgent(agent.id);
                      agentStore.selectSession(session);
                    }}
                  >
                    <span>{session.title || '新会话'}</span>
                    {#if session.pinned}
                      <span class="pinned">Pinned</span>
                    {/if}
                  </button>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/each}

      {#if agentStore.agents.length === 0 && !showNewAgent}
        <div class="empty">
          <span>暂无 Agent</span>
          <button class="btn-text" type="button" onclick={() => (showNewAgent = true)}>创建</button>
        </div>
      {/if}
    </div>
  </aside>

  <div class="chat">
    <div class="chat-header">
      <div class="header-info">
        <h2>{agentStore.currentAgent?.name || 'Agent'}</h2>
        <span class="session-name">
          {agentStore.currentSession?.title || '选择或新建会话'}
        </span>
        {#if initStatus === 'initializing'}
          <span class="init-badge init-running">Initializing</span>
        {:else if initStatus === 'degraded'}
          <span class="init-badge init-degraded">Degraded</span>
        {:else if initStatus === 'failed'}
          <span class="init-badge init-failed">Failed</span>
        {:else if initStatus === 'ok'}
          <span class="init-badge init-ok">Ready</span>
        {/if}
      </div>
      <div class="header-spacer"></div>
      <button class="cap-btn" type="button" onclick={toggleCapPanel} title="子 Agent 能力配额与审批历史">
        子 Agent
      </button>
      <ModelSelector
        modelId={agentStore.currentAgent?.model_id ?? null}
        {models}
        onSelect={handleSelectModel}
      />
    </div>

    {#if showCapPanel && agentStore.currentAgent}
      <div class="cap-panel">
        <div class="cap-grid">
          <label>MCP Server 数上限
            <input type="number" min="0" bind:value={capForm.max_mcp_servers} />
          </label>
          <label>MCP 工具总数上限
            <input type="number" min="0" bind:value={capForm.max_mcp_tools} />
          </label>
          <label>Skill 数上限
            <input type="number" min="0" bind:value={capForm.max_skills} />
          </label>
          <label>总工具数上限
            <input type="number" min="0" bind:value={capForm.max_total_tools} />
          </label>
          <label>工具白名单（逗号分隔）
            <input type="text" placeholder="file_read, web_search" bind:value={capForm.tool_allowlist} />
          </label>
          <label>工具黑名单（逗号分隔）
            <input type="text" placeholder="task_delete" bind:value={capForm.tool_denylist} />
          </label>
        </div>
        <div class="cap-actions">
          <button class="btn-primary" type="button" onclick={saveCapabilities}>保存配额</button>
          <span class="cap-hint">0 表示不限制</span>
        </div>
        {#if approvalHistory.length > 0}
          <div class="approval-history">
            <h4>最近审批</h4>
            <ul>
              {#each approvalHistory.slice(0, 8) as record (record.id)}
                <li>
                  <span class="ah-kind">{record.kind === 'delegation' ? '委派' : '工具'}</span>
                  <span class="ah-tool">{record.tool_name ?? 'delegate_to_agent'}</span>
                  <span class="ah-status ah-{record.status}">{record.status}</span>
                  <span class="ah-reason">{record.decision_reason ?? ''}</span>
                </li>
              {/each}
            </ul>
          </div>
        {/if}
      </div>
    {/if}

    {#if actionMessage}
      <div class="action-message" role="status">{actionMessage}</div>
    {/if}

    {#if agentStore.currentSession}
      <MessageList
        messages={chatStore.messages}
        streaming={chatStore.streaming}
        streamingText={chatStore.streamingText}
        streamingReasoningText={chatStore.streamingReasoningText}
        streamingToolCalls={chatStore.streamingToolCalls}
      />
      <Composer
        disabled={chatStore.isGenerating}
        generating={chatStore.isGenerating}
        onSend={handleSend}
        onAbort={() => chatStore.abort(agentStore.currentSession?.id ?? '')}
      />
    {:else}
      <div class="chat-empty">
        <p>从左侧选择会话，或新建一个对话</p>
        <button
          class="btn-primary"
          type="button"
          onclick={() => agentStore.currentAgent && handleNewSession(agentStore.currentAgent.id)}
          disabled={!agentStore.currentAgent}
        >
          新建对话
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .agent-page {
    flex: 1;
    min-width: 0;
    display: flex;
    overflow: hidden;
  }

  .agent-tree-pane {
    width: 260px;
    min-width: 260px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-right: 1px solid var(--color-separator);
    background: var(--color-bg-secondary);
  }

  .list-header {
    height: 48px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--color-separator);
    flex-shrink: 0;
  }

  .pane-title {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-fg-secondary);
  }

  .icon-btn-sm,
  .chevron,
  .add-btn {
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--color-fg-secondary);
    cursor: pointer;
  }

  .icon-btn-sm:hover,
  .chevron:hover,
  .add-btn:hover {
    color: var(--color-fg);
    background: var(--color-bg-tertiary);
  }

  .chevron-open {
    transform: rotate(90deg);
  }

  .new-form {
    padding: 10px 12px;
    display: flex;
    gap: 8px;
    border-bottom: 1px solid var(--color-separator);
  }

  .new-form input {
    flex: 1;
    min-width: 0;
    padding: 7px 9px;
    border: 1px solid var(--color-separator);
    border-radius: 7px;
    background: var(--color-bg);
    color: var(--color-fg);
  }

  .btn-confirm {
    padding: 7px 10px;
    border: none;
    border-radius: 7px;
    background: var(--color-accent);
    color: #fff;
    cursor: pointer;
  }

  .tree {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
  }

  .agent-group {
    margin-bottom: 4px;
    border-radius: 9px;
  }

  .agent-group.active {
    background: color-mix(in srgb, var(--color-accent) 8%, transparent);
  }

  .agent-row {
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .agent-name {
    flex: 1;
    min-width: 0;
    height: 38px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 4px 0 6px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--color-fg);
    cursor: pointer;
    text-align: left;
  }

  .agent-name:hover {
    background: var(--color-bg-tertiary);
  }

  .avatar {
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 700;
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
  }

  .agent-label {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-count {
    font-size: 11px;
    color: var(--color-muted);
  }

  .session-tree {
    margin: 2px 0 4px 24px;
    padding-left: 8px;
    border-left: 1px solid var(--color-separator);
  }

  .session-item,
  .empty-session {
    width: 100%;
    min-height: 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 0 8px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-fg-secondary);
    cursor: pointer;
    text-align: left;
  }

  .session-item:hover,
  .empty-session:hover {
    color: var(--color-fg);
    background: var(--color-bg-tertiary);
  }

  .session-item.active {
    color: var(--color-accent);
    font-weight: 600;
    background: color-mix(in srgb, var(--color-accent) 11%, transparent);
  }

  .session-item span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pinned {
    font-size: 10px;
    color: var(--color-muted);
  }

  .empty-session {
    justify-content: center;
    font-size: 12px;
    border: 1px dashed var(--color-separator);
  }

  .empty {
    padding: 24px 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    color: var(--color-fg-secondary);
  }

  .btn-text {
    border: none;
    background: transparent;
    color: var(--color-accent);
    cursor: pointer;
  }

  .chat {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .chat-header {
    height: 52px;
    padding: 0 16px;
    display: flex;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid var(--color-separator);
  }

  .header-info {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .header-info h2 {
    margin: 0;
    font-size: 16px;
    color: var(--color-fg);
    white-space: nowrap;
  }

  .session-name {
    color: var(--color-fg-secondary);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-spacer {
    flex: 1;
  }

  .init-badge {
    padding: 2px 7px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
  }

  .init-running {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }

  .init-ok {
    color: var(--color-green);
    background: color-mix(in srgb, var(--color-green) 10%, transparent);
  }

  .init-degraded {
    color: var(--color-yellow, #f59e0b);
    background: color-mix(in srgb, var(--color-yellow, #f59e0b) 10%, transparent);
  }

  .init-failed {
    color: var(--color-red);
    background: color-mix(in srgb, var(--color-red) 10%, transparent);
  }

  .action-message {
    margin: 8px 16px 0;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, var(--color-yellow, #f59e0b) 25%, transparent);
    background: color-mix(in srgb, var(--color-yellow, #f59e0b) 10%, transparent);
    color: var(--color-fg-secondary);
    font-size: 12px;
  }

  .chat-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--color-fg-secondary);
  }

  .btn-primary {
    padding: 8px 14px;
    border: none;
    border-radius: 8px;
    background: var(--color-accent);
    color: #fff;
    cursor: pointer;
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .cap-btn {
    padding: 6px 10px;
    border-radius: 8px;
    border: 1px solid var(--color-separator);
    background: var(--color-bg-secondary);
    color: var(--color-fg-secondary);
    font-size: 12px;
    cursor: pointer;
  }

  .cap-panel {
    margin: 10px 16px 0;
    padding: 14px 16px;
    border-radius: 12px;
    border: 1px solid var(--color-separator);
    background: var(--color-bg-elevated);
  }

  .cap-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px 14px;
  }

  .cap-grid label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--color-fg-secondary);
  }

  .cap-grid input {
    padding: 7px 10px;
    border-radius: 8px;
    border: 1px solid var(--color-separator);
    background: var(--color-bg-secondary);
    color: var(--color-fg);
    font-size: 13px;
  }

  .cap-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
  }

  .cap-hint {
    font-size: 12px;
    color: var(--color-fg-secondary);
  }

  .approval-history {
    margin-top: 14px;
  }

  .approval-history h4 {
    margin: 0 0 8px;
    font-size: 13px;
    color: var(--color-fg-secondary);
  }

  .approval-history ul {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 180px;
    overflow-y: auto;
  }

  .approval-history li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--color-separator) 60%, transparent);
    font-size: 12px;
  }

  .ah-kind {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--color-bg-tertiary);
    color: var(--color-fg-secondary);
  }

  .ah-tool {
    font-family: monospace;
    color: var(--color-fg);
  }

  .ah-status {
    flex-shrink: 0;
    font-weight: 600;
  }

  .ah-approved {
    color: var(--color-green);
  }

  .ah-rejected,
  .ah-expired {
    color: var(--color-red);
  }

  .ah-deferred {
    color: var(--color-yellow, #f59e0b);
  }

  .ah-reason {
    color: var(--color-fg-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 720px) {
    .agent-tree-pane {
      width: 210px;
      min-width: 210px;
    }
  }
</style>
