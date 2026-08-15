<script lang="ts">
	import type { McpServerStatus } from '$lib/stores/dashboard.svelte';

	let { servers }: { servers: McpServerStatus[] } = $props();

	const connectedCount = $derived(servers.filter((s) => s.status === 'connected').length);
	const totalTools = $derived(servers.reduce((sum, s) => sum + s.tools_count, 0));

	function statusDot(status: string): string {
		switch (status) {
			case 'connected': return 'var(--color-green)';
			case 'error': return 'var(--color-red)';
			default: return 'var(--color-muted)';
		}
	}
</script>

<div class="mcp-card">
	<div class="card-header">
		<h3>MCP Servers</h3>
	</div>
	<div class="mcp-stats">
		<div class="stat-row">
			<span class="stat-label">Connected</span>
			<span class="stat-value">{connectedCount} / {servers.length}</span>
		</div>
		<div class="stat-row">
			<span class="stat-label">Total tools</span>
			<span class="stat-value">{totalTools}</span>
		</div>
	</div>
	{#if servers.length > 0}
		<div class="server-list">
			{#each servers.slice(0, 4) as server}
				<div class="server-row">
					<span class="dot" style:background={statusDot(server.status)}></span>
					<span class="name">{server.name}</span>
					<span class="tools">{server.tools_count}</span>
				</div>
			{/each}
		</div>
	{:else}
		<div class="empty">No servers configured</div>
	{/if}
</div>

<style>
	.mcp-card {
		background: var(--glass-solid-bg);
		backdrop-filter: var(--glass-solid-blur);
		-webkit-backdrop-filter: var(--glass-solid-blur);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--glass-edge-highlight), var(--shadow-sm);
		padding: 20px;
	}

	.card-header {
		margin-bottom: 14px;
	}

	.card-header h3 {
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.mcp-stats {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-bottom: 14px;
	}

	.stat-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.stat-label {
		font-size: 13px;
		color: var(--color-fg-secondary);
	}

	.stat-value {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg);
	}

	.server-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		border-top: 1px solid var(--color-separator);
		padding-top: 12px;
	}

	.server-row {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 13px;
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.name {
		flex: 1;
		color: var(--color-fg);
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.tools {
		color: var(--color-muted);
		font-variant-numeric: tabular-nums;
	}

	.empty {
		font-size: 13px;
		color: var(--color-muted);
	}
</style>
