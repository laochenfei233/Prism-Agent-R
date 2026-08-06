<script lang="ts">
	import type { UsagePoint } from '$lib/stores/dashboard.svelte';

	let { data = [] }: { data?: UsagePoint[] } = $props();

	const width = 400;
	const height = 180;
	const padding = { top: 20, right: 16, bottom: 32, left: 50 };

	const chartW = $derived(width - padding.left - padding.right);
	const chartH = $derived(height - padding.top - padding.bottom);

	const maxTokens = $derived(
		data.length > 0 ? Math.max(...data.map((d) => d.tokens), 1) : 1
	);

	const points = $derived(
		data.map((d, i) => ({
			x: padding.left + (i / Math.max(data.length - 1, 1)) * chartW,
			y: padding.top + chartH - (d.tokens / maxTokens) * chartH,
			date: d.date,
			tokens: d.tokens,
		}))
	);

	const pathD = $derived(
		points.length > 1
			? points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ')
			: ''
	);

	const areaD = $derived(
		points.length > 1
			? pathD +
				` L ${points[points.length - 1].x} ${padding.top + chartH}` +
				` L ${points[0].x} ${padding.top + chartH} Z`
			: ''
	);

	const ticks = $derived(() => {
		const count = 4;
		return Array.from({ length: count + 1 }, (_, i) => {
			const val = (maxTokens / count) * i;
			const y = padding.top + chartH - (val / maxTokens) * chartH;
			let label: string;
			if (val >= 1_000_000) label = (val / 1_000_000).toFixed(1) + 'M';
			else if (val >= 1_000) label = (val / 1_000).toFixed(0) + 'K';
			else label = String(Math.round(val));
			return { y, label };
		});
	});

	function formatDate(dateStr: string): string {
		const d = new Date(dateStr);
		return `${d.getMonth() + 1}/${d.getDate()}`;
	}
</script>

<div class="chart-card">
	<div class="chart-header">
		<h3>用量趋势</h3>
		<span class="chart-subtitle">近 7 日</span>
	</div>

	{#if data.length === 0}
		<div class="chart-empty">暂无数据</div>
	{:else}
		<svg viewBox="0 0 {width} {height}" class="chart-svg">
			<!-- Y axis grid lines & labels -->
			{#each ticks() as tick}
				<line
					x1={padding.left}
					y1={tick.y}
					x2={width - padding.right}
					y2={tick.y}
					stroke="var(--color-separator)"
					stroke-dasharray="4 4"
				/>
				<text
					x={padding.left - 8}
					y={tick.y + 4}
					text-anchor="end"
					class="tick-label"
				>{tick.label}</text>
			{/each}

			<!-- X axis labels -->
			{#each points as p, i}
				{#if i % Math.max(1, Math.floor(points.length / 7)) === 0 || i === points.length - 1}
					<text
						x={p.x}
						y={height - 8}
						text-anchor="middle"
						class="tick-label"
					>{formatDate(p.date)}</text>
				{/if}
			{/each}

			<!-- Area fill -->
			<path d={areaD} fill="url(#areaGrad)" />

			<!-- Line -->
			<path d={pathD} fill="none" stroke="var(--color-accent)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />

			<!-- Data points -->
			{#each points as p}
				<circle cx={p.x} cy={p.y} r="3" fill="var(--color-accent)" stroke="var(--color-bg)" stroke-width="2" />
			{/each}

			<defs>
				<linearGradient id="areaGrad" x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stop-color="var(--color-accent)" stop-opacity="0.2" />
					<stop offset="100%" stop-color="var(--color-accent)" stop-opacity="0.02" />
				</linearGradient>
			</defs>
		</svg>
	{/if}
</div>

<style>
	.chart-card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		padding: 16px;
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.chart-header {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin-bottom: 12px;
	}

	.chart-header h3 {
		font-size: var(--text-headline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.chart-subtitle {
		font-size: var(--text-caption1);
		color: var(--color-fg-tertiary);
	}

	.chart-svg {
		width: 100%;
		flex: 1;
	}

	.chart-empty {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-fg-tertiary);
		font-size: var(--text-subheadline);
	}

	.tick-label {
		font-size: 10px;
		fill: var(--color-fg-tertiary);
		font-family: var(--font-sans);
	}
</style>
