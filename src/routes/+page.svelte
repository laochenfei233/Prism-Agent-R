<svelte:head>
	<title>Prism Agent</title>
</svelte:head>

<div class="home">
	<h1>Prism Agent</h1>
	<p>Rust-powered AI Agent Platform</p>
</div>

<style>
	.home {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 100vh;
		gap: 16px;
	}

	h1 {
		font-size: var(--text-4xl);
		font-weight: 700;
		color: var(--color-fg);
		margin: 0;
	}

	p {
		font-size: var(--text-lg);
		color: var(--color-fg-secondary);
		margin: 0;
	}
</style>
