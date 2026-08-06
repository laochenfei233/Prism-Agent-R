<script lang="ts">
	import type { McpServerStatus } from '$lib/stores/dashboard.svelte';

	let { servers }: { servers: McpServerStatus[] } = $props();

	const connectedCount = $derived(servers.filter((s) => s.status === 'connected').length);
	const totalTools = $derived(servers.reduce((sum, s) => sum + s.tools_count, 0));

	function statusColor(status: string): string {
		switch (status) {
			case 'connected': return 'var(--color-green)';
			case 'error': return 'var(--color-red)';
			default: return 'var(--color-gray)';
		}
	}

	function statusLabel(status: string): string {
		switch (status) {
			case 'connected': return '已连接';
			case 'error': return '错误';
			case 'disconnected': return '未连接';
			default: return status;
		}
	}
</script>

<div class="mcp-card">
	<div class="card-header">
		<h3>MCP 服务</h3>
	</div>

	<div class="mcp-summary">
		<div class="summary-item">
			<span class="summary-dot" style:background={connectedCount > 0 ? 'var(--color-green)' : 'var(--color-gray)'}></span>
			<span class="summary-value">{connectedCount}/{servers.length}</span>
			<span class="summary-label">已连接</span>
		</div>
		<div class="summary-divider"></div>
		<div class="summary-item">
			<span class="summary-value">{totalTools}</span>
			<span class="summary-label">工具总数</span>
		</div>
	</div>

	{#if servers.length > 0}
		<div class="server-list">
			{#each servers as server}
				<div class="server-row">
					<span class="server-dot" style:background={statusColor(server.status)}></span>
					<span class="server-name">{server.name}</span>
					<span class="server-tools">{server.tools_count} 工具</span>
					<span class="server-status">{statusLabel(server.status)}</span>
				</div>
			{/each}
		</div>
	{:else}
		<div class="empty">暂无 MCP 服务</div>
	{/if}
</div>

<style>
	.mcp-card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		padding: 16px;
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.card-header {
		margin-bottom: 16px;
	}

	.card-header h3 {
		font-size: var(--text-headline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.mcp-summary {
		display: flex;
		align-items: center;
		gap: 20px;
		margin-bottom: 16px;
	}

	.summary-item {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.summary-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.summary-value {
		font-size: var(--text-title3);
		font-weight: 700;
		color: var(--color-fg);
	}

	.summary-label {
		font-size: var(--text-caption1);
		color: var(--color-fg-secondary);
	}

	.summary-divider {
		width: 1px;
		height: 24px;
		background: var(--color-separator);
	}

	.server-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		flex: 1;
		overflow-y: auto;
	}

	.server-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		background: var(--color-bg);
	}

	.server-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.server-name {
		font-size: var(--text-subheadline);
		font-weight: 500;
		color: var(--color-fg);
		flex: 1;
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.server-tools {
		font-size: var(--text-caption2);
		color: var(--color-fg-secondary);
		flex-shrink: 0;
	}

	.server-status {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
		flex-shrink: 0;
	}

	.empty {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-fg-tertiary);
		font-size: var(--text-subheadline);
	}
</style>
