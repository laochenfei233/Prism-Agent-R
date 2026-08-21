<script lang="ts">
  import type { AgentContext } from '$lib/stores/context.svelte';
  import { mcpApi, type McpTool } from '$lib/api';

  let { data }: { data: AgentContext } = $props();

  const servers = $derived(data.mcp);
  let expandedId = $state<string | null>(null);

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
    if (expandedId === id) void fetchTools(id);
  }

  function statusColor(status: string): string {
    if (status === 'connected' || status === 'running') return 'var(--color-green)';
    if (status === 'error') return 'var(--color-red)';
    return 'var(--color-fg-secondary)';
  }

  // 工具列表（按服务器缓存）
  const toolsByServer = $state<Record<string, McpTool[]>>({});
  const toolsLoading = $state<Record<string, boolean>>({});

  async function fetchTools(serverId: string) {
    if (toolsByServer[serverId]) return;
    toolsLoading[serverId] = true;
    try {
      toolsByServer[serverId] = await mcpApi.tools(serverId);
    } catch {
      toolsByServer[serverId] = [];
    } finally {
      toolsLoading[serverId] = false;
    }
  }

  // 工具测试调用
  let expandedTool = $state<{ serverId: string; toolName: string } | null>(null);
  let toolArgs = $state('');
  let calling = $state(false);
  let callResult = $state<{ serverId: string; toolName: string; ok: boolean; text: string } | null>(
    null,
  );

  function openCall(serverId: string, toolName: string) {
    const same = expandedTool?.serverId === serverId && expandedTool.toolName === toolName;
    expandedTool = same ? null : { serverId, toolName };
    toolArgs = '';
    callResult = null;
  }

  async function runCall(serverId: string, toolName: string) {
    let parsed: unknown = {};
    if (toolArgs.trim()) {
      try {
        parsed = JSON.parse(toolArgs);
      } catch (e) {
        callResult = { serverId, toolName, ok: false, text: 'JSON 解析失败: ' + String(e) };
        return;
      }
    }
    calling = true;
    try {
      const result = await mcpApi.callTool(serverId, toolName, parsed);
      callResult = { serverId, toolName, ok: true, text: JSON.stringify(result, null, 2) };
    } catch (e) {
      callResult = { serverId, toolName, ok: false, text: String(e) };
    } finally {
      calling = false;
    }
  }
</script>

<div class="mcp-panel">
  {#if servers.length === 0}
    <div class="empty">
      <span>无 MCP 服务器</span>
    </div>
  {:else}
    <div class="server-list">
      {#each servers as server (server.id)}
        <div class="server-item">
          <button class="server-header" onclick={() => toggleExpand(server.id)}>
            <div class="status-dot" style:background={statusColor(server.status)}></div>
            <div class="server-info">
              <span class="server-name">{server.name}</span>
              <span class="server-status">{server.status}</span>
            </div>
            <span class="tool-count">{server.tools_count} 工具</span>
            <svg
              class="expand-icon"
              class:expanded={expandedId === server.id}
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </button>

          {#if expandedId === server.id}
            <div class="server-detail">
              {#if server.last_error}
                <div class="error-msg">{server.last_error}</div>
              {/if}
              <div class="detail-row">
                <span class="detail-label">状态</span>
                <span class="detail-value">{server.status}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">工具数</span>
                <span class="detail-value">{server.tools_count}</span>
              </div>

              <div class="tools-block">
                <div class="detail-label">工具</div>
                {#if toolsLoading[server.id]}
                  <div class="tools-hint">加载中…</div>
                {:else if (toolsByServer[server.id] || []).length === 0}
                  <div class="tools-hint">无工具</div>
                {:else}
                  {#each toolsByServer[server.id] as tool (tool.name)}
                    <div class="tool-item">
                      <div class="tool-info">
                        <span class="tool-name">{tool.name}</span>
                        {#if tool.description}
                          <span class="tool-desc">{tool.description}</span>
                        {/if}
                      </div>
                      <button
                        class="tool-call-btn"
                        class:active={expandedTool?.serverId === server.id &&
                          expandedTool?.toolName === tool.name}
                        onclick={() => openCall(server.id, tool.name)}
                      >
                        {expandedTool?.serverId === server.id &&
                        expandedTool?.toolName === tool.name
                          ? '收起'
                          : '调用'}
                      </button>
                    </div>
                    {#if expandedTool?.serverId === server.id && expandedTool?.toolName === tool.name}
                      <div class="call-box">
                        <input
                          class="call-input"
                          bind:value={toolArgs}
                          placeholder="JSON 参数，如 &#123;&#123;&quot;query&quot;: &quot;x&quot;&#125;&#125;（留空为 &#123;&#123;&#125;&#125;）"
                          onkeydown={(e) => {
                            if (e.key === 'Enter') runCall(server.id, tool.name);
                          }}
                        />
                        <button
                          class="tool-call-btn"
                          onclick={() => runCall(server.id, tool.name)}
                          disabled={calling}
                        >
                          {calling ? '执行中…' : '执行'}
                        </button>
                        {#if callResult && callResult.serverId === server.id && callResult.toolName === tool.name}
                          <pre
                            class="call-result"
                            class:error={!callResult.ok}>{callResult.text}</pre>
                        {/if}
                      </div>
                    {/if}
                  {/each}
                {/if}
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .mcp-panel {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px 0;
    font-size: 13px;
    color: var(--color-fg-secondary);
  }

  .server-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .server-item {
    background: var(--color-bg);
    border-radius: 8px;
    border: 1px solid var(--color-separator);
    overflow: hidden;
  }

  .server-header {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 10px 12px;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    color: var(--color-fg);
    transition: background 0.15s ease;
  }
  .server-header:hover {
    background: var(--color-bg-tertiary);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .server-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .server-name {
    font-size: 13px;
    font-weight: 500;
  }

  .server-status {
    font-size: 11px;
    color: var(--color-fg-secondary);
  }

  .tool-count {
    font-size: 11px;
    color: var(--color-fg-secondary);
    padding: 2px 6px;
    background: var(--color-bg-secondary);
    border-radius: 4px;
    flex-shrink: 0;
  }

  .expand-icon {
    color: var(--color-fg-secondary);
    flex-shrink: 0;
    transition: transform 0.2s ease;
  }
  .expand-icon.expanded {
    transform: rotate(180deg);
  }

  .server-detail {
    padding: 8px 12px 12px;
    border-top: 1px solid var(--color-separator);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .error-msg {
    font-size: 12px;
    color: var(--color-red);
    padding: 6px 8px;
    background: color-mix(in srgb, var(--color-red) 8%, transparent);
    border-radius: 4px;
    word-break: break-all;
  }

  .detail-row {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
  }

  .detail-label {
    color: var(--color-fg-secondary);
  }

  .detail-value {
    color: var(--color-fg);
    font-weight: 500;
  }

  .tools-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 4px;
  }

  .tools-hint {
    font-size: 12px;
    color: var(--color-fg-tertiary);
    padding: 4px 0;
  }

  .tool-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 5px 0;
    border-top: 1px solid var(--color-separator);
  }

  .tool-info {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .tool-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-fg);
  }

  .tool-desc {
    font-size: 11px;
    color: var(--color-fg-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tool-call-btn {
    padding: 3px 8px;
    border-radius: 4px;
    border: 1px solid var(--color-separator);
    background: var(--color-bg-secondary);
    color: var(--color-fg-secondary);
    font-size: 11px;
    flex-shrink: 0;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .tool-call-btn:hover {
    background: var(--color-bg-tertiary);
  }
  .tool-call-btn.active {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .tool-call-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .call-box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    margin-bottom: 6px;
    border-radius: 6px;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-separator);
  }

  .call-input {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    border-radius: 4px;
    border: 1px solid var(--color-separator);
    background: var(--color-bg);
    color: var(--color-fg);
    font-size: 12px;
    font-family: var(--font-mono, monospace);
    outline: none;
  }
  .call-input:focus {
    border-color: var(--color-accent);
  }

  .call-result {
    margin: 0;
    padding: 6px 8px;
    border-radius: 4px;
    background: var(--color-bg);
    border: 1px solid var(--color-separator);
    color: var(--color-fg);
    font-size: 11px;
    font-family: var(--font-mono, monospace);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 160px;
    overflow-y: auto;
  }
  .call-result.error {
    border-color: var(--color-red);
    color: var(--color-red);
  }
</style>
