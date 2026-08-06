<script lang="ts">
	import type { McpServerStatus } from '$lib/stores/dashboard.svelte';

	let { servers }: { servers: McpServerStatus[] } = $props();

	const connectedCount = $derived(servers.filter((s) => s.status === 'connected').length);
	const totalTools = $derived(servers.reduce((sum, s) => sum + s.tools_count, 0));

	function statusDot(status: string): string {
		switch (status) {
			case 'connected': return '#22c55e';
			case 'error': return '#ef4444';
			default: return '#a0a0a0';
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
		background: #f7f7f8;
		border: 1px solid rgba(0, 0, 0, 0.06);
		border-radius: 12px;
		padding: 20px;
	}

	.card-header {
		margin-bottom: 14px;
	}

	.card-header h3 {
		font-size: 15px;
		font-weight: 600;
		color: #171717;
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
		color: #6b6b6b;
	}

	.stat-value {
		font-size: 13px;
		font-weight: 600;
		color: #171717;
	}

	.server-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		border-top: 1px solid rgba(0, 0, 0, 0.06);
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
		color: #171717;
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.tools {
		color: #a0a0a0;
		font-variant-numeric: tabular-nums;
	}

	.empty {
		font-size: 13px;
		color: #a0a0a0;
	}
</style>
