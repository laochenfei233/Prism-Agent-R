<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		open = $bindable(false),
		position = 'right',
		onclose,
		children
	}: {
		open?: boolean;
		position?: 'left' | 'right' | 'bottom';
		onclose?: () => void;
		children: Snippet;
	} = $props();

	function close() {
		open = false;
		onclose?.();
	}
</script>

{#if open}
	<div class="overlay" onclick={close} role="presentation">
		<div
			class="sheet sheet-{position}"
			onclick={(e) => e.stopPropagation()}
			role="dialog"
		>
			{@render children()}
		</div>
	</div>
{/if}

<style>
	.overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.3);
		z-index: 900;
		animation: fadeIn var(--duration-fast) ease;
	}
	.sheet {
		background: var(--color-bg);
		box-shadow: var(--shadow-lg);
		overflow: auto;
		animation: slideIn var(--duration-base) var(--spring);
	}
	.sheet-right {
		position: absolute;
		top: 0;
		right: 0;
		bottom: 0;
		width: 400px;
		max-width: 90vw;
	}
	.sheet-left {
		position: absolute;
		top: 0;
		left: 0;
		bottom: 0;
		width: 400px;
		max-width: 90vw;
	}
	.sheet-bottom {
		position: absolute;
		left: 0;
		right: 0;
		bottom: 0;
		max-height: 70vh;
		border-radius: var(--radius-xl) var(--radius-xl) 0 0;
		padding: var(--space-6);
	}

	@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
	@keyframes slideIn {
		from { opacity: 0; transform: translateX(20px); }
		to { opacity: 1; transform: translateX(0); }
	}
</style>
