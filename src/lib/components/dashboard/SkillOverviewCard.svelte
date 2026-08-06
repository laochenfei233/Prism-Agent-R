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
		color: #6b6b6b;
	}

	.stat-value {
		font-size: 13px;
		font-weight: 600;
		color: #171717;
	}

	.popular {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.tag {
		padding: 3px 10px;
		border-radius: 6px;
		background: rgba(0, 0, 0, 0.04);
		color: #6b6b6b;
		font-size: 12px;
		font-weight: 500;
	}

	.empty {
		font-size: 13px;
		color: #a0a0a0;
	}
</style>
