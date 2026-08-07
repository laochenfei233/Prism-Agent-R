<script lang="ts">
	import type { UsageStats } from '$lib/stores/dashboard.svelte';

	let { usage }: { usage: UsageStats | null } = $props();

	function formatTokens(n: number): string {
		if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
		if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
		return String(n);
	}

	function formatCost(n: number): string {
		return '$' + n.toFixed(2);
	}
</script>

<div class="usage-card">
	<div class="card-header">
		<h3>Usage</h3>
	</div>
	<div class="stats-list">
		<div class="stat-row">
			<span class="stat-label">Today</span>
			<span class="stat-value">{formatTokens(usage?.today_tokens ?? 0)} tokens</span>
		</div>
		<div class="stat-row">
			<span class="stat-label">This week</span>
			<span class="stat-value">{formatTokens(usage?.week_tokens ?? 0)} tokens</span>
		</div>
		<div class="stat-row">
			<span class="stat-label">Month cost</span>
			<span class="stat-value">{formatCost(usage?.month_cost ?? 0)}</span>
		</div>
		<div class="stat-row">
			<span class="stat-label">Calls today</span>
			<span class="stat-value">{usage?.today_calls ?? 0}</span>
		</div>
	</div>
</div>

<style>
	.usage-card {
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-sm);
		padding: 20px;
	}

	.card-header {
		margin-bottom: 16px;
	}

	.card-header h3 {
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.stats-list {
		display: flex;
		flex-direction: column;
		gap: 12px;
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
		font-variant-numeric: tabular-nums;
	}
</style>
