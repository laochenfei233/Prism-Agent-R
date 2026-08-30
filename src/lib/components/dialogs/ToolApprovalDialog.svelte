<script lang="ts">
  import { listen, invoke } from '$lib/api/client';

  interface ToolApprovalRequest {
    call_id: string;
    agent_id: string;
    tool_name: string;
    risk_level: string;
    arguments: unknown;
    description?: string | null;
    kind?: 'tool' | 'delegation';
    session_id?: string | null;
    parent_agent_id?: string | null;
    child_agent_id?: string | null;
    task_summary?: string | null;
    capability_summary?: string | null;
    confirm_step?: number | null;
  }

  let visible = $state(false);
  let requests = $state<ToolApprovalRequest[]>([]);
  let index = $state(0);
  let rejectReason = $state('');

  listen('tool:approval-request', (event: { payload: ToolApprovalRequest }) => {
    const incoming = event.payload;
    if (!requests.some((item) => item.call_id === incoming.call_id)) {
      requests = [...requests, incoming];
    }
    visible = true;
  });

  function current() {
    return requests[index] ?? null;
  }

  async function respond(response: string) {
    const currentRequest = current();
    if (!currentRequest) return;
    await invoke('tool_approval_respond', {
      callId: currentRequest.call_id,
      response,
      reason: response === 'Rejected' ? rejectReason.trim() : undefined,
    });
    requests = requests.filter((item) => item.call_id !== currentRequest.call_id);
    rejectReason = '';
    if (requests.length === 0) {
      visible = false;
      index = 0;
    } else if (index >= requests.length) {
      index = requests.length - 1;
    }
  }

  function isDelegation() {
    return current()?.kind === 'delegation';
  }

  function isCriticalConfirm() {
    return current()?.confirm_step === 2;
  }

  function canAlwaysApprove() {
    return !isDelegation() && !current()?.confirm_step;
  }

  function displayAgent() {
    const request = current();
    if (!request) return '';
    if (request.kind === 'delegation') {
      return `${request.parent_agent_id ?? request.agent_id} → ${request.child_agent_id ?? '?'}`;
    }
    return request.agent_id;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && visible) {
      respond('Rejected');
    }
  }

</script>

<svelte:window on:keydown={handleKeydown} />

{#if visible && current()}
  <div class="overlay" role="presentation">
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="dialog glass"
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      tabindex="-1"
      aria-label="Tool Approval"
    >
      <div class="title-row">
        <h3 class="title">
          {isDelegation() ? 'Delegation Approval' : isCriticalConfirm() ? 'Critical Confirm' : 'Tool Approval'}
        </h3>
        {#if requests.length > 1}
          <span class="queue-badge">{index + 1} / {requests.length}</span>
        {/if}
      </div>
      <p class="agent-name">{displayAgent()} 请求执行：</p>

      <div class="tool-info">
        <span class="tool-name">{current()?.tool_name}</span>
        <span class="risk-badge risk-{current()?.risk_level?.toLowerCase() || 'low'}"
          >Risk: {current()?.risk_level}</span
        >
      </div>

      {#if isDelegation()}
        <div class="delegation-block">
          <h4>子任务</h4>
          <p class="task-summary">{current()?.task_summary || '无摘要'}</p>
          {#if current()?.capability_summary}
            <h4>子 Agent 能力</h4>
            <p class="task-summary">{current()?.capability_summary}</p>
          {/if}
        </div>
      {:else}
        <div class="params">
          <h4>Parameters</h4>
          <pre class="params-pre">{JSON.stringify(current()?.arguments, null, 2)}</pre>
        </div>
      {/if}

      {#if current()?.description}
        <p class="description">{current()?.description}</p>
      {/if}

      {#if current()?.confirm_step === 2}
        <p class="description">这是 Critical 操作的第二次确认，请再次核对后决定。</p>
      {/if}

      <label class="reason-field">
        <span>拒绝原因（拒绝时必填）</span>
        <input type="text" placeholder="说明拒绝理由…" bind:value={rejectReason} />
      </label>

      <div class="dialog-actions">
        <button class="btn-approve" onclick={() => respond('Approved')}>&#10003; Approve</button>
        <button class="btn-reject" onclick={() => respond('Rejected')}>&#10007; Reject</button>
        {#if canAlwaysApprove()}
          <button class="btn-always" onclick={() => respond('AlwaysApprove')}>
            Always Approve
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: var(--color-overlay);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1100;
    animation: fadeIn var(--duration-fast) ease;
  }

  .dialog {
    border-radius: var(--radius-xl);
    min-width: 380px;
    max-width: 520px;
    padding: var(--space-6);
    animation: scaleIn var(--duration-normal) var(--spring);
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-separator);
    box-shadow: var(--shadow-lg);
  }

  .title {
    margin: 0 0 var(--space-3);
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--color-fg);
  }

  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .queue-badge {
    flex-shrink: 0;
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--color-bg-tertiary);
    color: var(--color-fg-secondary);
    font-size: 12px;
    font-weight: 600;
  }

  .agent-name {
    margin: 0 0 var(--space-4);
    font-size: var(--text-sm);
    color: var(--color-fg-secondary);
  }

  .tool-info {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-4);
    background: var(--color-bg-secondary);
    border-radius: var(--radius-md);
    margin-bottom: var(--space-4);
  }

  .tool-name {
    font-family: monospace;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--color-fg);
  }

  .risk-badge {
    padding: 2px 10px;
    border-radius: var(--radius-sm);
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .risk-low {
    background: color-mix(in srgb, var(--color-green) 15%, transparent);
    color: var(--color-green);
  }

  .risk-medium {
    background: color-mix(in srgb, var(--color-orange) 15%, transparent);
    color: var(--color-orange);
  }

  .risk-high {
    background: color-mix(in srgb, var(--color-red) 15%, transparent);
    color: var(--color-red);
  }

  .params {
    margin-bottom: var(--space-4);
  }

  .params h4 {
    margin: 0 0 var(--space-2);
    font-size: 13px;
    font-weight: 600;
    color: var(--color-fg-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .params-pre {
    background: var(--color-bg-secondary);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    font-family: monospace;
    font-size: 13px;
    color: var(--color-fg);
    overflow-x: auto;
    margin: 0;
    max-height: 200px;
    overflow-y: auto;
  }

  .delegation-block {
    margin-bottom: var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--color-bg-secondary);
    border-radius: var(--radius-md);
  }

  .delegation-block h4 {
    margin: 0 0 var(--space-2);
    font-size: 13px;
    font-weight: 600;
    color: var(--color-fg-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .task-summary {
    margin: 0 0 var(--space-3);
    font-size: var(--text-sm);
    color: var(--color-fg);
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .reason-field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: var(--space-3) 0 0;
  }

  .reason-field span {
    font-size: 12px;
    color: var(--color-fg-secondary);
  }

  .reason-field input {
    padding: 8px 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-separator);
    background: var(--color-bg-secondary);
    color: var(--color-fg);
    font-size: 13px;
  }

  .description {
    margin: 0 0 var(--space-4);
    font-size: var(--text-sm);
    color: var(--color-fg-secondary);
    line-height: 1.5;
  }

  .dialog-actions {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }

  .btn-approve {
    flex: 1;
    padding: 10px 16px;
    border-radius: var(--radius-md);
    border: none;
    background: var(--color-green);
    color: #fff;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .btn-approve:hover {
    background: color-mix(in srgb, var(--color-green) 85%, #000);
  }
  .btn-approve:active {
    transform: scale(0.97);
  }

  .btn-reject {
    flex: 1;
    padding: 10px 16px;
    border-radius: var(--radius-md);
    border: none;
    background: var(--color-red);
    color: #fff;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .btn-reject:hover {
    background: color-mix(in srgb, var(--color-red) 85%, #000);
  }
  .btn-reject:active {
    transform: scale(0.97);
  }

  .btn-always {
    flex: 1;
    padding: 10px 16px;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-separator);
    background: var(--color-bg-secondary);
    color: var(--color-fg-secondary);
    font-size: 15px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .btn-always:hover {
    background: var(--color-bg-tertiary);
  }
  .btn-always:active {
    transform: scale(0.97);
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes scaleIn {
    from {
      opacity: 0;
      transform: scale(0.95);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
</style>
