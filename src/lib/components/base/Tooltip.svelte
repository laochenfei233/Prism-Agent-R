<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		content = '',
		position = 'top',
		children
	}: {
		content?: string;
		position?: 'top' | 'bottom' | 'left' | 'right';
		children: Snippet;
	} = $props();

	let show = $state(false);
</script>

<div
	class="tooltip-wrap"
	onmouseenter={() => show = true}
	onmouseleave={() => show = false}
>
	{@render children()}
	{#if show && content}
		<div class="tooltip tooltip-{position}" role="tooltip">
			{content}
		</div>
	{/if}
</div>

<style>
	.tooltip-wrap { position: relative; display: inline-flex; }
	.tooltip {
		position: absolute;
		padding: 4px 8px;
		border-radius: var(--radius-sm);
		background: var(--color-fg);
		color: var(--color-bg);
		font-size: var(--text-xs);
		white-space: nowrap;
		z-index: 500;
		pointer-events: none;
		animation: fadeIn var(--duration-fast) ease;
	}
	.tooltip-top { bottom: calc(100% + 6px); left: 50%; transform: translateX(-50%); }
	.tooltip-bottom { top: calc(100% + 6px); left: 50%; transform: translateX(-50%); }
	.tooltip-left { right: calc(100% + 6px); top: 50%; transform: translateY(-50%); }
	.tooltip-right { left: calc(100% + 6px); top: 50%; transform: translateY(-50%); }

	@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
</style>
