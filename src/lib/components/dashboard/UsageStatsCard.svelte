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

	const stats = $derived(usage ? [
		{ label: '今日 Tokens', value: formatTokens(usage.today_tokens), icon: '⚡', color: 'var(--color-accent)' },
		{ label: '本周 Tokens', value: formatTokens(usage.week_tokens), icon: '📊', color: 'var(--color-purple)' },
		{ label: '本月费用', value: formatCost(usage.month_cost), icon: '💰', color: 'var(--color-green)' },
		{ label: '调用次数', value: String(usage.today_calls), icon: '🔄', color: 'var(--color-orange)' },
	] : []);
</script>

<div class="stats-grid">
	{#each stats as stat}
		<div class="stat-card">
			<div class="stat-icon" style:background="{stat.color}15" style:color={stat.color}>
				{stat.icon}
			</div>
			<div class="stat-content">
				<div class="stat-value">{stat.value}</div>
				<div class="stat-label">{stat.label}</div>
			</div>
		</div>
	{/each}
</div>

<style>
	.stats-grid {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 12px;
		padding: 0 24px;
	}

	.stat-card {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 16px;
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		transition: transform 0.15s var(--ease-default);
	}

	.stat-card:hover {
		transform: translateY(-1px);
	}

	.stat-icon {
		width: 40px;
		height: 40px;
		border-radius: var(--radius-sm);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 18px;
		flex-shrink: 0;
	}

	.stat-content {
		min-width: 0;
	}

	.stat-value {
		font-size: var(--text-headline);
		font-weight: 700;
		color: var(--color-fg);
		line-height: 1.2;
	}

	.stat-label {
		font-size: var(--text-caption1);
		color: var(--color-fg-secondary);
		margin-top: 2px;
	}

	@media (max-width: 900px) {
		.stats-grid {
			grid-template-columns: repeat(2, 1fr);
		}
	}
</style>
