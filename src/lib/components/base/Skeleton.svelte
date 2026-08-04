<script lang="ts">
	let {
		lines = 3,
		variant = 'text'
	}: {
		lines?: number;
		variant?: 'text' | 'circle' | 'card';
	} = $props();
</script>

{#if variant === 'circle'}
	<div class="skeleton skeleton-circle"></div>
{:else if variant === 'card'}
	<div class="skeleton skeleton-card">
		<div class="skeleton-line" style:width="60%"></div>
		<div class="skeleton-line" style:width="100%"></div>
		<div class="skeleton-line" style:width="80%"></div>
	</div>
{:else}
	<div class="skeleton-group">
		{#each Array(lines) as _, i}
			<div
				class="skeleton skeleton-line"
				style:width={i === lines - 1 ? '70%' : '100%'}
			></div>
		{/each}
	</div>
{/if}

<style>
	.skeleton {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		animation: pulse 1.5s ease-in-out infinite;
	}
	.skeleton-line {
		height: 14px;
		margin-bottom: var(--space-2);
	}
	.skeleton-circle {
		width: 48px;
		height: 48px;
		border-radius: 50%;
	}
	.skeleton-card {
		padding: var(--space-4);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.skeleton-group {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.4; }
	}
</style>
