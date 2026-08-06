<script lang="ts">
	let { agentCount = 0 }: { agentCount?: number } = $props();
</script>

<header class="dashboard-header">
	<div class="greeting">
		<h1>{agentCount > 0 ? 'Good to see you' : 'Welcome to Prism'}</h1>
		{#if agentCount > 0}
			<p>{agentCount} agent{agentCount !== 1 ? 's' : ''} ready</p>
		{:else}
			<p>Create an agent to get started</p>
		{/if}
	</div>
</header>

<style>
	.dashboard-header {
		padding: 24px 0 4px;
	}

	.greeting h1 {
		font-size: 24px;
		font-weight: 600;
		color: #171717;
		margin: 0;
		letter-spacing: -0.3px;
	}

	.greeting p {
		font-size: 14px;
		color: #6b6b6b;
		margin: 4px 0 0;
	}
</style>
