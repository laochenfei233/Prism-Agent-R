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
	.badge-success { background: color-mix(in srgb, var(--color-green) 15%, transparent); color: var(--color-green); }
	.badge-warning { background: color-mix(in srgb, var(--color-orange) 15%, transparent); color: var(--color-orange); }
	.badge-error { background: color-mix(in srgb, var(--color-red) 15%, transparent); color: var(--color-red); }
	.badge-accent { background: color-mix(in srgb, var(--color-accent) 15%, transparent); color: var(--color-accent); }
</style>
