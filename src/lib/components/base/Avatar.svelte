<script lang="ts">
	let {
		src = '',
		name = '',
		size = 'md'
	}: {
		src?: string;
		name?: string;
		size?: 'sm' | 'md' | 'lg';
	} = $props();

	let initials = $derived(
		name.split(' ').map(w => w[0]).join('').toUpperCase().slice(0, 2)
	);

	let colors = ['var(--color-accent)', 'var(--color-green)', 'var(--color-orange)', 'var(--color-purple)', 'var(--color-red)', 'var(--color-teal)'];
	let colorIndex = $derived(name.charCodeAt(0) % colors.length);
</script>

{#if src}
	<img class="avatar avatar-{size}" {src} alt={name} />
{:else}
	<div
		class="avatar avatar-{size} avatar-fallback"
		style:background={colors[colorIndex]}
	>
		{initials}
	</div>
{/if}

<style>
	.avatar {
		border-radius: 50%;
		object-fit: cover;
		flex-shrink: 0;
	}
	.avatar-sm { width: 28px; height: 28px; font-size: var(--text-xs); }
	.avatar-md { width: 36px; height: 36px; font-size: var(--text-sm); }
	.avatar-lg { width: 48px; height: 48px; font-size: var(--text-base); }
	.avatar-fallback {
		display: flex;
		align-items: center;
		justify-content: center;
		color: #fff;
		font-weight: 600;
	}
</style>
