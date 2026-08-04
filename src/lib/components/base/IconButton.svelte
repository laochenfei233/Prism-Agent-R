<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		disabled = false,
		size = 'md',
		tooltip = '',
		onclick,
		children
	}: {
		disabled?: boolean;
		size?: 'sm' | 'md' | 'lg';
		tooltip?: string;
		onclick?: () => void;
		children: Snippet;
	} = $props();
</script>

<button
	class="icon-btn icon-btn-{size}"
	{disabled}
	{onclick}
	title={tooltip}
>
	{@render children()}
</button>

<style>
	.icon-btn {
		border-radius: var(--radius-md);
		border: none;
		cursor: pointer;
		background: transparent;
		color: var(--color-fg-secondary);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		transition: background var(--duration-fast), color var(--duration-fast);
	}
	.icon-btn:hover:not(:disabled) { background: var(--color-bg-secondary); color: var(--color-fg); }
	.icon-btn:active:not(:disabled) { transform: scale(0.92); }
	.icon-btn:disabled { opacity: 0.3; cursor: not-allowed; }

	.icon-btn-sm { width: 28px; height: 28px; }
	.icon-btn-md { width: 36px; height: 36px; }
	.icon-btn-lg { width: 44px; height: 44px; }
</style>
