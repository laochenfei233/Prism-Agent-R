<script lang="ts">
	let {
		value = $bindable(''),
		placeholder = '',
		disabled = false,
		rows = 4
	}: {
		value?: string;
		placeholder?: string;
		disabled?: boolean;
		rows?: number;
	} = $props();
</script>

<textarea
	class="textarea"
	{placeholder}
	{disabled}
	{rows}
	bind:value
></textarea>

<style>
	.textarea {
		width: 100%;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: var(--text-base);
		font-family: var(--font-sans);
		outline: none;
		resize: vertical;
		transition: border-color var(--duration-fast), box-shadow var(--duration-fast);
	}
	.textarea:focus {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.15);
	}
	.textarea:disabled { opacity: 0.5; cursor: not-allowed; }
	.textarea::placeholder { color: var(--color-fg-tertiary); }
</style>
