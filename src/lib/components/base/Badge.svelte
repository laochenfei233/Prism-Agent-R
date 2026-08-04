<script lang="ts">
	type BadgeVariant = 'default' | 'success' | 'warning' | 'error' | 'accent';

	let {
		variant = 'default',
		children
	}: {
		variant?: BadgeVariant;
		children: any;
	} = $props();
</script>

<span class="badge badge-{variant}">
	{@render children()}
</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		padding: 2px 8px;
		border-radius: var(--radius-pill);
		font-size: var(--text-xs);
		font-weight: 600;
		line-height: 1.4;
	}
	.badge-default { background: var(--color-bg-secondary); color: var(--color-fg-secondary); }
	.badge-success { background: rgba(52, 199, 89, 0.15); color: var(--color-green); }
	.badge-warning { background: rgba(255, 149, 0, 0.15); color: var(--color-orange); }
	.badge-error { background: rgba(255, 59, 48, 0.15); color: var(--color-red); }
	.badge-accent { background: rgba(0, 113, 227, 0.15); color: var(--color-accent); }
</style>
