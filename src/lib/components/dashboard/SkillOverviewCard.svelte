<script lang="ts">
	import type { SkillOverview } from '$lib/stores/dashboard.svelte';

	let { skills }: { skills: SkillOverview | null } = $props();

	const percentage = $derived(
		skills && skills.total > 0 ? Math.round((skills.enabled / skills.total) * 100) : 0
	);
</script>

<div class="skill-card">
	<div class="card-header">
		<h3>技能总览</h3>
	</div>

	{#if skills}
		<div class="skill-stats">
			<div class="ring-wrapper">
				<svg viewBox="0 0 36 36" class="ring-svg">
					<circle
						cx="18" cy="18" r="15.9"
						fill="none"
						stroke="var(--color-separator)"
						stroke-width="3"
					/>
					<circle
						cx="18" cy="18" r="15.9"
						fill="none"
						stroke="var(--color-green)"
						stroke-width="3"
						stroke-dasharray="{percentage} {100 - percentage}"
						stroke-dashoffset="25"
						stroke-linecap="round"
					/>
				</svg>
				<span class="ring-label">{percentage}%</span>
			</div>
			<div class="skill-numbers">
				<div class="number-row">
					<span class="num-value">{skills.enabled}</span>
					<span class="num-label">已启用</span>
				</div>
				<div class="number-row">
					<span class="num-value">{skills.total}</span>
					<span class="num-label">总数</span>
				</div>
			</div>
		</div>

		{#if skills.popular.length > 0}
			<div class="popular-section">
				<span class="popular-label">热门技能</span>
				<div class="popular-tags">
					{#each skills.popular as skill}
						<span class="popular-tag">{skill}</span>
					{/each}
				</div>
			</div>
		{/if}
	{:else}
		<div class="empty">暂无技能数据</div>
	{/if}
</div>

<style>
	.skill-card {
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

	.skill-stats {
		display: flex;
		align-items: center;
		gap: 20px;
		margin-bottom: 16px;
	}

	.ring-wrapper {
		position: relative;
		width: 72px;
		height: 72px;
		flex-shrink: 0;
	}

	.ring-svg {
		width: 100%;
		height: 100%;
		transform: rotate(-90deg);
	}

	.ring-label {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		font-size: var(--text-footnote);
		font-weight: 700;
		color: var(--color-fg);
	}

	.skill-numbers {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.number-row {
		display: flex;
		align-items: baseline;
		gap: 8px;
	}

	.num-value {
		font-size: var(--text-title3);
		font-weight: 700;
		color: var(--color-fg);
	}

	.num-label {
		font-size: var(--text-caption1);
		color: var(--color-fg-secondary);
	}

	.popular-section {
		border-top: 1px solid var(--color-separator);
		padding-top: 12px;
	}

	.popular-label {
		font-size: var(--text-caption1);
		color: var(--color-fg-secondary);
		display: block;
		margin-bottom: 8px;
	}

	.popular-tags {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.popular-tag {
		padding: 3px 10px;
		border-radius: 9999px;
		background: rgba(0, 113, 227, 0.1);
		color: var(--color-accent);
		font-size: var(--text-caption2);
		font-weight: 500;
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
