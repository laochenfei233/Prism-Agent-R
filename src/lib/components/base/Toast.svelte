<script lang="ts">
	type ToastType = 'success' | 'error' | 'info';

	let {
		message = '',
		type = 'info',
		duration = 3000,
		onclose
	}: {
		message?: string;
		type?: ToastType;
		duration?: number;
		onclose?: () => void;
	} = $props();

	let visible = $state(false);

	$effect(() => {
		if (message) {
			visible = true;
			const timer = setTimeout(() => {
				visible = false;
				onclose?.();
			}, duration);
			return () => clearTimeout(timer);
		}
	});
</script>

{#if visible && message}
	<div class="toast toast-{type}">
		<span class="icon">
			{#if type === 'success'}✓{:else if type === 'error'}✕{:else}i{/if}
		</span>
		<span class="message">{message}</span>
	</div>
{/if}

<style>
	.toast {
		position: fixed;
		bottom: var(--space-6);
		right: var(--space-6);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		border: 1px solid var(--color-separator);
		box-shadow: var(--shadow-lg);
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
		z-index: 2000;
		animation: slideUp var(--duration-base) var(--spring);
	}
	.icon {
		width: 20px;
		height: 20px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 12px;
		font-weight: 700;
		flex-shrink: 0;
	}
	.toast-success .icon { background: var(--color-green); color: #fff; }
	.toast-error .icon { background: var(--color-red); color: #fff; }
	.toast-info .icon { background: var(--color-accent); color: #fff; }

	@keyframes slideUp {
		from { opacity: 0; transform: translateY(16px); }
		to { opacity: 1; transform: translateY(0); }
	}
</style>
