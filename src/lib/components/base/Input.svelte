<script lang="ts">
	let {
		value = $bindable(''),
		placeholder = '',
		disabled = false,
		type = 'text'
	}: {
		value?: string;
		placeholder?: string;
		disabled?: boolean;
		type?: string;
	} = $props();
</script>

<input
	class="input"
	{type}
	{placeholder}
	{disabled}
	bind:value
/>

<style>
	.input {
		width: 100%;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: var(--text-base);
		font-family: var(--font-sans);
		outline: none;
		transition: border-color var(--duration-fast), box-shadow var(--duration-fast);
	}
	.input:focus {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.15);
	}
	.input:disabled { opacity: 0.5; cursor: not-allowed; }
	.input::placeholder { color: var(--color-fg-tertiary); }
</style>
