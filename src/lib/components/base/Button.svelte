<script lang="ts">
	import type { Snippet } from 'svelte';

	type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
	let {
		variant = 'primary',
		disabled = false,
		size = 'md',
		onclick,
		children
	}: {
		variant?: Variant;
		disabled?: boolean;
		size?: 'sm' | 'md' | 'lg';
		onclick?: () => void;
		children: Snippet;
	} = $props();
</script>

<button
	class="btn btn-{variant} btn-{size}"
	{disabled}
	{onclick}
>
	{@render children()}
</button>

<style>
	.btn {
		border-radius: var(--radius-pill);
		border: none;
		cursor: pointer;
		font-weight: 600;
		font-size: var(--text-base);
		transition: transform var(--duration-fast) var(--spring), opacity var(--duration-fast);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		white-space: nowrap;
	}
	.btn:active { transform: scale(0.96); }
	.btn:disabled { opacity: 0.4; cursor: not-allowed; transform: none; }

	.btn-sm { font-size: var(--text-sm); padding: 5px 12px; }
	.btn-md { font-size: var(--text-base); padding: 7px 16px; }
	.btn-lg { font-size: var(--text-lg); padding: 9px 20px; }

	.btn-primary { background: var(--color-accent); color: #fff; }
	.btn-primary:hover:not(:disabled) { background: var(--color-accent-hover); }

	.btn-secondary { background: var(--color-bg-secondary); color: var(--color-fg); }
	.btn-secondary:hover:not(:disabled) { background: var(--color-bg-tertiary); }

	.btn-ghost { background: transparent; color: var(--color-accent); }
	.btn-ghost:hover:not(:disabled) { background: var(--color-bg-secondary); }

	.btn-danger { background: var(--color-red); color: #fff; }
	.btn-danger:hover:not(:disabled) { opacity: 0.9; }
</style>
