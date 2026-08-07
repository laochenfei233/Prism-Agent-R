<script lang="ts">
	import type { SkillOverview } from '$lib/stores/dashboard.svelte';

	let { skills }: { skills: SkillOverview | null } = $props();
</script>

<div class="skill-card">
	<div class="card-header">
		<h3>Skills</h3>
	</div>
	{#if skills}
		<div class="skill-stats">
			<div class="stat-row">
				<span class="stat-label">Enabled</span>
				<span class="stat-value">{skills.enabled} / {skills.total}</span>
			</div>
		</div>
		{#if skills.popular.length > 0}
			<div class="popular">
				{#each skills.popular.slice(0, 3) as skill}
					<span class="tag">{skill}</span>
				{/each}
			</div>
		{/if}
	{:else}
		<div class="empty">No skills installed</div>
	{/if}
</div>

<style>
	.skill-card {
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-sm);
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

	.skill-stats {
		margin-bottom: 12px;
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

	.popular {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.tag {
		padding: 3px 10px;
		border-radius: 6px;
		background: var(--color-bg-hover);
		color: var(--color-fg-secondary);
		font-size: 12px;
		font-weight: 500;
	}

	.empty {
		font-size: 13px;
		color: var(--color-muted);
	}
</style>
