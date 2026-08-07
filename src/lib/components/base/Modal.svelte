<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		open = $bindable(false),
		title = '',
		onclose,
		children
	}: {
		open?: boolean;
		title?: string;
		onclose?: () => void;
		children: Snippet;
	} = $props();

	function close() {
		open = false;
		onclose?.();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
	<div class="overlay" onclick={close} role="presentation">
		<div class="modal glass" onclick={(e) => e.stopPropagation()} role="dialog" aria-label={title}>
			{#if title}
				<div class="header">
					<h2>{title}</h2>
					<button class="close-btn" onclick={close}>×</button>
				</div>
			{/if}
			<div class="body">
				{@render children()}
			</div>
		</div>
	</div>
{/if}

<style>
	.overlay {
		position: fixed;
		inset: 0;
		background: var(--color-overlay);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		animation: fadeIn var(--duration-fast) ease;
	}
	.modal {
		border-radius: var(--radius-xl);
		min-width: 320px;
		max-width: 560px;
		max-height: 80vh;
		overflow: auto;
		animation: scaleIn var(--duration-base) var(--spring);
	}
	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-4) var(--space-6);
		border-bottom: 1px solid var(--color-separator);
	}
	.header h2 {
		font-size: var(--text-lg);
		font-weight: 600;
		margin: 0;
	}
	.close-btn {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		border: none;
		background: var(--color-bg-secondary);
		cursor: pointer;
		font-size: 18px;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.body { padding: var(--space-6); }

	@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
	@keyframes scaleIn { from { opacity: 0; transform: scale(0.95); } to { opacity: 1; transform: scale(1); } }
</style>
