<script lang="ts">
	import type { AgentContext } from '$lib/stores/context.svelte';

	let { data }: { data: AgentContext } = $props();

	const servers = $derived(data.mcp);
	let expandedId = $state<string | null>(null);

	function toggleExpand(id: string) {
		expandedId = expandedId === id ? null : id;
	}

	function statusColor(status: string): string {
		if (status === 'connected' || status === 'running') return 'var(--color-green, #10b981)';
		if (status === 'error') return 'var(--color-red, #ef4444)';
		return 'var(--color-fg-secondary)';
	}
</script>

<div class="mcp-panel">
	{#if servers.length === 0}
		<div class="empty">
			<span>无 MCP 服务器</span>
		</div>
	{:else}
		<div class="server-list">
			{#each servers as server}
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
							width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
						>
							<polyline points="6 9 12 15 18 9"/>
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
		color: var(--color-red, #ef4444);
		padding: 6px 8px;
		background: rgba(239, 68, 68, 0.08);
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
</style>
