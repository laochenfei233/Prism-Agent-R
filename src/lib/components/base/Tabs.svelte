<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		tabs = [],
		active = $bindable(0),
		onchange
	}: {
		tabs: { label: string; id?: string }[];
		active?: number;
		onchange?: (index: number) => void;
	} = $props();

	function select(index: number) {
		active = index;
		onchange?.(index);
	}
</script>

<div class="tabs" role="tablist">
	{#each tabs as tab, i}
		<button
			class="tab"
			class:active={active === i}
			role="tab"
			aria-selected={active === i}
			onclick={() => select(i)}
		>
			{tab.label}
		</button>
	{/each}
</div>

<style>
	.tabs {
		display: flex;
		gap: var(--space-1);
		border-bottom: 1px solid var(--color-separator);
		overflow-x: auto;
	}
	.tab {
		padding: var(--space-2) var(--space-3);
		border: none;
		background: none;
		cursor: pointer;
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-fg-secondary);
		border-bottom: 2px solid transparent;
		transition: color var(--duration-fast), border-color var(--duration-fast);
		white-space: nowrap;
	}
	.tab:hover { color: var(--color-fg); }
	.tab.active {
		color: var(--color-accent);
		border-bottom-color: var(--color-accent);
	}
</style>
